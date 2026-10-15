//! Git 轻量集成：status / diff / log / tag —— 全部 shell 调用 git 命令行，不引 libgit2。
//! commit/push 等写操作不提供命令，交给 AI Agent 走确认门执行。

use super::{CommitInfo, FileStatus, VcsStatus};
use crate::config;
use crate::toolchain;
use std::path::Path;
use tauri::AppHandle;

/// git 可执行：settings.git_path 非空用之 → toolchain 自动探测取绝对路径 → 裸名 "git" 兜底
fn git_exe(app: &AppHandle) -> String {
    let settings = config::load_settings(app);
    let prefer = settings.git_path.trim();
    if !prefer.is_empty() {
        return prefer.to_string();
    }
    let info = toolchain::detect("git", "");
    if info.found && !info.path.is_empty() {
        return info.path;
    }
    "git".to_string()
}

/// 执行 git（stdout 为结果）；非零退出向上层返回 stderr 文本
fn git_exec(app: &AppHandle, root: &Path, args: &[&str]) -> Result<String, String> {
    let (code, stdout, stderr) = git_raw(app, root, args)?;
    if code != 0 {
        // 非 repo（"not a git repository"）等场景向上层返回错误文本
        return Err(stderr.trim().to_string());
    }
    Ok(stdout)
}

