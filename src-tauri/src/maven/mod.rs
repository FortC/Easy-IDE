//! Maven 工具箱：依赖树解析（mvn dependency:tree -B 输出）+ 冲突检测 + 一键排除（pom.xml 文本级插入）。
//!
//! 子模块：
//! - [`pom`]：离线 pom.xml 解析器（roxmltree），模块树 + 模块依赖，供 MavenPanel「模块结构」视图秒开。

pub mod pom;

use crate::config;
use crate::toolchain;
use std::path::{Path, PathBuf};
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

/// Maven 可执行解析（四级链，返回完整可执行路径）：
/// ① 项目内 mvnw.cmd / mvnw.bat（项目自带真理源）→ ② settings.maven_path（文件直用须是
/// Windows 可执行 Maven；目录视为安装根推导 bin/mvn.cmd）→ ③ toolchain 自动探测（留空也探测）
/// → ④ 报错带检测指引。
/// Windows 下 Unix 脚本（mvnw / bin/mvn）绝不直接 spawn —— 那会报
/// 「%1 不是有效的 Win32 应用程序。(os error 193)」。
pub(crate) fn resolve_mvn(root: &Path, settings: &config::AppSettings) -> Result<String, String> {
    let windows = cfg!(target_os = "windows");
    let mut hints: Vec<String> = Vec::new();

    // ① 项目内 wrapper（Windows 只认 mvnw.cmd / mvnw.bat；仅有 mvnw 时跳过并提示）
    let (wrapper, hint) = resolve_wrapper(root, windows);
    if let Some(h) = hint {
        hints.push(h);
    }
    if let Some(w) = wrapper {
        return Ok(w.to_string_lossy().to_string());
    }

    // ② 设置页指定路径
    let prefer = settings.maven_path.trim();
    if !prefer.is_empty() {
        let p = PathBuf::from(prefer);
        if p.is_file() {
            // 显式配置的文件必须是 Windows 可执行 Maven；格式不对直接报错（不裸报 os error 193）
            validate_mvn_file(&p, windows)?;
            return Ok(p.to_string_lossy().to_string());
        }
        if p.is_dir() {
            if let Some(full) = mvn_from_install_dir(&p, windows) {
                return Ok(full.to_string_lossy().to_string());
            }
            // 装的是 Unix 版（只有 bin/mvn）：跳过并提示，继续自动探测
            if p.join("bin").join("mvn").is_file() {
                hints.push(format!(
                    "设置的 Maven 目录「{}」下只有 bin/mvn（Unix 脚本），Windows 请使用 bin/mvn.cmd",
                    prefer
                ));
            } else {
                hints.push(format!("设置的 Maven 目录「{}」下没有 bin/mvn.cmd，已忽略", prefer));
            }
        } else {
            // 路径不存在：继续走自动探测，并记录原因
            hints.push(format!("设置的 Maven 路径「{}」不存在，已忽略", prefer));
        }
    }

    // ③ 自动探测（settings 留空也必须探测；结果再过一次格式校验兜底）
    let info = toolchain::detect("maven", "");
    if info.found && !info.path.is_empty() {
        if validate_mvn_file(Path::new(&info.path), windows).is_ok() {
            return Ok(info.path);
        }
        hints.push(format!("自动探测到的「{}」不是可执行的 Windows Maven，已跳过", info.path));
    }

    // ④ 全部落空：报错带检测指引（附过程提示）
    let mut msg = String::from(
        "未找到 Maven。已检测：PATH、MAVEN_HOME、M2_HOME、常见安装目录。请三选一：①安装 Maven 并加入 PATH ②到 设置 → 工具链 填写 Maven 路径 ③在项目根放置 mvnw.cmd",
    );
    for h in hints {
        msg.push_str(&format!("\n- {}", h));
    }
    Err(msg)
}

