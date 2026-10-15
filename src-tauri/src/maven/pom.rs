//! pom.xml 离线结构解析（B4）
//!
//! 为什么需要：`maven.rs` 原来**从不读 pom.xml**，依赖树完全靠执行
//! `mvn -B dependency:tree` 再解析 stdout。后果是：① 强依赖 mvn 可执行 + 能连仓库，
//! 离线或从未 `mvn install` 过就完全空；② 依赖 `[INFO] ` 前缀，用户 settings.xml
//! 设了 `-q` 就全空；③ `tree_depth` 用 `marker/3+1` 硬编码每层 3 字符，遇到
//! 非标准 logger 层级就乱，而 `attach` 注释明写「层级跳跃兜底：挂到当前层」——
//! 用户看到的层级是错的。
//!
//! 本模块提供**不依赖 mvn** 的静态结构：坐标、父子关系、modules、properties、
//! dependencies、dependencyManagement、build plugins。递归沿 `<modules>` 下钻，
//! 深度上限 6 层（聚合工程罕见超过 4 层）。

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 聚合工程最大下钻深度
const MAX_DEPTH: usize = 6;
/// 单个聚合工程的模块数上限（防畸形 pom 声明上千个模块拖垮扫描）
const MAX_MODULES: usize = 64;

#[derive(Serialize, Clone, Debug, Default)]
pub struct PomRef {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PomDep {
    pub group_id: String,
    pub artifact_id: String,
    /// 未显式声明时由 dependencyManagement 推断（compile）
    pub version: String,
    /// compile | provided | runtime | test | system | import
    pub scope: String,
    pub optional: bool,
    /// 该依赖排除掉的 groupId:artifactId 列表
    pub exclusions: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PomPlugin {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PomInfo {
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    /// jar | war | ear | pom | maven-plugin
    pub packaging: String,
    /// 父模块坐标
    pub parent: Option<PomRef>,
    /// <modules> 声明的相对路径（原样，未校验存在性）
    pub modules: Vec<String>,
    /// <properties>（用于 ${} 插值）
    pub properties: HashMap<String, String>,
    /// <dependencies>
    pub dependencies: Vec<PomDep>,
    /// <dependencyManagement><dependencies>
    pub managed: Vec<PomDep>,
    /// <build><plugins>
    pub plugins: Vec<PomPlugin>,
    /// 相对工作区根的路径（前端展示与跳转用，不能只用 artifactId —— 多模块会重名）
    pub rel_path: String,
    /// 解析失败原因（有值说明降级了）
    pub error: String,
}

impl PomInfo {
    /// `groupId:artifactId`
    pub fn ga(&self) -> String {
        if self.group_id.is_empty() {
            self.artifact_id.clone()
        } else {
            format!("{}:{}", self.group_id, self.artifact_id)
        }
    }
    /// 是否 spring-boot 应用（有 boot 插件且 packaging=jar）
    pub fn is_boot_app(&self) -> bool {
        self.plugins
            .iter()
            .any(|p| p.artifact_id.contains("spring-boot-maven-plugin"))
    }
}

/// 解析单个 pom.xml；`rel` 是它相对工作区根的路径（用于回填 rel_path）
pub fn parse_pom(path: &Path, rel: &str) -> PomInfo {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return PomInfo {
                rel_path: rel.to_string(),
                error: format!("读取失败：{}", e),
                ..Default::default()
            }
        }
    };
    let doc = match roxmltree::Document::parse(&text) {
        Ok(d) => d,
        Err(e) => {
            return PomInfo {
                rel_path: rel.to_string(),
                error: format!("XML 解析失败：{}", e),
                ..Default::default()
            }
        }
    };
    let root = doc.root_element();
    // 带命名空间的 pom（极少见）也要能取到节点
    let root = if root.tag_name().name() == "project" {
        root
    } else {
        root.descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "project")
            .unwrap_or(root)
    };

    let parent = child(root, "parent").map(|p| PomRef {
        group_id: text_of(p, "groupId"),
        artifact_id: text_of(p, "artifactId"),
        version: text_of(p, "version"),
    });

    let mut info = PomInfo {
        group_id: text_of(root, "groupId"),
        artifact_id: text_of(root, "artifactId"),
        version: text_of(root, "version"),
        packaging: non_empty(text_of(root, "packaging"), "jar"),
        parent,
        rel_path: rel.to_string(),
        ..Default::default()
    };

