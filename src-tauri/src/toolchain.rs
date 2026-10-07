//! 工具链配置：JDK / Maven / Node / Git 的自动探测、版本解析与命令执行环境注入。
//! 路径可由用户在 设置 → 工具链 手动指定；为空时按 环境变量 → where → 常见安装目录 探测。

use crate::config::AppSettings;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, Clone, Debug)]
pub struct ToolInfo {
    pub kind: String,
    pub found: bool,
    /// 可执行文件或主目录的绝对路径
    pub path: String,
    /// 解析出的版本号（探测失败为空）
    pub version: String,
    pub error: String,
}

/// `where <name>` 找第一个可执行文件
fn where_is(name: &str) -> Option<String> {
    let out = std::process::Command::new("cmd")
        .args(["/C", &format!("where {}", name)])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    if line.is_empty() {
        None
    } else {
        Some(line)
    }
}

/// 列出目录下匹配前缀的子目录（如 C:\Program Files\Java\jdk-*）
fn glob_dirs(parent: &str, prefixes: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(parent) else { return out };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if !p.is_dir() {
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if prefixes.iter().any(|pre| name.starts_with(pre)) {
            out.push(p);
        }
    }
    out
}

fn jdk_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    // JAVA_HOME（可能指向 JDK 根，也可能直接是 bin）
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let h = PathBuf::from(&home);
        if h.join("bin").join("java.exe").exists() {
            out.push(h.join("bin").join("java.exe"));
        }
    }
    if let Some(p) = where_is("java") {
        out.push(PathBuf::from(p));
    }
    for base in [
        r"C:\Program Files\Java",
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\Microsoft",
        r"C:\Program Files\Zulu",
        r"C:\Program Files\Amazon Corretto",
        r"C:\Program Files\BellSoft",
        r"C:\Program Files\Android\Android Studio\jbr",
        r"C:\Program Files\JetBrains",
    ] {
        for d in glob_dirs(base, &["jdk", "jbr", "temurin", "zulu", "corretto", "jetty"]) {
            let exe = d.join("bin").join("java.exe");
            if exe.exists() {
                out.push(exe);
            } else if d.file_name().map(|n| n == "bin").unwrap_or(false) {
                out.push(d.join("java.exe"));
            }
        }
    }
    out
}

/// 探测单个工具；prefer 为用户设置路径（空则自动）
pub fn detect(kind: &str, prefer: &str) -> ToolInfo {
    match kind {
        "jdk" => {
            let exe = resolve_preferred(prefer, &["java.exe"])
                .into_iter()
            .chain(jdk_candidates())
            .next();
            match exe {
                Some(p) => {
                    // java -version 输出到 stderr
                    let out = std::process::Command::new(&p).arg("-version").output();
                    let ver = out
                        .ok()
                        .map(|o| {
                            let text = format!(
                                "{}{}",
                                String::from_utf8_lossy(&o.stdout),
                                String::from_utf8_lossy(&o.stderr)
                            );
                            parse_between(&text, '"', '"').unwrap_or_default()
                        })
                        .unwrap_or_default();
                    ToolInfo { kind: "jdk".into(), found: true, path: p.to_string_lossy().to_string(), version: ver, error: String::new() }
                }
                None => ToolInfo { kind: "jdk".into(), found: false, path: String::new(), version: String::new(), error: "未找到 JDK".into() },
            }
        }
        "maven" => {
            let exe = resolve_preferred(prefer, &["mvn.cmd", "mvn.bat", "mvn"])
                .into_iter()
                .chain(maven_candidates())
                .next();
            match exe {
                Some(p) => {
                    let out = std::process::Command::new(&p).arg("-v").output();
                    let ver = out
                        .ok()
                        .and_then(|o| {
                            let text = String::from_utf8_lossy(&o.stdout).to_string();
                            text.lines()
                                .find(|l| l.contains("Apache Maven"))
                                .and_then(|l| l.split_whitespace().nth(2).map(|s| s.to_string()))
                        })
                        .unwrap_or_default();
                    ToolInfo { kind: "maven".into(), found: true, path: p.to_string_lossy().to_string(), version: ver, error: String::new() }
                }
                None => ToolInfo { kind: "maven".into(), found: false, path: String::new(), version: String::new(), error: "未找到 Maven".into() },
            }
        }
        "node" => detect_simple("node", prefer, &["-v"]),
        "git" => detect_simple("git", prefer, &["--version"]),
        _ => ToolInfo { kind: kind.into(), found: false, path: String::new(), version: String::new(), error: "未知工具".into() },
    }
}

