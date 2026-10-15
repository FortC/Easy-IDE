//! 文件树右键「构建命令」：按项目结构探测构建体系（Maven / Gradle / npm），
//! 生成常用目标菜单供用户手动执行，也支持自定义目标参数。
//!
//! 与 AI 执行命令的区别：这里是用户点菜单发起（用户即批准），无需确认门；
//! 但仍过危险命令硬拦截，目标参数走字符白名单（命令经 cmd 拼接执行，
//! shell 元字符一律拒绝），可执行路径复用 toolchain 的格式校验（防 os error 193）。

use crate::commands::agent::spawn_streaming_detached_argv;
use crate::config;
use crate::maven;
use crate::state::AppState;
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, State};

/// 右键目标处探测到的一个构建模块
#[derive(serde::Serialize, Clone, Debug)]
pub struct BuildTarget {
    /// "maven" | "gradle" | "npm"
    pub kind: String,
    /// 模块目录（工作区相对路径，"" = 工作区根）
    pub dir: String,
    /// 构建文件相对路径（菜单副标题）
    pub file: String,
    /// 模块显示名（目录名；根模块为空串，前端回落「工作区根」）
    pub name: String,
    /// npm scripts 名（仅 npm；已过滤不安全字符，maven/gradle 为空）
    pub scripts: Vec<String>,
    /// npm 包管理器 npm | pnpm | yarn（按 lockfile 判定；仅 npm）
    pub pm: String,
}

#[derive(serde::Deserialize)]
pub struct BuildRunArgs {
    /// "maven" | "gradle" | "npm"
    pub kind: String,
    /// 模块目录（工作区相对路径，"" = 根）
    pub dir: String,
    /// 目标参数（如 `install -DskipTests`；npm 非自定义时为 script 名）
    pub goal: String,
    /// true = 用户自由输入（与预置目标同一套字符白名单）
    pub custom: bool,
}

#[derive(serde::Serialize)]
pub struct BuildRunResult {
    pub id: u32,
    pub pid: u32,
    /// 实际执行的命令（控制台展示用，可直接粘回终端）
    pub command: String,
}

/// 沿路径链向上最多收集的 maven 目标数（多层父子聚合够用即可，防菜单过长）
const MAX_MAVEN_TARGETS: usize = 4;

// ---------------- 探测 ----------------

/// 从右键目标（文件/目录/空白=根）出发，沿目录链向上探测构建体系：
/// pom.xml → maven（最近优先，沿链至多 4 层）；build.gradle(.kts) → gradle（只取最近）；
/// package.json → npm（只取最近，附 scripts 与包管理器）。
#[tauri::command]
pub fn build_detect(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<Vec<BuildTarget>, String> {
    let root = super::recipe_cmd::vault_root(&state)?;
    let abs = normalize_inside(&root, path.as_deref().unwrap_or(""))?;
    let start = if abs.is_dir() {
        abs
    } else {
        abs.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone())
    };

    let mut out: Vec<BuildTarget> = Vec::new();
    let mut maven_count = 0usize;
    let mut dir = start.as_path();
    loop {
        let rel = rel_of(&root, dir);
        if maven_count < MAX_MAVEN_TARGETS && dir.join("pom.xml").is_file() {
            maven_count += 1;
            out.push(BuildTarget {
                kind: "maven".into(),
                dir: rel.clone(),
                file: join_rel(&rel, "pom.xml"),
                name: dir_name(dir),
                scripts: Vec::new(),
                pm: String::new(),
            });
        }
        if !out.iter().any(|t| t.kind == "gradle") {
            let gradle_file = if dir.join("build.gradle.kts").is_file() {
                Some("build.gradle.kts")
            } else if dir.join("build.gradle").is_file() {
                Some("build.gradle")
            } else {
                None
            };
            if let Some(f) = gradle_file {
                out.push(BuildTarget {
                    kind: "gradle".into(),
                    dir: rel.clone(),
                    file: join_rel(&rel, f),
                    name: dir_name(dir),
                    scripts: Vec::new(),
                    pm: String::new(),
                });
            }
        }
        if !out.iter().any(|t| t.kind == "npm") && dir.join("package.json").is_file() {
            out.push(BuildTarget {
                kind: "npm".into(),
                dir: rel.clone(),
                file: join_rel(&rel, "package.json"),
                name: dir_name(dir),
                scripts: npm_scripts(dir),
                pm: npm_pm(dir),
            });
        }
        if dir == root.as_path() {
            break;
        }
        match dir.parent() {
            Some(p) if p.starts_with(&root) => dir = p,
            _ => break,
        }
    }
    Ok(out)
}

