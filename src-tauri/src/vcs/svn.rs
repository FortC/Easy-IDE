//! SVN 轻量集成（只读）：status / diff / log / 基线变更 —— 全部 shell 调用 svn 命令行。
//! commit/update 等写操作不提供命令，交给 AI Agent 走确认门执行。

use super::{CommitInfo, FileStatus, VcsStatus};
use crate::config;
use crate::toolchain;
use std::path::Path;
use tauri::AppHandle;

/// svn 可执行：settings.svn_path 非空用之 → toolchain 自动探测取绝对路径 → 裸名 "svn" 兜底
fn svn_exe(app: &AppHandle) -> String {
    let settings = config::load_settings(app);
    let prefer = settings.svn_path.trim();
    if !prefer.is_empty() {
        return prefer.to_string();
    }
    let info = toolchain::detect("svn", "");
    if info.found && !info.path.is_empty() {
        return info.path;
    }
    "svn".to_string()
}

/// 执行 svn（stdout 为结果）；启动失败 / 非零退出报错
fn svn_exec(app: &AppHandle, root: &Path, args: &[&str]) -> Result<String, String> {
    let svn = svn_exe(app);
    // .cmd 走 cmd /C、.exe 直接启动；svn_path 配成 Unix 脚本/未知格式时给可读报错（不裸报 os error 193）
    let mut cmd = toolchain::command_for(&svn).map_err(|fmt| {
        format!(
            "SVN 可执行路径「{}」{}，请到 设置 → 工具链 修正（需要 svn.exe / svn.cmd）",
            svn,
            toolchain::describe_format(fmt)
        )
    })?;
    let out = cmd
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("SVN 启动失败：{}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("svn {} 执行失败", args.first().copied().unwrap_or_default())
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// 是否 SVN 工作副本。
///
/// ⚠️ 入参必须是**已解析出的工作副本根目录**（`vcs::root::detect_repo_root` 的结果）。
/// 兼容 1.7+（根单个 `.svn`）与 1.6（每目录一个 `.svn`）—— 后者由
/// `detect_repo_root` 的「取最浅命中」收敛到工作副本根，这里只做存在性判断。
pub fn is_repo(repo_dir: &Path) -> bool {
    repo_dir.join(".svn").exists()
}

/// svn status / diff --summarize 行解析：前 7 列是状态位，路径从第 8 列起。
/// 兼容第 8 列为分隔空格的实现（多剥一个前导空格）。
fn parse_status_line(line: &str) -> Option<(char, &str)> {
    let line = line.trim_end_matches('\r');
    if line.is_empty() {
        return None;
    }
    let st = line.chars().next()?;
    if line.len() < 8 {
        return Some((st, ""));
    }
    let path = &line[7..];
    Some((st, path.strip_prefix(' ').unwrap_or(path)))
}

/// svn 状态字母 → 统一四类：A 新增 / D 删除 / ? 未跟踪 / 其余（M/C/!/~/R…）→ M
fn map_status(st: char) -> &'static str {
    match st {
        'A' => "A",
        'D' => "D",
        '?' => "?",
        _ => "M",
    }
}

/// svn info → (branch, revision)：branch = URL 尾段（trunk / branches/x），revision = r<rev>
fn info(app: &AppHandle, root: &Path) -> (String, String) {
    let Ok(text) = svn_exec(app, root, &["info"]) else {
        return (String::new(), String::new());
    };
    let mut url = String::new();
    let mut rev = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("URL:") {
            url = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Revision:") {
            rev = v.trim().to_string();
        }
    }
    let branch = branch_from_url(&url);
    let revision = if rev.is_empty() { String::new() } else { format!("r{}", rev) };
    (branch, revision)
}

/// URL 尾段：/trunk → "trunk"；/branches/x → "branches/x"；其他取最后一段
fn branch_from_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    if url.is_empty() {
        return String::new();
    }
    if let Some(idx) = url.find("/branches/") {
        return format!("branches/{}", &url[idx + "/branches/".len()..]);
    }
    url.rsplit('/').next().unwrap_or_default().to_string()
}

/// 工作区状态（svn status 解析）。
/// `repo_dir` = 已解析的工作副本根，不是工作区根。
pub fn status(app: &AppHandle, repo_dir: &Path) -> VcsStatus {
    if !is_repo(repo_dir) {
        return VcsStatus {
            kind: "svn".to_string(),
            is_repo: false,
            branch: String::new(),
            revision: String::new(),
            files: Vec::new(),
            repo_root: None,
            nested: false,
        };
    }
    let (branch, revision) = info(app, repo_dir);
    let mut files = Vec::new();
    if let Ok(text) = svn_exec(app, repo_dir, &["status"]) {
        for line in text.lines() {
            if let Some((st, path)) = parse_status_line(line) {
                if path.is_empty() {
                    continue;
                }
                files.push(FileStatus {
                    path: path.to_string(),
                    status: map_status(st).to_string(),
                });
            }
        }
    }
    VcsStatus {
        kind: "svn".to_string(),
        is_repo: true,
        branch,
        revision,
        files,
        repo_root: None,
        nested: false,
    }
}