fn maven_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = where_is("mvn") {
        out.push(PathBuf::from(p));
    }
    if let Ok(home) = std::env::var("MAVEN_HOME").or_else(|_| std::env::var("M2_HOME")) {
        let h = PathBuf::from(home);
        let mvn = h.join("bin").join("mvn.cmd");
        if mvn.exists() {
            out.push(mvn);
        }
    }
    for base in [r"C:\Program Files\apache-maven", r"C:\apache-maven", r"C:\tools\apache-maven"] {
        for d in glob_dirs(base, &["apache-maven"]) {
            let mvn = d.join("bin").join("mvn.cmd");
            if mvn.exists() {
                out.push(mvn);
            }
        }
    }
    out
}

fn resolve_preferred(prefer: &str, exes: &[&str]) -> Vec<PathBuf> {
    let p = prefer.trim();
    if p.is_empty() {
        return vec![];
    }
    let path = PathBuf::from(p);
    if path.is_file() {
        return vec![path];
    }
    // 指向主目录：补 bin/<exe>
    for exe in exes {
        let full = path.join("bin").join(exe);
        if full.exists() {
            return vec![full];
        }
    }
    vec![]
}

fn detect_simple(kind: &str, prefer: &str, ver_args: &[&str]) -> ToolInfo {
    let exe = resolve_preferred(prefer, &[&format!("{}.exe", kind)])
        .into_iter()
        .chain(where_is(kind).map(PathBuf::from))
        .next();
    match exe {
        Some(p) => {
            let out = std::process::Command::new(&p).args(ver_args).output();
            let ver = out
                .ok()
                .map(|o| {
                    let text = format!(
                        "{}{}",
                        String::from_utf8_lossy(&o.stdout),
                        String::from_utf8_lossy(&o.stderr)
                    );
                    text.split_whitespace().find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit())).unwrap_or("").to_string()
                })
                .unwrap_or_default();
            ToolInfo { kind: kind.into(), found: true, path: p.to_string_lossy().to_string(), version: ver, error: String::new() }
        }
        None => ToolInfo { kind: kind.into(), found: false, path: String::new(), version: String::new(), error: format!("未找到 {}", kind) },
    }
}

/// 取 `open` 与 `close` 之间的内容（如 java -version 的 "17.0.2" ）
fn parse_between(s: &str, open: char, close: char) -> Option<String> {
    let start = s.find(open)?;
    let rest = &s[start + 1..];
    let end = rest.find(close)?;
    Some(rest[..end].to_string())
}

/// 为命令执行构造环境变量（JAVA_HOME / PATH 前置），AI 生成的命令自动使用配置的工具链
pub fn build_env(settings: &AppSettings) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = Vec::new();
    let mut path_prefix: Vec<String> = Vec::new();
    if !settings.jdk_path.trim().is_empty() {
        let info = detect("jdk", &settings.jdk_path);
        if info.found {
            let exe = PathBuf::from(&info.path);
            if let Some(bin) = exe.parent() {
                path_prefix.push(bin.to_string_lossy().to_string());
                if let Some(home) = bin.parent() {
                    env.push(("JAVA_HOME".into(), home.to_string_lossy().to_string()));
                }
            }
        }
    }
    for (setting, kind) in [(&settings.maven_path, "maven"), (&settings.node_path, "node"), (&settings.git_path, "git")] {
        if setting.trim().is_empty() {
            continue;
        }
        let info = detect(kind, setting);
        if info.found {
            if let Some(bin) = Path::new(&info.path).parent() {
                path_prefix.push(bin.to_string_lossy().to_string());
            }
        }
    }
    if !path_prefix.is_empty() {
        let old = std::env::var("PATH").unwrap_or_default();
        env.push(("PATH".into(), format!("{};{}", path_prefix.join(";"), old)));
    }
    env
}