/// package.json 的 scripts 名（只留单 token 且无 shell 元字符的，按名排序）
fn npm_scripts(dir: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let mut names: Vec<String> = v
        .get("scripts")
        .and_then(|s| s.as_object())
        .map(|m| m.keys().filter(|k| safe_token(k)).cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

/// 包管理器按 lockfile 判定：pnpm-lock.yaml → pnpm，yarn.lock → yarn，其余 npm
fn npm_pm(dir: &Path) -> String {
    if dir.join("pnpm-lock.yaml").is_file() || dir.join("pnpm-workspace.yaml").is_file() {
        "pnpm".into()
    } else if dir.join("yarn.lock").is_file() {
        "yarn".into()
    } else {
        "npm".into()
    }
}

// ---------------- 执行 ----------------

/// 执行一条构建目标：解析工具可执行 → 拼 argv → 危险命令硬拦截 → spawn（立即返回）。
/// 工作目录恒为工作区根，模块用工具自身的目录参数定位（mvn -f / gradle -p / npm --prefix），
/// 这样路径含空格也不会碎。
#[tauri::command]
pub async fn build_goal_run(
    app: AppHandle,
    state: State<'_, AppState>,
    args: BuildRunArgs,
) -> Result<BuildRunResult, String> {
    let root = super::recipe_cmd::vault_root(&state)?;
    let module_abs = normalize_inside(&root, &args.dir)?;
    if !module_abs.is_dir() {
        return Err(format!("模块目录不存在：{}", args.dir));
    }
    let module_rel = rel_of(&root, &module_abs);

    // 设置与工具链环境（含 PATH 注入）一次加载，供工具解析与 spawn 共用
    let bg_app = app.clone();
    let (settings, env) = tauri::async_runtime::spawn_blocking(move || {
        let settings = crate::config::load_settings(&bg_app);
        let env = crate::toolchain::build_env(&settings);
        (settings, env)
    })
    .await
    .map_err(|e| format!("后台任务异常：{}", e))?;

    let (exe, prefix_args) = match args.kind.as_str() {
        "maven" => {
            let mvn = resolve_maven(&module_abs, &root, &settings)?;
            let pom = join_rel(&module_rel, "pom.xml");
            if !module_abs.join("pom.xml").is_file() {
                return Err(format!("{} 下没有 pom.xml", module_rel));
            }
            (mvn, vec!["-f".to_string(), pom])
        }
        "gradle" => {
            let gradle = resolve_gradle(&module_abs, &root);
            if gradle == "gradle" && !module_abs.join("settings.gradle").is_file()
                && !module_abs.join("settings.gradle.kts").is_file()
                && !module_abs.join("build.gradle").is_file()
                && !module_abs.join("build.gradle.kts").is_file()
            {
                return Err("未找到 gradlew 与 Gradle 构建文件".to_string());
            }
            // 根模块即工作目录本身，免 -p；子模块用 -p 定位
            let p = if module_rel.is_empty() {
                Vec::new()
            } else {
                vec!["-p".to_string(), module_rel.clone()]
            };
            (gradle, p)
        }
        "npm" => {
            let pm = npm_pm(&module_abs);
            let p = if module_rel.is_empty() {
                Vec::new()
            } else {
                let flag = match pm.as_str() {
                    "pnpm" => "--dir",
                    "yarn" => "--cwd",
                    _ => "--prefix",
                };
                vec![flag.to_string(), module_rel.clone()]
            };
            (pm, p)
        }
        other => return Err(format!("未知构建体系：{}", other)),
    };

    // 目标参数：npm 预置目标是 script 名（run <script>）；其余为空格分隔的工具参数
    let goal_args: Vec<String> = if args.kind == "npm" && !args.custom {
        if !safe_token(&args.goal) {
            return Err("非法的 script 名".to_string());
        }
        vec!["run".to_string(), args.goal.clone()]
    } else {
        if !safe_args(&args.goal) {
            return Err("目标参数含不允许的字符（仅限字母数字与 - _ . : = / , + @ * [ ] 空格）".to_string());
        }
        args.goal.split_whitespace().map(String::from).collect()
    };
    if goal_args.is_empty() {
        return Err("目标参数为空".to_string());
    }

    let mut argv = prefix_args;
    argv.extend(goal_args);
    let display = display_command(&exe, &argv);
    if let Some(why) = crate::commands::agent::is_dangerous(&display) {
        return Err(format!("已拦截危险命令（{}）：{}", why, display));
    }

    let (id, pid) = spawn_streaming_detached_argv(app, exe, argv, root, env);
    Ok(BuildRunResult { id, pid, command: display })
}

// ---------------- 工具解析 ----------------

/// Maven 可执行：模块目录 wrapper → 工作区根 wrapper → settings/自动探测链（resolve_mvn）。
/// 项目自带 mvnw.cmd 是「真理源」，优先级高于一切外部安装。
fn resolve_maven(
    module: &Path,
    root: &Path,
    settings: &config::AppSettings,
) -> Result<String, String> {
    let windows = cfg!(target_os = "windows");
    for dir in [module, root] {
        if let (Some(w), _) = maven::resolve_wrapper(dir, windows) {
            return Ok(w.to_string_lossy().to_string());
        }
    }
    maven::resolve_mvn(root, settings)
}

/// Gradle 可执行：模块目录 → 工作区根的 gradlew.cmd/.bat/.exe；都没有则回落裸名 gradle
/// （经设置页/系统 PATH 解析，找不到时控制台报「不是内部或外部命令」可见可查）。
fn resolve_gradle(module: &Path, root: &Path) -> String {
    let windows = cfg!(target_os = "windows");
    let names: &[&str] = if windows {
        &["gradlew.cmd", "gradlew.bat", "gradlew.exe"]
    } else {
        &["gradlew"]
    };
    for dir in [module, root] {
        for name in names {
            let w = dir.join(name);
            if w.is_file() {
                return w.to_string_lossy().to_string();
            }
        }
    }
    "gradle".to_string()
}

// ---------------- 路径与安全 ----------------

/// 相对路径 → 工作区内绝对路径；含 `..` 逃逸或越界直接拒绝
fn normalize_inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return Err(format!("非法路径：{}", rel)),
        }
    }
    Ok(out)
}

