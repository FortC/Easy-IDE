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

/// Windows `CREATE_NO_WINDOW` = 0x08000000
///
/// 用途：让子进程**不弹控制台窗口**。开发模式下 EasyIDE 自身是 console 子系统，
/// 所有 `std::process::Command` 都会继承并弹出黑窗（cmd/powershell/taskkill/where…），
/// 必须显式加此标志。release 模式下 `main.rs` 的 `windows_subsystem` 只隐藏自身窗口，
/// 隐藏不了子进程，所以两种模式都要加。
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 统一包装：非 Windows 直接返回原命令；Windows 附加 CREATE_NO_WINDOW。
///
/// 所有 spawn 子进程的地方都应经过它，避免漏网（问题 5 的根治点）。
pub fn hidden(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
    cmd
}

/// 构造「已隐藏窗口」的新命令（等价于 `hidden(Command::new(..))`，少写一次可变借用）
pub fn hidden_new(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    hidden(&mut cmd);
    cmd
}

/// `where <name>` 找第一个可执行文件
fn where_is(name: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/C", &format!("where {}", name)]);
    let out = hidden(&mut cmd).output().ok()?;
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

// ---------------- 可执行格式识别（防 os error 193） ----------------
//
// Windows CreateProcess 只能直接启动 PE 文件：把无扩展名的 Unix 脚本
// （项目里的 mvnw、安装目录的 bin/mvn）或 .txt 之类文件直接 spawn 会报
// 「%1 不是有效的 Win32 应用程序。(os error 193)」。因此所有工具可执行
// 路径必须先过这里的格式分类：只接受 .exe / .cmd / .bat，批处理经 cmd /C。

/// Windows 可执行文件格式（按扩展名判定，纯函数，大小写不敏感）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecFormat {
    /// .exe：可直接 CreateProcess
    Exe,
    /// .cmd / .bat：必须经 cmd /C 启动
    Batch,
    /// 裸命令名（无目录无扩展名，如 "git"）：交给系统 PATH/PATHEXT 搜索
    BareName,
    /// 带目录的无扩展名文件 / .sh 等 Unix 脚本：Windows 不能直接执行
    UnixScript,
    /// 其它扩展名（.txt 等）：不是可执行文件
    Unknown,
}

/// 按扩展名判定可执行格式；windows = false 时不拦截（非 Windows 平台脚本可直接 exec）
pub fn exec_format(path: &str, windows: bool) -> ExecFormat {
    if !windows {
        return ExecFormat::Exe;
    }
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("exe") => ExecFormat::Exe,
        Some("cmd") | Some("bat") => ExecFormat::Batch,
        Some("sh") | Some("bash") => ExecFormat::UnixScript,
        Some(_) => ExecFormat::Unknown,
        None => {
            if p.components().count() == 1 {
                ExecFormat::BareName
            } else {
                // 带目录的无扩展名文件（mvnw / bin/mvn 等 Unix 脚本）：绝不直接 spawn
                ExecFormat::UnixScript
            }
        }
    }
}

/// 子进程启动方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// 直接 CreateProcess（.exe / 裸命令名）
    Direct,
    /// 经 cmd /C 启动（.cmd / .bat 批处理）
    ViaCmd,
}

/// 启动方式判定；Windows 拒绝 Unix 脚本/未知格式（Err 带回被拒格式，调用方自定义文案）
pub fn launch_mode(path: &str, windows: bool) -> Result<LaunchMode, ExecFormat> {
    match exec_format(path, windows) {
        ExecFormat::Exe | ExecFormat::BareName => Ok(LaunchMode::Direct),
        ExecFormat::Batch => Ok(LaunchMode::ViaCmd),
        fmt => Err(fmt),
    }
}

/// 拒绝/格式描述（错误文案拼接用）
pub fn describe_format(fmt: ExecFormat) -> &'static str {
    match fmt {
        ExecFormat::Exe => "是 .exe 可执行文件",
        ExecFormat::Batch => "是批处理（需经 cmd /C 启动）",
        ExecFormat::BareName => "是裸命令名",
        ExecFormat::UnixScript => "是 Unix 脚本（Windows 不能直接执行）",
        ExecFormat::Unknown => "不是可执行文件",
    }
}

