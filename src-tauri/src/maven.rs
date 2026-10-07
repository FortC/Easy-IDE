//! Maven 工具箱：依赖树解析（mvn dependency:tree -B 输出）+ 冲突检测 + 一键排除（pom.xml 文本级插入）。

use crate::config;
use crate::toolchain;
use std::path::Path;
use std::process::Command;
use tauri::AppHandle;

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct DepNode {
    /// group:artifact:version（显示用组合键）
    pub gav: String,
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub scope: String,
    /// 树深度（0 = 模块根）
    pub depth: usize,
    /// 冲突信息：被保留的更高版本号；None = 正常
    pub conflict_with: Option<String>,
    pub children: Vec<DepNode>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ModuleTree {
    pub module: String,
    pub root: DepNode,
}

/// 执行 mvn dependency:tree 并解析（-B 批处理模式；注入工具链环境）
pub fn dependency_tree(app: &AppHandle, root: &Path) -> Result<Vec<ModuleTree>, String> {
    let settings = config::load_settings(app);
    let mvn = if settings.maven_path.trim().is_empty() {
        "mvn".to_string()
    } else {
        let info = toolchain::detect("maven", &settings.maven_path);
        if !info.found {
            return Err("未找到 Maven，请到 设置 → 工具链 配置或安装".into());
        }
        info.path
    };
    let mut cmd = Command::new(&mvn);
    cmd.args(["-B", "dependency:tree"]).current_dir(root);
    for (k, v) in toolchain::build_env(&settings) {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Maven 启动失败：{}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() && !stdout.contains("maven-dependency-plugin") {
        let tail: String = stdout
            .lines()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("mvn dependency:tree 失败：\n{}", tail));
    }
    Ok(parse_tree(&stdout))
}

/// 解析 dependency:tree 文本（支持多模块多段）
pub fn parse_tree(text: &str) -> Vec<ModuleTree> {
    let mut modules: Vec<ModuleTree> = Vec::new();
    let mut current_root: Option<DepNode> = None;

    for line in text.lines() {
        let Some(info) = line.strip_prefix("[INFO] ") else { continue };
        let info = info.trim_end();
        // 段落头：--- maven-dependency-plugin:...:tree (...) @ module ---
        if info.starts_with("---") && info.contains(" @ ") {
            if let Some(root) = current_root.take() {
                modules.push(ModuleTree { module: root.artifact.clone(), root });
            }
            continue;
        }
        let Some(node) = parse_line(info.trim_start()) else { continue };
        let depth = tree_depth(info);
        match current_root.as_mut() {
            None => {
                let mut root = node;
                root.depth = 0;
                current_root = Some(root);
            }
            Some(root) => attach(root, node, depth),
        }
    }
    if let Some(root) = current_root.take() {
        modules.push(ModuleTree { module: root.artifact.clone(), root });
    }
    modules
}

/// 单行 → 节点（不含深度）。非 GAV 行返回 None。
fn parse_line(s: &str) -> Option<DepNode> {
    // 分离行尾冲突标注： (version X omitted for conflict with Y)
    let (main, conflict) = if let Some(idx) = s.find("(version ") {
        let tail = &s[idx..];
        if tail.contains("omitted for conflict") {
            let inner = tail
                .trim_start_matches("(version ")
                .trim_end_matches(')');
            // inner: "1.8.0 omitted for conflict with 1.9.0"
            let with = inner
                .split("omitted for conflict with")
                .nth(1)
                .map(|w| w.trim().to_string());
            (s[..idx].trim_end(), with)
        } else {
            // (version managed from X) 等其它标注：不当作冲突
            (s[..idx].trim_end(), None)
        }
    } else {
        (s, None)
    };
    let parts: Vec<&str> = main.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    // 去掉树前缀符号：group 字段里第一个字母数字前的 +- \- | 空格都算前缀
    let first = parts[0];
    let gav_start = first
        .find(|c: char| c.is_ascii_alphanumeric())
        .unwrap_or(0);
    let group = first[gav_start..].to_string();
    if group.is_empty() {
        return None;
    }
    let artifact = parts.get(1)?.to_string();
    // 标准格式 group:artifact:type:version[:scope]；老格式 group:artifact:version[:scope]
    const SCOPES: [&str; 7] = ["compile", "test", "runtime", "provided", "system", "import", "optional"];
    let (version, scope) = if parts.len() >= 5 {
        (parts[3].to_string(), parts[4].to_string())
    } else if parts.len() == 4 && SCOPES.contains(&parts[3]) {
        (parts[2].to_string(), parts[3].to_string())
    } else if parts.len() == 4 {
        (parts[3].to_string(), String::new())
    } else {
        (parts.get(2)?.to_string(), String::new())
    };
    Some(DepNode {
        gav: format!("{}:{}:{}", group, artifact, version),
        group,
        artifact,
        version,
        scope,
        depth: 0,
        conflict_with: conflict,
        children: Vec::new(),
    })
}

/// 树前缀深度：maven 树每层占 3 字符（"|  " 或 "   "），层标记 "+- " 或 "\\- "；
/// depth = 标记位置 / 3 + 1；无标记（模块根行）= 0
fn tree_depth(prefix_line: &str) -> usize {
    let marker = prefix_line
        .find("+- ")
        .or_else(|| prefix_line.find("\\- "))
        .unwrap_or(usize::MAX);
    if marker == usize::MAX {
        return 0;
    }
    marker / 3 + 1
}

/// 按深度挂到最近的祖先（depth ≥ 1）
fn attach(root: &mut DepNode, node: DepNode, depth: usize) {
    let mut cur: &mut DepNode = root;
    let target = depth.max(1);
    for _ in 1..target {
        if cur.children.is_empty() {
            break; // 层级跳跃兜底：挂到当前层
        }
        cur = cur.children.last_mut().unwrap();
    }
    let mut n = node;
    n.depth = target;
    cur.children.push(n);
}

/// 一键排除：在 pom.xml 直接依赖 (dep_group:dep_artifact) 中加入
/// <exclusion>(excl_group:excl_artifact)，返回修改后的 pom 文本。
pub fn apply_exclusion(
    pom: &str,
    dep_group: &str,
    dep_artifact: &str,
    excl_group: &str,
    excl_artifact: &str,
) -> Result<String, String> {
    let dep_start = find_dependency_block(pom, dep_group, dep_artifact)
        .ok_or_else(|| format!("pom.xml 中未找到直接依赖 {}:{}（可能来自 BOM/父 POM）", dep_group, dep_artifact))?;
    let dep_end = dep_start
        + pom[dep_start..]
            .find("</dependency>")
            .ok_or("pom.xml 依赖块结构异常")?;
    let block = &pom[dep_start..dep_end];

    // 查重：排除项已存在
    if block.contains(&format!("<groupId>{}</groupId>", excl_group))
        && block.contains(&format!("<artifactId>{}</artifactId>", excl_artifact))
    {
        return Err("该排除已存在".into());
    }

    let excl_xml = format!(
        "      <exclusion>\n        <groupId>{}</groupId>\n        <artifactId>{}</artifactId>\n      </exclusion>\n",
        excl_group, excl_artifact
    );

    let (insert_at, insertion) = if let Some(rel) = block.find("</exclusions>") {
        // 已有 exclusions：追加一个 exclusion
        (dep_start + rel, excl_xml)
    } else {
        // 无 exclusions：在 </dependency> 前插入整块
        (
            dep_end,
            format!("    <exclusions>\n{}    </exclusions>\n  ", excl_xml),
        )
    };

    let mut out = String::with_capacity(pom.len() + insertion.len());
    out.push_str(&pom[..insert_at]);
    out.push_str(&insertion);
    out.push_str(&pom[insert_at..]);
    Ok(out)
}

/// 定位直接依赖 <dependency> 块起点（优先带 <version> 的真实声明）
fn find_dependency_block(pom: &str, group: &str, artifact: &str) -> Option<usize> {
    let mut search_from = 0;
    let mut fallback: Option<usize> = None;
    loop {
        let start = pom[search_from..].find("<dependency>")? + search_from;
        let end = start + pom[start..].find("</dependency>")?;
        let block = &pom[start..end];
        let g_ok = block.contains(&format!("<groupId>{}</groupId>", group));
        let a_ok = block.contains(&format!("<artifactId>{}</artifactId>", artifact));
        if g_ok && a_ok {
            if block.contains("<version>") {
                return Some(start);
            }
            fallback = fallback.or(Some(start));
        }
        search_from = end + "</dependency>".len();
        if search_from >= pom.len() {
            return fallback;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[INFO] --- maven-dependency-plugin:3.6.1:tree (default-cli) @ app-demo ---
[INFO] com.example:app-demo:jar:1.0.0
[INFO] +- org.springframework:spring-core:jar:5.3.20:compile
[INFO] |  +- org.springframework:spring-jcl:jar:5.3.20:compile
[INFO] |  \- io.micrometer:micrometer-core:jar:1.8.0:compile (version 1.8.0 omitted for conflict with 1.9.0)
[INFO] +- junit:junit:jar:4.13.2:test
[INFO] \- com.google.guava:guava:jar:31.1-jre:compile
[INFO] --- maven-dependency-plugin:3.6.1:tree (default-cli) @ child-mod ---
[INFO] com.example:child-mod:jar:1.0.0
[INFO] +- org.apache.commons:commons-lang3:jar:3.12.0:compile
"#;

    #[test]
    fn parse_tree_multi_module() {
        let modules = parse_tree(SAMPLE);
        assert_eq!(modules.len(), 2);
        let root = &modules[0].root;
        assert_eq!(root.gav, "com.example:app-demo:1.0.0");
        assert_eq!(root.children.len(), 3);
        let spring = &root.children[0];
        assert_eq!(spring.gav, "org.springframework:spring-core:5.3.20");
        assert_eq!(spring.children.len(), 2);
        let micro = &spring.children[1];
        assert_eq!(micro.artifact, "micrometer-core");
        assert_eq!(micro.conflict_with.as_deref(), Some("1.9.0"));
        let junit = &root.children[1];
        assert_eq!(junit.scope, "test");
        let child = &modules[1].root;
        assert_eq!(child.children.len(), 1);
        assert_eq!(child.children[0].artifact, "commons-lang3");
    }

    #[test]
    fn tree_depth_calc() {
        assert_eq!(tree_depth("+- a:b:1"), 1);
        assert_eq!(tree_depth("|  +- a:b:1"), 2);
        assert_eq!(tree_depth("|  \\- a:b:1"), 2);
        assert_eq!(tree_depth("com.x:y:1"), 0);
        assert_eq!(tree_depth("|  |  \\- a:b:1"), 3);
    }

    #[test]
    fn exclusion_insertion() {
        let pom = r#"<project>
  <dependencies>
    <dependency>
      <groupId>org.springframework</groupId>
      <artifactId>spring-core</artifactId>
      <version>5.3.20</version>
    </dependency>
  </dependencies>
</project>"#;
        let out = apply_exclusion(pom, "org.springframework", "spring-core", "io.micrometer", "micrometer-core").unwrap();
        assert!(out.contains("<exclusions>"));
        assert!(out.contains("<artifactId>micrometer-core</artifactId>"));
        let dep_pos = out.find("<artifactId>spring-core</artifactId>").unwrap();
        let excl_pos = out.find("<exclusion>").unwrap();
        assert!(excl_pos > dep_pos);
        // 已存在 → 拒绝
        assert!(apply_exclusion(&out, "org.springframework", "spring-core", "io.micrometer", "micrometer-core").is_err());
        // 已有 exclusions → 追加而非嵌套
        let out2 = apply_exclusion(&out, "org.springframework", "spring-core", "junit", "junit").unwrap();
        assert_eq!(out2.matches("<exclusions>").count(), 1);
        assert_eq!(out2.matches("<exclusion>").count(), 2);
    }
}
