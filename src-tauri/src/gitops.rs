//! Git 轻量集成：status / diff / log / tag —— 全部 shell 调用 git 命令行，不引 libgit2。
//! commit/push 等写操作不提供命令，交给 AI Agent 走确认门执行。

use crate::config;
use std::path::Path;
use std::process::Command;
use tauri::AppHandle;

#[derive(serde::Serialize, Clone, Debug)]
pub struct FileStatus {
    pub path: String,
    /// M 修改 / A 新增 / D 删除 / R 重命名 / ? 未跟踪
    pub status: String,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: String,
    pub files: Vec<FileStatus>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct CommitInfo {
    pub hash: String,
    pub subject: String,
    /// 作者时间（原文）
    pub date: String,
}

fn git_exec(app: &AppHandle, root: &Path, args: &[&str]) -> Result<String, String> {
    let settings = config::load_settings(app);
    let git = if settings.git_path.trim().is_empty() {
        "git".to_string()
    } else {
        settings.git_path.trim().to_string()
    };
    let out = Command::new(&git)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git 启动失败：{}", e))?;
    if !out.status.success() {
        // 非 repo（"not a git repository"）等场景向上层返回错误文本
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// 是否 git 仓库
pub fn is_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// 工作区状态（porcelain 解析）
pub fn status(app: &AppHandle, root: &Path) -> GitStatus {
    if !is_repo(root) {
        return GitStatus { is_repo: false, branch: String::new(), files: vec![] };
    }
    let branch = git_exec(app, root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let mut files = Vec::new();
    if let Ok(text) = git_exec(app, root, &["status", "--porcelain"]) {
        for line in text.lines() {
            if line.len() < 4 {
                continue;
            }
            let xy = &line[..2];
            let path = line[3..].trim().trim_matches('"').to_string();
            // 重命名 "old -> new"：取新路径
            let path = path.rsplit(" -> ").next().unwrap_or(&path).to_string();
            let status = if xy.ends_with("?") {
                "?".to_string()
            } else {
                let c = xy.chars().rev().find(|c| *c != ' ').unwrap_or('M');
                c.to_string()
            };
            files.push(FileStatus { path, status });
        }
    }
    GitStatus { is_repo: true, branch, files }
}

/// 单文件 unified diff
pub fn diff_file(app: &AppHandle, root: &Path, path: &str) -> Result<String, String> {
    git_exec(app, root, &["diff", "--", path]).and_then(|d| {
        if d.trim().is_empty() {
            // 未跟踪文件：diff against /dev/null
            git_exec(app, root, &["diff", "--no-index", "--", "NUL", path])
                .or_else(|_| Ok(String::new()))
        } else {
            Ok(d)
        }
    })
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
pub fn changed_since(app: &AppHandle, root: &Path, baseline: &str) -> Result<Vec<FileStatus>, String> {
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
            if line.len() < 4 {
                continue;
            }
            let xy = &line[..2];
            let path = line[3..].trim().trim_matches('"').to_string();
            let path = path.rsplit(" -> ").next().unwrap_or(&path).to_string();
            let status = if xy.ends_with("?") { "?" } else { "M" };
            if !out.iter().any(|f| f.path == path) {
                out.push(FileStatus { path, status: status.to_string() });
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
}