/// 执行 git 返回 (退出码, stdout, stderr)，仅启动失败报错。
/// `git diff --no-index` 等场景退出码 1 = "有差异"，是有效结果，由调用方自行判定。
fn git_raw(app: &AppHandle, root: &Path, args: &[&str]) -> Result<(i32, String, String), String> {
    let git = git_exe(app);
    // .cmd 走 cmd /C、.exe 直接启动；git_path 配成 Unix 脚本/未知格式时给可读报错（不裸报 os error 193）
    let mut cmd = toolchain::command_for(&git).map_err(|fmt| {
        format!(
            "Git 可执行路径「{}」{}，请到 设置 → 工具链 修正（需要 git.exe / git.cmd）",
            git,
            toolchain::describe_format(fmt)
        )
    })?;
    let out = cmd
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git 启动失败：{}", e))?;
    Ok((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

/// 是否 git 仓库。
///
/// ⚠️ 入参必须是**已解析出的仓库根目录**（`vcs::root::detect_repo_root` 的结果），
/// 而不是工作区根 —— 打开 monorepo 子目录时两者不同。
/// `.git` 在 worktree/submodule 场景下是文件，故用 `exists()` 而非 `is_dir()`。
pub fn is_repo(repo_dir: &Path) -> bool {
    repo_dir.join(".git").exists()
}

/// porcelain 行解析：XY 状态列 + 路径。
/// 重命名/复制（状态列含 R）按 " -> " 第一次出现分割取后段（新路径）；
/// 普通路径即使含 " -> " 字面量也不切分。
fn parse_porcelain_line(line: &str) -> Option<FileStatus> {
    if line.len() < 4 {
        return None;
    }
    let xy = &line[..2];
    let raw = line[3..].trim();
    let path = if xy.contains('R') {
        match raw.split_once(" -> ") {
            Some((_, new)) => new.trim().trim_matches('"').to_string(),
            None => raw.trim_matches('"').to_string(),
        }
    } else {
        raw.trim_matches('"').to_string()
    };
    let status = if xy.ends_with('?') {
        "?".to_string()
    } else {
        let c = xy.chars().rev().find(|c| *c != ' ').unwrap_or('M');
        c.to_string()
    };
    Some(FileStatus { path, status })
}

/// 当前分支：detached HEAD（abbrev-ref 得 "HEAD"）时显示 `HEAD@<短提交号>`
pub fn branch(app: &AppHandle, root: &Path) -> String {
    let name = git_exec(app, root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if name == "HEAD" {
        let rev = revision(app, root);
        if rev.is_empty() {
            "HEAD".to_string()
        } else {
            format!("HEAD@{}", rev)
        }
    } else {
        name
    }
}

/// 短提交号（失败为空）
pub fn revision(app: &AppHandle, root: &Path) -> String {
    git_exec(app, root, &["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// 工作区状态（porcelain 解析）。
/// `repo_dir` = 已解析的仓库根（`vcs::resolve_repo` 给出），不是工作区根。
pub fn status(app: &AppHandle, repo_dir: &Path) -> VcsStatus {
    if !is_repo(repo_dir) {
        return VcsStatus {
            kind: "git".to_string(),
            is_repo: false,
            branch: String::new(),
            revision: String::new(),
            files: Vec::new(),
            repo_root: None,
            nested: false,
        };
    }
    let branch = branch(app, repo_dir);
    let revision = revision(app, repo_dir);
    let mut files = Vec::new();
    if let Ok(text) = git_exec(app, repo_dir, &["status", "--porcelain"]) {
        for line in text.lines() {
            if let Some(f) = parse_porcelain_line(line) {
                files.push(f);
            }
        }
    }
    VcsStatus {
        kind: "git".to_string(),
        is_repo: true,
        branch,
        revision,
        files,
        repo_root: None,
        nested: false,
    }
}

/// 单文件 unified diff
pub fn diff_file(app: &AppHandle, root: &Path, path: &str) -> Result<String, String> {
    let d = git_exec(app, root, &["diff", "--", path])?;
    if !d.trim().is_empty() {
        return Ok(d);
    }
    // 未跟踪文件：diff against /dev/null（Windows 用 NUL）。
    // `git diff --no-index` 退出码 1 表示"发现差异"，是有效 diff，不能当错误吞掉。
    let (code, stdout, _stderr) = git_raw(app, root, &["diff", "--no-index", "--", "NUL", path])?;
    match code {
        0 | 1 => Ok(stdout),
        // NUL 不可用等异常：保持"没有差异内容"语义
        _ => Ok(String::new()),
    }
}

/// 最近提交
pub fn log(app: &AppHandle, root: &Path, limit: usize) -> Vec<CommitInfo> {
    let mut out = Vec::new();
    let Ok(text) = git_exec(
        app,
        root,
        &["log", "-n", &limit.to_string(), "--pretty=%h%x09%ad%x09%s", "--date=short"],
    ) else {
        return out;
    };
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        if let (Some(hash), Some(date), Some(subject)) = (parts.next(), parts.next(), parts.next()) {
            out.push(CommitInfo {
                hash: hash.to_string(),
                date: date.to_string(),
                subject: subject.to_string(),
            });
        }
    }
    out
}

/// 标签列表
pub fn tags(app: &AppHandle, root: &Path) -> Vec<String> {
    git_exec(app, root, &["tag", "--sort=-creatordate"])
        .map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

/// 变更文件（增量包基线）：baseline..HEAD + 未提交修改
pub fn changed_since(
    app: &AppHandle,
    root: &Path,
    baseline: &str,
) -> Result<Vec<FileStatus>, String> {
    let mut out: Vec<FileStatus> = Vec::new();
    let text = git_exec(app, root, &["diff", "--name-status", baseline, "HEAD"])?;
    for line in text.lines() {
        let mut parts = line.split('\t');
        let Some(st) = parts.next() else { continue };
        let status = st.chars().next().unwrap_or('M').to_string();
        // R100\told\tnew：取新路径
        let paths: Vec<&str> = parts.collect();
        let path = paths.last().copied().unwrap_or_default().to_string();
        if !path.is_empty() {
            out.push(FileStatus { path, status });
        }
    }
    // 工作区未提交（M/A，不含删除）
    if let Ok(wt) = git_exec(app, root, &["status", "--porcelain"]) {
        for line in wt.lines() {
            if let Some(f) = parse_porcelain_line(line) {
                let status = if f.status == "?" { "?" } else { "M" };
                if !out.iter().any(|x| x.path == f.path) {
                    out.push(FileStatus { path: f.path, status: status.to_string() });
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_line_parse_shape() {
        // 解析逻辑内联在 status()，此处验证字段抽取的字符串约定
        let line = "M  src/A.java";
        assert_eq!(&line[..2], "M ");
        assert_eq!(line[3..].trim(), "src/A.java");
        let untracked = "?? docs/new.md";
        assert!(untracked[..2].ends_with('?'));
        let renamed = "R  old/x.md -> new/x.md";
        let path = renamed[3..].trim().rsplit(" -> ").next().unwrap();
        assert_eq!(path, "new/x.md");
    }

    #[test]
    fn porcelain_rename_split_only_for_renamed() {
        let renamed = parse_porcelain_line("R  old/x.md -> new/x.md").unwrap();
        assert_eq!(renamed.path, "new/x.md");
        assert_eq!(renamed.status, "R");
        // 普通路径含 " -> " 字面量不再误切
        let plain = parse_porcelain_line("M  docs/a -> b.md").unwrap();
        assert_eq!(plain.path, "docs/a -> b.md");
        assert_eq!(plain.status, "M");
        let untracked = parse_porcelain_line("?? docs/new.md").unwrap();
        assert_eq!(untracked.status, "?");
        assert_eq!(untracked.path, "docs/new.md");
        // 带引号的重命名：取引号内的新路径
        let quoted = parse_porcelain_line("R  \"old name.md\" -> \"new name.md\"").unwrap();
        assert_eq!(quoted.path, "new name.md");
    }
}