/// 单文件 unified diff（空输出 = 没有差异内容）
pub fn diff_file(app: &AppHandle, root: &Path, path: &str) -> Result<String, String> {
    svn_exec(app, root, &["diff", path])
}

/// 最近提交（svn log -l N）
pub fn log(app: &AppHandle, root: &Path, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let text = svn_exec(app, root, &["log", "-l", &limit.to_string()])?;
    Ok(parse_log(&text))
}

/// 基线选择：最近 30 个版本号（r<rev>）
pub fn tags(app: &AppHandle, root: &Path) -> Result<Vec<String>, String> {
    Ok(log(app, root, 30)?.into_iter().map(|c| c.hash).collect())
}

/// 基线之后的变更文件（增量包基线）：svn diff --summarize -r <baseline>:HEAD
pub fn changed_since(
    app: &AppHandle,
    root: &Path,
    baseline: &str,
) -> Result<Vec<FileStatus>, String> {
    let range = format!("{}:HEAD", baseline.trim());
    let text = svn_exec(app, root, &["diff", "--summarize", "-r", &range])?;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some((st, path)) = parse_status_line(line) {
            if path.is_empty() {
                continue;
            }
            out.push(FileStatus {
                path: path.to_string(),
                status: map_status(st).to_string(),
            });
        }
    }
    Ok(out)
}

/// svn log 文本解析：`r<rev> | <author> | <date>` 头行 + 空行后首个非空行作 subject
fn parse_log(text: &str) -> Vec<CommitInfo> {
    let mut out: Vec<CommitInfo> = Vec::new();
    let mut cur: Option<CommitInfo> = None;
    let mut body_started = false;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with("--------") {
            continue; // 分隔线
        }
        if let Some((hash, date)) = parse_log_header(line) {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            cur = Some(CommitInfo { hash, subject: String::new(), date });
            body_started = false;
            continue;
        }
        if let Some(c) = cur.as_mut() {
            if c.subject.is_empty() {
                if line.trim().is_empty() {
                    body_started = true;
                } else if body_started {
                    // 空行后首个非空行作 subject（多行消息只取第一行）
                    c.subject = line.trim().to_string();
                }
            }
        }
    }
    if let Some(c) = cur.take() {
        out.push(c);
    }
    out
}

/// 头行：`r123 | alice | 2026-01-01 12:00:00 +0800 (...) | N lines`
fn parse_log_header(line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() < 3 {
        return None;
    }
    let rev = parts[0].trim();
    let digits = rev.strip_prefix('r')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // parts[2] 为日期原文（其后可能还有 "N lines" 字段，不取）
    let date = parts[2].trim().to_string();
    Some((rev.to_string(), date))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_parse_and_map() {
        assert_eq!(parse_status_line("M       src/Main.java"), Some(('M', "src/Main.java")));
        assert_eq!(parse_status_line("A       docs/new.md"), Some(('A', "docs/new.md")));
        assert_eq!(parse_status_line("?       tmp/cache"), Some(('?', "tmp/cache")));
        assert_eq!(parse_status_line("!       lost.txt"), Some(('!', "lost.txt")));
        // 路径含 " -> " 字面量不误切
        assert_eq!(parse_status_line("M       a -> b.md"), Some(('M', "a -> b.md")));
        assert_eq!(map_status('A'), "A");
        assert_eq!(map_status('D'), "D");
        assert_eq!(map_status('?'), "?");
        assert_eq!(map_status('M'), "M");
        assert_eq!(map_status('C'), "M");
        assert_eq!(map_status('!'), "M");
        assert_eq!(map_status('~'), "M");
        assert_eq!(map_status('R'), "M");
    }

    #[test]
    fn log_parse_subject_after_blank() {
        let text = "\
------------------------------------------------------------------------
r3 | zhangsan | 2026-01-01 10:00:00 +0800 (Thu, 01 Jan 2026) | 2 lines

修复登录超时
补充说明第二行
------------------------------------------------------------------------
r2 | lisi | 2025-12-31 09:00:00 +0800 (Wed, 31 Dec 2025) | 1 line

初始化工程
------------------------------------------------------------------------
";
        let log = parse_log(text);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].hash, "r3");
        assert_eq!(log[0].subject, "修复登录超时");
        assert!(log[0].date.starts_with("2026-01-01"));
        assert_eq!(log[1].hash, "r2");
        assert_eq!(log[1].subject, "初始化工程");
    }

    #[test]
    fn branch_from_url_tail() {
        assert_eq!(branch_from_url("https://svn.example.com/repo/trunk"), "trunk");
        assert_eq!(
            branch_from_url("https://svn.example.com/repo/branches/feature-x"),
            "branches/feature-x"
        );
        assert_eq!(branch_from_url(""), "");
    }
}