/// ① 项目内 wrapper（纯函数）：Windows 只认 mvnw.cmd / mvnw.bat；
/// 只有 mvnw（Unix 脚本）时跳过并给提示「检测到 mvnw（Unix 脚本），Windows 请提供 mvnw.cmd」。
pub(crate) fn resolve_wrapper(root: &Path, windows: bool) -> (Option<PathBuf>, Option<String>) {
    if windows {
        for name in ["mvnw.cmd", "mvnw.bat"] {
            let w = root.join(name);
            if w.is_file() {
                return (Some(w), None);
            }
        }
        if root.join("mvnw").is_file() {
            return (
                None,
                Some("检测到 mvnw（Unix 脚本），Windows 请提供 mvnw.cmd".to_string()),
            );
        }
        (None, None)
    } else {
        let w = root.join("mvnw");
        if w.is_file() {
            (Some(w), None)
        } else {
            (None, None)
        }
    }
}

/// settings 指向文件时的格式校验（纯函数）：Windows 只接受 mvn.cmd / mvn.bat / mvn.exe；
/// 其余（.txt / sh 脚本等）明确报错「该路径不是可执行的 Windows Maven」，不裸报 os error 193。
fn validate_mvn_file(p: &Path, windows: bool) -> Result<(), String> {
    match toolchain::launch_mode(&p.to_string_lossy(), windows) {
        Ok(_) => Ok(()),
        Err(fmt) => Err(format!(
            "设置中的 Maven 路径「{}」{}，不是可执行的 Windows Maven（需要 mvn.cmd / mvn.exe），请到 设置 → 工具链 修正",
            p.display(),
            toolchain::describe_format(fmt)
        )),
    }
}

/// ② 安装目录推导（纯函数）：bin/mvn.cmd → bin/mvn.bat → bin/mvn.exe；
/// bin/mvn 是 Unix 脚本不采用（提示由调用方负责）。
fn mvn_from_install_dir(p: &Path, windows: bool) -> Option<PathBuf> {
    if windows {
        for exe in ["mvn.cmd", "mvn.bat", "mvn.exe"] {
            let full = p.join("bin").join(exe);
            if full.is_file() {
                return Some(full);
            }
        }
        None
    } else {
        let full = p.join("bin").join("mvn");
        if full.is_file() {
            Some(full)
        } else {
            None
        }
    }
}

/// 构造 mvn 命令：.cmd/.bat 一律经 cmd /C（CreateProcess 不能直接跑批处理）；
/// .exe 直接 spawn；其余格式（.sh / 无扩展名等）绝不直接 spawn，报错说明原因。
fn mvn_command(mvn: &str) -> Result<Command, String> {
    toolchain::command_for(mvn).map_err(|fmt| {
        format!(
            "Maven 路径「{}」{}，不是可执行的 Windows Maven（需要 mvn.cmd / mvn.exe），请到 设置 → 工具链 修正",
            mvn,
            toolchain::describe_format(fmt)
        )
    })
}

/// 模块执行目录：root/module（不存在回退 root）；module 空/None = 根
/// 解析模块工作目录。
///
/// 原来是「目录不存在就静默回退到项目根」——用户选了一个失效模块，命令却跑在根上，
/// 结果看起来成功但改错文件。改为返回 Result：目录缺失/越界都明确报错。
pub fn module_dir(root: &Path, module: Option<&str>) -> Result<PathBuf, String> {
    let m = module.unwrap_or("").trim();
    if m.is_empty() {
        return Ok(root.to_path_buf());
    }
    // 越界防护：模块路径必须是 root 之下的相对路径
    let rel = Path::new(m);
    if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(format!("模块路径非法（不允许越出工作区）：{}", m));
    }
    let dir = root.join(rel);
    if !dir.is_dir() {
        return Err(format!("模块目录不存在：{}", m));
    }
    Ok(dir)
}

/// stdout 是否包含 dependency:tree 的真实输出（插件 goal 段落头）
fn has_dependency_tree_output(stdout: &str) -> bool {
    stdout
        .lines()
        .any(|l| l.contains("maven-dependency-plugin") && l.contains(":tree"))
}