/// 构造子进程命令：.cmd/.bat → cmd /C；.exe/裸名 → 直接 spawn；
/// 其余格式（.sh / 无扩展名 / .txt 等）拒绝启动，Err 带回被拒格式。
/// 结果已带 CREATE_NO_WINDOW（不弹黑窗）。
pub fn command_for(exe: &str) -> Result<std::process::Command, ExecFormat> {
    match launch_mode(exe, cfg!(target_os = "windows"))? {
        LaunchMode::ViaCmd => {
            let mut cmd = hidden_new("cmd");
            cmd.arg("/C").arg(exe);
            Ok(cmd)
        }
        LaunchMode::Direct => Ok(hidden_new(exe)),
    }
}

/// Windows 候选过滤与同目录升级（防 os error 193）：
/// - .exe / .cmd / .bat → 保留；
/// - Unix 脚本（无扩展名 / .sh）→ 尝试同目录同名升级（bin/mvn → bin/mvn.cmd、mvnw → mvnw.cmd，
///   优先 .cmd 再 .bat 再 .exe），无 Windows 版则跳过；
/// - 其它扩展名（.txt 等）→ 跳过。
/// 非 Windows 平台原样返回。
pub fn normalize_windows_candidates(cands: Vec<PathBuf>) -> Vec<PathBuf> {
    if !cfg!(target_os = "windows") {
        return cands;
    }
    let mut out = Vec::new();
    for p in cands {
        match exec_format(&p.to_string_lossy(), true) {
            ExecFormat::Exe | ExecFormat::Batch | ExecFormat::BareName => out.push(p),
            ExecFormat::UnixScript => {
                if let Some(up) = upgrade_sibling(&p) {
                    out.push(up);
                }
            }
            ExecFormat::Unknown => {}
        }
    }
    out
}