    // <properties> 先收，因为后面要用来做 ${} 插值
    if let Some(props) = child(root, "properties") {
        for n in props.children().filter(|n| n.is_element()) {
            info.properties
                .insert(n.tag_name().name().to_string(), n.text().unwrap_or("").trim().to_string());
        }
    }

    if let Some(mods) = child(root, "modules") {
        for n in mods.children().filter(|n| n.is_element()) {
            if info.modules.len() >= MAX_MODULES {
                break;
            }
            if n.tag_name().name() == "module" {
                let v = n.text().unwrap_or("").trim().to_string();
                if !v.is_empty() {
                    info.modules.push(v);
                }
            }
        }
    }

    if let Some(deps) = child(root, "dependencies") {
        info.dependencies = parse_deps(deps);
    }
    if let Some(dm) = child(root, "dependencyManagement") {
        if let Some(deps) = child(dm, "dependencies") {
            info.managed = parse_deps(deps);
        }
    }
    if let Some(build) = child(root, "build") {
        if let Some(plugins) = child(build, "plugins") {
            for p in plugins.children().filter(|n| n.is_element()) {
                if p.tag_name().name() != "plugin" {
                    continue;
                }
                info.plugins.push(PomPlugin {
                    // 插件坐标省略 groupId 时默认 org.apache.maven.plugins
                    group_id: non_empty(text_of(p, "groupId"), "org.apache.maven.plugins"),
                    artifact_id: text_of(p, "artifactId"),
                    version: text_of(p, "version"),
                });
            }
        }
    }

    resolve_properties(&mut info, root);
    info
}

/// 解析 <dependencies> 下的全部 <dependency>
fn parse_deps(deps: roxmltree::Node) -> Vec<PomDep> {
    let mut out = Vec::new();
    for d in deps.children().filter(|n| n.is_element()) {
        if d.tag_name().name() != "dependency" {
            continue;
        }
        let mut exclusions = Vec::new();
        if let Some(ex) = child(d, "exclusions") {
            for e in ex.children().filter(|n| n.is_element()) {
                if e.tag_name().name() != "exclusion" {
                    continue;
                }
                let g = text_of(e, "groupId");
                let a = text_of(e, "artifactId");
                if !a.is_empty() {
                    exclusions.push(if g.is_empty() {
                        a
                    } else {
                        format!("{}:{}", g, a)
                    });
                }
            }
        }
        out.push(PomDep {
            group_id: text_of(d, "groupId"),
            artifact_id: text_of(d, "artifactId"),
            version: text_of(d, "version"),
            scope: non_empty(text_of(d, "scope"), "compile"),
            optional: text_of(d, "optional").eq_ignore_ascii_case("true"),
            exclusions,
        });
    }
    out
}

/// ${} 属性插值：`${project.version}` / `${project.groupId}` + <properties>。
/// 迭代两轮即可覆盖常见的嵌套引用（如 ${spring.version} 又引用另一个属性）。
fn resolve_properties(info: &mut PomInfo, root: roxmltree::Node) {
    // 先把坐标快照出来：subst 需要读它们，而 info 随后要被可变借用，两边不能同时存在
    let own_version = info.version.clone();
    let own_group = info.group_id.clone();
    let own_artifact = info.artifact_id.clone();
    let subst = |v: &str, props: &HashMap<String, String>, depth: usize| -> String {
        let own_version = &own_version;
        let own_group = &own_group;
        let own_artifact = &own_artifact;
        if depth == 0 || !v.contains("${") {
            return v.to_string();
        }
        let mut cur = v.to_string();
        for _ in 0..8 {
            let next = cur.clone();
            let mut out = String::with_capacity(cur.len());
            let mut rest = next.as_str();
            while let Some(i) = rest.find("${") {
                out.push_str(&rest[..i]);
                match rest[i + 2..].find('}') {
                    Some(j) => {
                        let key = &rest[i + 2..i + 2 + j];
                        let val = match key {
                            "project.version" | "version" => own_version.clone(),
                            "project.groupId" | "pom.groupId" => own_group.clone(),
                            "project.artifactId" | "pom.artifactId" => own_artifact.clone(),
                            other => props.get(other).cloned().unwrap_or_default(),
                        };
                        if val.is_empty() {
                            // 解析不出来就保留原样，便于用户看到是哪个属性缺失
                            out.push_str("${");
                            out.push_str(key);
                            out.push('}');
                        } else {
                            out.push_str(&val);
                        }
                        rest = &rest[i + 2 + j + 1..];
                    }
                    None => break,
                }
            }
            out.push_str(rest);
            cur = out;
            if !cur.contains("${") {
                break;
            }
        }
        cur
    };

    // properties 自身也要插值（属性可引用属性）
    let props_snapshot = info.properties.clone();
    let keys: Vec<String> = info.properties.keys().cloned().collect();
    for k in keys {
        let v = info.properties.get(&k).cloned().unwrap_or_default();
        if v.contains("${") {
            let r = subst(&v, &props_snapshot, 2);
            info.properties.insert(k, r);
        }
    }

    info.version = subst(&info.version, &info.properties, 3);
    if let Some(p) = &mut info.parent {
        p.version = subst(&p.version, &info.properties, 3);
    }
    for d in info.dependencies.iter_mut().chain(info.managed.iter_mut()) {
        d.version = subst(&d.version, &info.properties, 3);
    }
    for pl in info.plugins.iter_mut() {
        pl.version = subst(&pl.version, &info.properties, 3);
    }
    let _ = root;
}

fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

fn text_of(node: roxmltree::Node, name: &str) -> String {
    child(node, name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn non_empty(v: String, d: &str) -> String {
    if v.is_empty() {
        d.to_string()
    } else {
        v
    }
}

/// 聚合工程模块树（B4 的核心产出）
#[derive(Serialize, Clone, Debug)]
pub struct ModuleNode {
    /// 相对工作区根的路径
    pub rel_path: String,
    pub group_id: String,
    pub artifact_id: String,
    pub version: String,
    pub packaging: String,
    /// 父模块的 rel_path（空 = 顶层聚合 pom）
    pub parent: Option<String>,
    /// 树形深度：0 = 顶层聚合 pom
    pub depth: usize,
    /// <modules> 声明数
    pub module_count: usize,
    /// 直接依赖数
    pub dep_count: usize,
    /// dependencyManagement 条数
    pub managed_count: usize,
    pub is_boot_app: bool,
    /// 自身是否解析失败
    pub error: String,
}

/// 递归解析聚合工程结构。
///
/// 与 `project_model::collect_submodules` 的区别（那边是给配方预填用的浅层扫描）：
///  - 沿 `<modules>` **声明**下钻，不是「任意子目录里有 pom 就算模块」
///  - 校验每个声明路径确实存在 pom.xml，不存在则跳过（旧实现会收脏数据）
///  - 保留 `parent` / `depth`，前端才能画出真正的层级（原来只有扁平列表，树形信息全丢）
pub fn scan_module_tree(root: &Path) -> Vec<ModuleNode> {
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    walk(root, root, None, 0, &mut out, &mut seen);
    out
}

fn walk(
    root: &Path,
    dir: &Path,
    parent_rel: Option<String>,
    depth: usize,
    out: &mut Vec<ModuleNode>,
    seen: &mut Vec<String>,
) {
    if depth > MAX_DEPTH || out.len() > 400 {
        return;
    }
    let pom = dir.join("pom.xml");
    if !pom.is_file() {
        return;
    }
    let rel = dir
        .strip_prefix(root)
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .replace('\\', "/");
    let rel = if rel.is_empty() { ".".to_string() } else { rel };
    // 同一 pom 被多个父声明（模块路径写重叠）时只解析一次
    if seen.contains(&rel) {
        return;
    }
    seen.push(rel.clone());

    let info = parse_pom(&pom, &rel);
    // groupId 缺省继承 parent（聚合 pom 常见只写 artifactId）
    let group_id = if info.group_id.is_empty() {
        info.parent.as_ref().map(|p| p.group_id.clone()).unwrap_or_default()
    } else {
        info.group_id.clone()
    };

    out.push(ModuleNode {
        rel_path: rel.clone(),
        group_id,
        artifact_id: info.artifact_id.clone(),
        version: info.version.clone(),
        packaging: info.packaging.clone(),
        parent: parent_rel.clone(),
        depth,
        module_count: info.modules.len(),
        dep_count: info.dependencies.len(),
        managed_count: info.managed.len(),
        is_boot_app: info.is_boot_app(),
        error: info.error.clone(),
    });

    for m in &info.modules {
        // 声明可能是 `../other`（少见但合法），规范化后再拼
        let child_dir = if m.starts_with("..") {
            dir.join(m)
        } else {
            dir.join(m)
        };
        if !child_dir.join("pom.xml").is_file() {
            continue;
        }
        walk(root, &child_dir, Some(rel.clone()), depth + 1, out, seen);
    }
}

/// 某个模块的完整依赖明细（含继承自 dependencyManagement 的版本）
pub fn module_dependencies(root: &Path, rel: &str) -> Vec<PomDep> {
    let dir = if rel.is_empty() || rel == "." {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    let pom = dir.join("pom.xml");
    if !pom.is_file() {
        return Vec::new();
    }
    let info = parse_pom(&pom, rel);
    // 未声明版本的依赖从 dependencyManagement 补全
    let mut out = info.dependencies.clone();
    for d in out.iter_mut() {
        if d.version.is_empty() {
            if let Some(m) = info
                .managed
                .iter()
                .find(|m| m.group_id == d.group_id && m.artifact_id == d.artifact_id)
            {
                d.version = m.version.clone();
                if d.scope == "compile" && m.scope != "compile" && m.scope != "import" {
                    d.scope = m.scope.clone();
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.group_id
            .cmp(&b.group_id)
            .then(a.artifact_id.cmp(&b.artifact_id))
    });
    out
}

/// 找某个 artifactId 对应的模块路径（前端「跳到模块」用）
pub fn find_module(root: &Path, artifact: &str) -> Option<PathBuf> {
    scan_module_tree(root)
        .into_iter()
        .find(|m| m.artifact_id == artifact)
        .map(|m| root.join(m.rel_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 在 `root/rel` 下写一份 pom.xml（rel 为空表示工作区根）
    fn w(root: &Path, rel: &str, body: &str) {
        let dir = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pom.xml"), body).unwrap();
    }

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("easyide-pom-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn parses_basic_coordinates() {
        let root = tmp();
        w(&root, "", r#"<project><modelVersion>4.0.0</modelVersion>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>1.0.0</version>
        </project>"#);
        let p = parse_pom(&root.join("pom.xml"), ".");
        assert_eq!(p.group_id, "com.demo");
        assert_eq!(p.artifact_id, "app");
        assert_eq!(p.version, "1.0.0");
        assert_eq!(p.packaging, "jar", "缺省 packaging 应回落 jar");
        assert!(p.error.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn resolves_property_interpolation() {
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>${rev}</version>
          <properties><rev>2.1.0</rev><spring.version>6.0.9</spring.version></properties>
          <dependencies><dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
            <version>${spring.version}</version>
          </dependency></dependencies>
        </project>"#);
        let p = parse_pom(&root.join("pom.xml"), ".");
        // 注意：assert_eq 的第三参是格式串，${{rev}} 里的 {{ 要写成双括号转义
        assert_eq!(p.version, "2.1.0", "${{rev}} 应被 properties 替换");
        assert_eq!(p.dependencies[0].version, "6.0.9", "嵌套属性插值");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn resolves_project_version_token() {
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>3.0</version>
          <dependencies><dependency>
            <groupId>x</groupId><artifactId>y</artifactId><version>${project.version}</version>
          </dependency></dependencies>
        </project>"#);
        let p = parse_pom(&root.join("pom.xml"), ".");
        assert_eq!(p.dependencies[0].version, "3.0");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handles_comments_and_cdata() {
        // regex 抠 XML 在注释/CDATA 上会出错，这正是引入 roxmltree 的原因
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>1.0</version>
          <!-- <modules><module>ghost</module></modules> 这段在注释里，不该被识别 -->
          <modules><module>real</module></modules>
        </project>"#);
        let p = parse_pom(&root.join("pom.xml"), ".");
        assert_eq!(p.modules, vec!["real".to_string()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scans_nested_module_tree_with_depth() {
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>parent</artifactId><version>1.0</version>
          <packaging>pom</packaging>
          <modules><module>svc</module><module>lib</module><module>ghost</module></modules>
        </project>"#);
        w(&root, "svc", r#"<project>
          <parent><groupId>com.demo</groupId><artifactId>parent</artifactId><version>1.0</version></parent>
          <artifactId>svc</artifactId>
          <packaging>pom</packaging>
          <modules><module>api</module></modules>
        </project>"#);
        w(&root, "svc/api", r#"<project>
          <parent><groupId>com.demo</groupId><artifactId>svc</artifactId><version>1.0</version></parent>
          <artifactId>api</artifactId>
        </project>"#);
        w(&root, "lib", r#"<project>
          <parent><groupId>com.demo</groupId><artifactId>parent</artifactId><version>1.0</version></parent>
          <artifactId>lib</artifactId>
        </project>"#);
        // ghost 声明在 <modules> 里但没有 pom.xml，应被跳过
        let tree = scan_module_tree(&root);
        let mut paths: Vec<&str> = tree.iter().map(|m| m.rel_path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(paths, vec![".", "lib", "svc", "svc/api"], "ghost 不该进树；api 应在 svc 下");
        let svc = tree.iter().find(|m| m.rel_path == "svc").unwrap();
        assert_eq!(svc.depth, 1);
        // 缺 groupId 时继承 parent
        assert_eq!(svc.group_id, "com.demo", "groupId 应从 parent 继承");
        assert_eq!(svc.parent.as_deref(), Some("."));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn detects_spring_boot_app() {
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>parent</artifactId><version>1.0</version>
          <packaging>pom</packaging><modules><module>svc</module></modules>
        </project>"#);
        w(&root, "svc", r#"<project>
          <groupId>com.demo</groupId><artifactId>svc</artifactId><version>1.0</version>
          <build><plugins><plugin>
            <artifactId>spring-boot-maven-plugin</artifactId>
          </plugin></plugins></build>
        </project>"#);
        w(&root, "lib", r#"<project>
          <groupId>com.demo</groupId><artifactId>lib</artifactId><version>1.0</version>
        </project>"#);
        let tree = scan_module_tree(&root);
        let svc = tree.iter().find(|m| m.rel_path == "svc").expect("svc 应在树里");
        assert!(svc.is_boot_app, "含 spring-boot-maven-plugin 的模块应标记为 boot app");
        let parent = tree.iter().find(|m| m.rel_path == ".").unwrap();
        assert!(!parent.is_boot_app, "聚合 pom 不该被标记");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn fills_version_from_dependency_management() {
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>1.0</version>
          <dependencyManagement><dependencies><dependency>
            <groupId>junit</groupId><artifactId>junit</artifactId><version>4.13.2</version>
          </dependency></dependencies></dependencyManagement>
          <dependencies><dependency>
            <groupId>junit</groupId><artifactId>junit</artifactId><scope>test</scope>
          </dependency></dependencies>
        </project>"#);
        let deps = module_dependencies(&root, "");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version, "4.13.2", "版本应从 dependencyManagement 补全");
        assert_eq!(deps[0].scope, "test");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn parses_exclusions() {
        let root = tmp();
        w(&root, "", r#"<project>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>1.0</version>
          <dependencies><dependency>
            <groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter</artifactId>
            <exclusions><exclusion>
              <groupId>org.slf4j</groupId><artifactId>slf4j-log4j12</artifactId>
            </exclusion></exclusions>
          </dependency></dependencies>
        </project>"#);
        let deps = module_dependencies(&root, "");
        assert_eq!(deps[0].exclusions, vec!["org.slf4j:slf4j-log4j12".to_string()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn malformed_xml_reports_error_not_panic() {
        let root = tmp();
        std::fs::write(root.join("pom.xml"), "<project><artifactId>x</proj").unwrap();
        let p = parse_pom(&root.join("pom.xml"), ".");
        assert!(!p.error.is_empty(), "应给出错误原因而不是静默返回空");
        // 树扫描不应 panic
        let tree = scan_module_tree(&root);
        assert_eq!(tree.len(), 1);
        assert!(!tree[0].error.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handles_namespaced_pom() {
        let root = tmp();
        w(&root, "", r#"<project xmlns="http://maven.apache.org/POM/4.0.0">
          <modelVersion>4.0.0</modelVersion>
          <groupId>com.demo</groupId><artifactId>app</artifactId><version>1.0</version>
        </project>"#);
        let p = parse_pom(&root.join("pom.xml"), ".");
        assert_eq!(p.artifact_id, "app", "带命名空间的 pom 也应能解析");
        std::fs::remove_dir_all(&root).ok();
    }
}