/// 执行 mvn dependency:tree 并解析（-B 批处理模式；注入工具链环境）。
/// root = 项目根（wrapper 探测起点）；module = 模块相对路径（空 = 根），执行目录 root/module。
pub fn dependency_tree(
    app: &AppHandle,
    root: &Path,
    module: Option<&str>,
) -> Result<Vec<ModuleTree>, String> {
    let settings = config::load_settings(app);
    let mvn = resolve_mvn(root, &settings)?;
    let workdir = module_dir(root, module)?;
    let mut cmd = mvn_command(&mvn)?;
    cmd.args(["-B", "dependency:tree"]).current_dir(&workdir);
    for (k, v) in toolchain::build_env(&settings) {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("Maven 启动失败：{}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        // 显式判定：成功仅认退出码 0；非 0 时若拿到完整依赖树输出则降级放行并告警
        if has_dependency_tree_output(&stdout) {
            eprintln!(
                "[maven] mvn dependency:tree 非零退出（{:?}），但已获得依赖树输出，按成功处理",
                out.status.code()
            );
        } else {
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

    #[test]
    fn module_dir_resolve() {
        let root = std::env::temp_dir().join(format!("easyide-mvnmod-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        assert_eq!(module_dir(&root, Some("sub")).unwrap(), root.join("sub"));
        assert_eq!(module_dir(&root, None).unwrap(), root);
        assert_eq!(module_dir(&root, Some("  ")).unwrap(), root);
        // 模块目录不存在必须报错，不能静默回退到项目根（否则会改错 pom）
        assert!(module_dir(&root, Some("missing")).is_err(), "失效模块应报错而非回退根");
        // 越界路径必须被拒
        assert!(module_dir(&root, Some("../..")).is_err(), ".. 越界应被拒");
        assert!(module_dir(&root, Some("sub/../..")).is_err(), "嵌套 .. 越界应被拒");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn mvn_resolution_prefers_wrapper_and_settings() {
        let root = std::env::temp_dir().join(format!("easyide-mvnr-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // ① wrapper 优先（不碰 settings / 自动探测）
        let wrapper = if cfg!(target_os = "windows") {
            root.join("mvnw.cmd")
        } else {
            root.join("mvnw")
        };
        std::fs::write(&wrapper, b"@echo off").unwrap();
        let resolved = resolve_mvn(&root, &config::AppSettings::default()).unwrap();
        assert_eq!(resolved, wrapper.to_string_lossy().to_string());
        std::fs::remove_file(&wrapper).unwrap();

        // ② settings 指向文件直接用
        let custom = root.join("custom-mvn.exe");
        std::fs::write(&custom, b"x").unwrap();
        let mut settings = config::AppSettings::default();
        settings.maven_path = custom.to_string_lossy().to_string();
        assert_eq!(resolve_mvn(&root, &settings).unwrap(), custom.to_string_lossy().to_string());

        // ② settings 指向安装根 → bin/mvn.cmd
        let home = root.join("apache-maven-3.9.6");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        let bin_mvn = home.join("bin").join(if cfg!(target_os = "windows") { "mvn.cmd" } else { "mvn" });
        std::fs::write(&bin_mvn, b"x").unwrap();
        settings.maven_path = home.to_string_lossy().to_string();
        assert_eq!(resolve_mvn(&root, &settings).unwrap(), bin_mvn.to_string_lossy().to_string());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn dependency_tree_output_detection() {
        assert!(has_dependency_tree_output(
            "[INFO] --- maven-dependency-plugin:3.6.1:tree (default-cli) @ app ---\n[INFO] com.x:y:jar:1"
        ));
        assert!(!has_dependency_tree_output("[ERROR] BUILD FAILURE"));
        assert!(!has_dependency_tree_output("[INFO] --- maven-compiler-plugin:3.1:compile"));
    }

    #[test]
    fn wrapper_windows_skips_unix_script_with_hint() {
        let root = std::env::temp_dir().join(format!("easyide-mvnw-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // 仅有 mvnw（Unix 脚本）：跳过并明确提示，绝不返回该路径（否则 spawn 即 os error 193）
        std::fs::write(root.join("mvnw"), b"#!/bin/sh").unwrap();
        let (found, hint) = resolve_wrapper(&root, true);
        assert!(found.is_none());
        assert!(hint.unwrap().contains("检测到 mvnw（Unix 脚本），Windows 请提供 mvnw.cmd"));
        // 出现 mvnw.cmd：优先取它，不再提示
        std::fs::write(root.join("mvnw.cmd"), b"@echo off").unwrap();
        let (found, hint) = resolve_wrapper(&root, true);
        assert_eq!(found.unwrap(), root.join("mvnw.cmd"));
        assert!(hint.is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn settings_wrong_format_gives_readable_error() {
        let root = std::env::temp_dir().join(format!("easyide-mvns-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // .txt：明确报错「不是可执行的 Windows Maven」，不裸报 os error 193
        let txt = root.join("maven.txt");
        std::fs::write(&txt, b"x").unwrap();
        let err = validate_mvn_file(&txt, true).unwrap_err();
        assert!(err.contains("不是可执行的 Windows Maven（需要 mvn.cmd / mvn.exe）"), "err = {}", err);
        assert!(!err.contains("os error 193"), "err = {}", err);
        // 无扩展名 Unix 脚本（bin/mvn / mvnw）同样拒绝
        let sh = root.join("mvn");
        std::fs::write(&sh, b"#!/bin/sh").unwrap();
        let err = validate_mvn_file(&sh, true).unwrap_err();
        assert!(err.contains("不是可执行的 Windows Maven"), "err = {}", err);
        // .cmd / .bat / .exe 放行
        assert!(validate_mvn_file(Path::new(r"C:\maven\bin\mvn.cmd"), true).is_ok());
        assert!(validate_mvn_file(Path::new(r"C:\maven\bin\mvn.bat"), true).is_ok());
        assert!(validate_mvn_file(Path::new(r"C:\maven\bin\mvn.exe"), true).is_ok());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_dir_skips_unix_script_and_prefers_cmd() {
        let home = std::env::temp_dir().join(format!("easyide-mvnd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(home.join("bin")).unwrap();
        // 只有 bin/mvn（Unix 脚本）：不采用
        std::fs::write(home.join("bin").join("mvn"), b"#!/bin/sh").unwrap();
        assert_eq!(mvn_from_install_dir(&home, true), None);
        // bin/mvn.cmd 优先于 mvn.exe
        std::fs::write(home.join("bin").join("mvn.cmd"), b"@echo off").unwrap();
        std::fs::write(home.join("bin").join("mvn.exe"), b"x").unwrap();
        assert_eq!(mvn_from_install_dir(&home, true), Some(home.join("bin").join("mvn.cmd")));
        std::fs::remove_dir_all(&home).ok();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn mvn_command_launch_mode() {
        // .cmd → cmd /C
        let cmd = mvn_command(r"C:\maven\bin\mvn.cmd").unwrap();
        assert_eq!(cmd.get_program(), "cmd");
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
        assert_eq!(args, vec!["/C", r"C:\maven\bin\mvn.cmd"]);
        // .exe 直接 spawn
        let direct = mvn_command(r"C:\maven\bin\mvn.exe").unwrap();
        assert_eq!(direct.get_program(), r"C:\maven\bin\mvn.exe");
        // Unix 脚本：绝不直接 spawn，报错可读
        let err = mvn_command(r"C:\proj\mvnw").unwrap_err();
        assert!(err.contains("不是可执行的 Windows Maven"), "err = {}", err);
        let err = mvn_command(r"C:\maven\bin\mvn").unwrap_err();
        assert!(err.contains("Unix 脚本"), "err = {}", err);
    }
}