/// Unix 脚本 → 同目录 Windows 可执行升级：优先 .cmd，再 .bat，再 .exe
fn upgrade_sibling(p: &Path) -> Option<PathBuf> {
    let stem = p.file_stem()?.to_string_lossy().to_string();
    for ext in ["cmd", "bat", "exe"] {
        let cand = p.with_file_name(format!("{}.{}", stem, ext));
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// 取第一个格式合法的候选（内部走 normalize_windows_candidates）
fn first_valid(cands: Vec<PathBuf>) -> Option<PathBuf> {
    normalize_windows_candidates(cands).into_iter().next()
}

/// 版本探测：启动 `<exe> <args>` 返回合并输出（stdout+stderr）；启动失败返回 None
fn probe_output(exe: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = command_for(&exe.to_string_lossy()).ok()?;
    let out = cmd.args(args).output().ok()?;
    Some(format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    ))
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

/// 探测单个工具；prefer 为用户设置路径（空则自动）。
/// 所有候选先过 Windows 可执行格式过滤（Unix 脚本/未知格式不参与探测，防 os error 193）。
pub fn detect(kind: &str, prefer: &str) -> ToolInfo {
    match kind {
        "jdk" => {
            let exe = first_valid(resolve_preferred(prefer, &["java.exe"]).into_iter().chain(jdk_candidates()).collect());
            match exe {
                Some(p) => {
                    // java -version 输出到 stderr
                    let ver = probe_output(&p, &["-version"])
                        .map(|text| parse_between(&text, '"', '"').unwrap_or_default())
                        .unwrap_or_default();
                    ToolInfo { kind: "jdk".into(), found: true, path: p.to_string_lossy().to_string(), version: ver, error: String::new() }
                }
                None => ToolInfo { kind: "jdk".into(), found: false, path: String::new(), version: String::new(), error: "未找到 JDK".into() },
            }
        }
        "maven" => {
            let exe = first_valid(resolve_preferred(prefer, &["mvn.cmd", "mvn.bat", "mvn.exe"]).into_iter().chain(maven_candidates()).collect());
            match exe {
                Some(p) => {
                    let ver = probe_output(&p, &["-v"])
                        .and_then(|text| {
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
        "svn" => {
            // 候选：where svn / SVN_HOME / TortoiseSVN / SlikSvn（prefer 为空也要走候选探测）
            let exe = first_valid(resolve_preferred(prefer, &["svn.exe"]).into_iter().chain(svn_candidates()).collect());
            match exe {
                Some(p) => {
                    let ver = probe_output(&p, &["--version", "--quiet"])
                        .map(|text| text.trim().to_string())
                        .unwrap_or_default();
                    ToolInfo { kind: "svn".into(), found: true, path: p.to_string_lossy().to_string(), version: ver, error: String::new() }
                }
                None => ToolInfo { kind: "svn".into(), found: false, path: String::new(), version: String::new(), error: "未找到 SVN".into() },
            }
        }
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

fn svn_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = where_is("svn") {
        out.push(PathBuf::from(p));
    }
    if let Ok(home) = std::env::var("SVN_HOME") {
        let exe = PathBuf::from(home).join("bin").join("svn.exe");
        if exe.exists() {
            out.push(exe);
        }
    }
    for p in [
        r"C:\Program Files\TortoiseSVN\bin\svn.exe",
        r"C:\Program Files\SlikSvn\bin\svn.exe",
    ] {
        let exe = PathBuf::from(p);
        if exe.exists() {
            out.push(exe);
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
        // 用户指定文件同样过滤 Windows 格式（Unix 脚本/未知格式不返回，防 os error 193）
        return normalize_windows_candidates(vec![path]);
    }
    // 指向主目录：补 bin/<exe>（候选同样过滤）
    let found: Vec<PathBuf> = exes
        .iter()
        .map(|exe| path.join("bin").join(exe))
        .filter(|full| full.exists())
        .collect();
    normalize_windows_candidates(found)
}

fn detect_simple(kind: &str, prefer: &str, ver_args: &[&str]) -> ToolInfo {
    let exe = first_valid(
        resolve_preferred(prefer, &[&format!("{}.exe", kind)])
            .into_iter()
            .chain(where_is(kind).map(PathBuf::from))
            .collect(),
    );
    match exe {
        Some(p) => {
            let ver = probe_output(&p, ver_args)
                .map(|text| {
                    text.split_whitespace()
                        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))
                        .unwrap_or("")
                        .to_string()
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

/// 为命令执行构造环境变量（JAVA_HOME / PATH 前置），AI 生成的命令自动使用配置的工具链。
/// 探测涉及 spawn `java -version` 等子进程，按工具路径指纹缓存（配方快速通道 <50ms 的前提）。
static ENV_CACHE: std::sync::Mutex<Option<(String, Vec<(String, String)>)>> = std::sync::Mutex::new(None);

/// 手动失效环境缓存（设置 → 工具链"重新探测"）：新装 Maven/JDK 等后免重启生效
pub fn invalidate() {
    if let Ok(mut cache) = ENV_CACHE.lock() {
        *cache = None;
    }
}

pub fn build_env(settings: &AppSettings) -> Vec<(String, String)> {
    let key = format!(
        "{}|{}|{}|{}",
        settings.jdk_path, settings.maven_path, settings.node_path, settings.git_path
    );
    if let Ok(cache) = ENV_CACHE.lock() {
        if let Some((k, env)) = cache.as_ref() {
            if *k == key {
                return env.clone();
            }
        }
    }
    let env = build_env_uncached(settings);
    if let Ok(mut cache) = ENV_CACHE.lock() {
        *cache = Some((key, env.clone()));
    }
    env
}

fn build_env_uncached(settings: &AppSettings) -> Vec<(String, String)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_format_classify() {
        // .exe 直接可跑；.cmd/.bat 走 cmd /C
        assert_eq!(exec_format(r"C:\maven\bin\mvn.exe", true), ExecFormat::Exe);
        assert_eq!(exec_format(r"C:\maven\bin\mvn.cmd", true), ExecFormat::Batch);
        assert_eq!(exec_format(r"C:\maven\bin\MVN.CMD", true), ExecFormat::Batch);
        assert_eq!(exec_format(r"C:\maven\bin\mvn.bat", true), ExecFormat::Batch);
        // 无扩展名的 Unix 脚本（mvnw / bin/mvn）与 .sh：Windows 不可直接执行
        assert_eq!(exec_format(r"C:\proj\mvnw", true), ExecFormat::UnixScript);
        assert_eq!(exec_format(r"C:\maven\bin\mvn", true), ExecFormat::UnixScript);
        assert_eq!(exec_format(r"C:\tools\mvn.sh", true), ExecFormat::UnixScript);
        // 其它扩展名：不是可执行文件
        assert_eq!(exec_format(r"C:\tools\mvn.txt", true), ExecFormat::Unknown);
        // 裸命令名交给系统 PATH/PATHEXT 搜索
        assert_eq!(exec_format("git", true), ExecFormat::BareName);
        assert_eq!(exec_format("mvn", true), ExecFormat::BareName);
        // 非 Windows 平台不做拦截
        assert_eq!(exec_format("/opt/maven/bin/mvn", false), ExecFormat::Exe);
    }

    #[test]
    fn launch_mode_selection() {
        // .cmd/.bat → cmd /C；.exe/裸名 → 直接 spawn
        assert_eq!(launch_mode(r"C:\maven\bin\mvn.cmd", true), Ok(LaunchMode::ViaCmd));
        assert_eq!(launch_mode(r"C:\maven\bin\mvn.bat", true), Ok(LaunchMode::ViaCmd));
        assert_eq!(launch_mode(r"C:\maven\bin\mvn.exe", true), Ok(LaunchMode::Direct));
        assert_eq!(launch_mode("git", true), Ok(LaunchMode::Direct));
        // Unix 脚本/未知格式：拒绝启动（os error 193 根因）
        assert_eq!(launch_mode(r"C:\proj\mvnw", true), Err(ExecFormat::UnixScript));
        assert_eq!(launch_mode(r"C:\maven\bin\mvn", true), Err(ExecFormat::UnixScript));
        assert_eq!(launch_mode(r"C:\tools\mvn.txt", true), Err(ExecFormat::Unknown));
        // 拒绝原因文案可读
        assert!(describe_format(ExecFormat::UnixScript).contains("Unix 脚本"));
        assert!(describe_format(ExecFormat::Unknown).contains("不是可执行文件"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn command_for_wraps_batch_with_cmd_c() {
        let cmd = command_for(r"C:\maven\bin\mvn.cmd").unwrap();
        assert_eq!(cmd.get_program(), "cmd");
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
        assert_eq!(args, vec!["/C", r"C:\maven\bin\mvn.cmd"]);
        let direct = command_for(r"C:\maven\bin\mvn.exe").unwrap();
        assert_eq!(direct.get_program(), r"C:\maven\bin\mvn.exe");
        let bare = command_for("git").unwrap();
        assert_eq!(bare.get_program(), "git");
        // Unix 脚本绝不直接 spawn
        assert_eq!(command_for(r"C:\proj\mvnw").err(), Some(ExecFormat::UnixScript));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn candidates_filter_unix_script_and_upgrade() {
        let root = std::env::temp_dir().join(format!("easyide-toolc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("bin")).unwrap();
        let mvn_sh = root.join("bin").join("mvn");
        std::fs::write(&mvn_sh, b"#!/bin/sh").unwrap();

        // 只有 Unix 脚本 → 被跳过，绝不进入候选（否则 spawn 即 os error 193）
        assert!(normalize_windows_candidates(vec![mvn_sh.clone()]).is_empty());

        // 同目录出现 bin/mvn.cmd → 升级为 mvn.cmd（bin/mvn.cmd 优先）
        let mvn_cmd = root.join("bin").join("mvn.cmd");
        std::fs::write(&mvn_cmd, b"@echo off").unwrap();
        assert_eq!(normalize_windows_candidates(vec![mvn_sh.clone()]), vec![mvn_cmd.clone()]);
        // .cmd/.exe 本身直接保留
        let mvn_exe = root.join("bin").join("mvn.exe");
        std::fs::write(&mvn_exe, b"x").unwrap();
        assert_eq!(
            normalize_windows_candidates(vec![mvn_cmd.clone(), mvn_exe.clone()]),
            vec![mvn_cmd.clone(), mvn_exe.clone()]
        );

        // mvnw（Unix 脚本）→ 升级为 mvnw.cmd；没有 Windows 版则跳过并可提示
        let mvnw = root.join("mvnw");
        std::fs::write(&mvnw, b"#!/bin/sh").unwrap();
        assert!(normalize_windows_candidates(vec![mvnw.clone()]).is_empty());
        let mvnw_cmd = root.join("mvnw.cmd");
        std::fs::write(&mvnw_cmd, b"@echo off").unwrap();
        assert_eq!(normalize_windows_candidates(vec![mvnw]), vec![mvnw_cmd]);

        // 未知扩展名（.txt）直接丢弃，不做同名升级
        let txt = root.join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert!(normalize_windows_candidates(vec![txt]).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }
}