/// 绝对路径 → 工作区相对路径（`/` 分隔，根返回空串）
fn rel_of(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn join_rel(rel: &str, name: &str) -> String {
    if rel.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", rel, name)
    }
}

fn dir_name(dir: &Path) -> String {
    dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// 单 token 安全字符（script 名 / 模块名等不含空白与 shell 元字符）
fn safe_token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '+' | '@' | '#')
        })
}

/// 目标参数字符白名单：字母数字与常用构建参数符号。
/// 拒绝 cmd 元字符（& | < > ^ ( ) % ! " ' ;）与换行 —— 命令经 cmd /C 拼接执行，
/// 这些字符任一进串就是命令注入。
fn safe_args(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 400
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, ' ' | '-' | '_' | '.' | ':' | '=' | '/' | ',' | '+' | '@' | '*' | '[' | ']')
        })
}

/// 控制台展示命令：带空格的段加引号（可直接粘回终端手敲）
fn display_command(exe: &str, args: &[String]) -> String {
    let mut parts = vec![quote_display(exe)];
    parts.extend(args.iter().map(|a| quote_display(a)));
    parts.join(" ")
}

fn quote_display(s: &str) -> String {
    if s.contains(' ') {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_args_rejects_shell_metachars() {
        assert!(safe_args("install -DskipTests"));
        assert!(safe_args("dependency:tree -pl a,b"));
        assert!(safe_args(":module:build --stacktrace"));
        assert!(!safe_args("install & del /f"));
        assert!(!safe_args("run | more"));
        assert!(!safe_args("a && b"));
        assert!(!safe_args("x > out.txt"));
        assert!(!safe_args("echo %PATH%"));
        assert!(!safe_args("a\"b"));
        assert!(!safe_args(""));
    }

    #[test]
    fn safe_token_single_word_only() {
        assert!(safe_token("dev"));
        assert!(safe_token("build:prod"));
        assert!(!safe_token("run dev"));
        assert!(!safe_token("a&b"));
        assert!(!safe_token(""));
    }

    #[test]
    fn normalize_inside_blocks_escape() {
        let root = Path::new("E:/ws");
        assert_eq!(normalize_inside(root, "a/b").unwrap(), root.join("a").join("b"));
        assert_eq!(normalize_inside(root, "").unwrap(), root.to_path_buf());
        assert!(normalize_inside(root, "../x").is_err());
        assert!(normalize_inside(root, "a/../..").is_err());
    }

    #[test]
    fn rel_of_and_join_rel() {
        let root = Path::new("E:/ws");
        assert_eq!(rel_of(root, &root.join("code").join("m")), "code/m");
        assert_eq!(rel_of(root, root), "");
        assert_eq!(join_rel("", "pom.xml"), "pom.xml");
        assert_eq!(join_rel("code/m", "pom.xml"), "code/m/pom.xml");
    }

    #[test]
    fn display_quotes_spaced_segments() {
        let exe = r"C:\maven home\bin\mvn.cmd".to_string();
        let args = vec!["-f".to_string(), "a b/pom.xml".to_string(), "install".to_string()];
        assert_eq!(
            display_command(&exe, &args),
            "\"C:\\maven home\\bin\\mvn.cmd\" -f \"a b/pom.xml\" install"
        );
    }
}
