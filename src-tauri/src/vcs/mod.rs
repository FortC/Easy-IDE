//! 版本控制统一抽象（Git / SVN，只读集成）：
//! status / diff / log / tag / 基线变更 统一数据模型与分发，git 优先自动探测。
//! commit/push/update 等写操作不提供命令，交给 AI Agent 走确认门执行。

pub mod git;
pub mod root;
pub mod svn;

use root::RepoScope;
use std::path::Path;
use tauri::AppHandle;

/// 已定位的仓库：类型 + 实际仓库根（相对工作区根）
#[derive(Clone, Debug)]
pub struct Resolved {
    pub kind: VcsKind,
    pub scope: RepoScope,
    /// 探测到的仓库根绝对目录（执行 git/svn 命令的 cwd）
    pub dir: std::path::PathBuf,
}

/// 版本控制类型
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VcsKind {
    Git,
    Svn,
    /// 未检测到仓库
    None,
}

impl VcsKind {
    /// 序列化/前端徽标用短名
    pub fn as_str(self) -> &'static str {
        match self {
            VcsKind::Git => "git",
            VcsKind::Svn => "svn",
            VcsKind::None => "none",
        }
    }

    /// 命令层 kind 参数解析："git" / "svn" 强制指定；其余（含空）= 自动探测
    pub fn parse(s: &str) -> Option<VcsKind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "git" => Some(VcsKind::Git),
            "svn" => Some(VcsKind::Svn),
            _ => None,
        }
    }
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct FileStatus {
    pub path: String,
    /// M 修改 / A 新增 / D 删除 / R 重命名 / ? 未跟踪
    pub status: String,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct VcsStatus {
    /// "git" | "svn" | "none"
    pub kind: String,
    pub is_repo: bool,
    /// git 分支名（detached HEAD 显示 HEAD@<短提交号>）；svn 显示 URL 尾段（trunk / branches/x）
    pub branch: String,
    /// git 短提交号；svn 显示 r<版本号>
    pub revision: String,
    pub files: Vec<FileStatus>,
    /// 仓库根相对工作区根的路径（null = 仓库根就是工作区根）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_root: Option<String>,
    /// 仓库根在工作区根的子目录里 —— 前端据此提示「你打开的是子目录」
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub nested: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct CommitInfo {
    /// git 短提交号；svn 显示 r<版本号>
    pub hash: String,
    pub subject: String,
    /// 作者时间（原文）
    pub date: String,
}

/// 探测工作区内的版本控制仓库（按 git 优先排序，可能同时返回 Git 与 Svn）。
/// 已支持向子层级扫描，见`root::detect_repo_root`。
pub fn detect_vcs(root: &Path) -> Vec<VcsKind> {
    let mut out = Vec::new();
    if root::detect_repo_root(root, ".git").is_some() {
        out.push(VcsKind::Git);
    }
    if root::detect_repo_root(root, ".svn").is_some() {
        out.push(VcsKind::Svn);
    }
    out
}

/// 解析生效的仓库：显式指定优先，否则 git 优先自动探测。
/// 与 `resolve_kind` 的区别：本函数同时给出**实际仓库根目录**，
/// 避免下游再各自探测一遍（原来 status/is_repo 二次判定就是这个毛病的来源）。
pub fn resolve_repo(root: &Path, kind: Option<VcsKind>) -> Option<Resolved> {
    let k = match kind {
        Some(k) if k != VcsKind::None => k,
        _ => detect_vcs(root).into_iter().next()?,
    };
    let marker = match k {
        VcsKind::Git => ".git",
        VcsKind::Svn => ".svn",
        VcsKind::None => return None,
    };
    let scope = root::detect_repo_root(root, marker)?;
    let dir = root::resolve_dir(root, &scope);
    Some(Resolved { kind: k, scope, dir })
}

/// 解析实际生效的类型（保留给只需要 kind、不需要路径的调用方）
pub fn resolve_kind(root: &Path, kind: Option<VcsKind>) -> VcsKind {
    resolve_repo(root, kind).map(|r| r.kind).unwrap_or(VcsKind::None)
}

fn not_repo(kind: &str) -> VcsStatus {
    VcsStatus {
        kind: kind.to_string(),
        is_repo: false,
        branch: String::new(),
        revision: String::new(),
        files: Vec::new(),
        repo_root: None,
        nested: false,
    }
}

/// 把仓库相对路径转成工作区相对路径（子目录仓库需补前缀，否则文件树对不上）
fn remap(scope: &RepoScope, files: Vec<FileStatus>) -> Vec<FileStatus> {
    if matches!(scope, RepoScope::AtRoot) {
        return files;
    }
    files
        .into_iter()
        .map(|mut f| {
            f.path = root::to_workspace_path(scope, &f.path);
            f
        })
        .collect()
}

/// 工作区状态（kind 为空自动探测；无仓库返回 is_repo=false）
pub fn status(app: &AppHandle, root: &Path, kind: Option<VcsKind>) -> VcsStatus {
    let Some(r) = resolve_repo(root, kind) else {
        return not_repo("none");
    };
    let files = match r.kind {
        VcsKind::Git => git::status(app, &r.dir),
        VcsKind::Svn => svn::status(app, &r.dir),
        VcsKind::None => not_repo("none"),
    };
    let (branch, revision, raw) = (files.branch, files.revision, files.files);
    let (repo_root, nested) = match &r.scope {
        RepoScope::AtRoot => (None, false),
        RepoScope::Child(rel) => (Some(rel.clone()), true),
    };
    VcsStatus {
        kind: r.kind.as_str().to_string(),
        is_repo: true,
        branch,
        revision,
        files: remap(&r.scope, raw),
        repo_root,
        nested,
    }
}

/// 单文件 unified diff（空结果 = 没有差异内容）
pub fn diff_file(
    app: &AppHandle,
    root: &Path,
    kind: Option<VcsKind>,
    path: &str,
) -> Result<String, String> {
    let r = resolve_repo(root, kind).ok_or("未检测到版本控制（.git / .svn）")?;
    // diff 的路径参数要相对**仓库根**，而调用方给的是工作区相对路径
    let rel = match &r.scope {
        RepoScope::AtRoot => path.to_string(),
        RepoScope::Child(rp) => {
            let prefix = format!("{}/", rp.trim_end_matches('/'));
            path.strip_prefix(&prefix).unwrap_or(path).to_string()
        }
    };
    match r.kind {
        VcsKind::Git => git::diff_file(app, &r.dir, &rel),
        VcsKind::Svn => svn::diff_file(app, &r.dir, &rel),
        VcsKind::None => Err("未检测到版本控制（.git / .svn）".into()),
    }
}

/// 最近提交
pub fn log(
    app: &AppHandle,
    root: &Path,
    kind: Option<VcsKind>,
    limit: usize,
) -> Result<Vec<CommitInfo>, String> {
    let r = resolve_repo(root, kind).ok_or("未检测到版本控制（.git / .svn）")?;
    match r.kind {
        VcsKind::Git => Ok(git::log(app, &r.dir, limit)),
        VcsKind::Svn => svn::log(app, &r.dir, limit),
        VcsKind::None => Err("未检测到版本控制（.git / .svn）".into()),
    }
}

/// 标签 / 基线列表（git = tag 列表；svn = 最近版本号）
pub fn tags(app: &AppHandle, root: &Path, kind: Option<VcsKind>) -> Result<Vec<String>, String> {
    let r = resolve_repo(root, kind).ok_or("未检测到版本控制（.git / .svn）")?;
    match r.kind {
        VcsKind::Git => Ok(git::tags(app, &r.dir)),
        VcsKind::Svn => svn::tags(app, &r.dir),
        VcsKind::None => Err("未检测到版本控制（.git / .svn）".into()),
    }
}

/// 基线之后的变更文件（增量包基线 diff）
pub fn changed_since(
    app: &AppHandle,
    root: &Path,
    kind: Option<VcsKind>,
    baseline: &str,
) -> Result<Vec<FileStatus>, String> {
    let r = resolve_repo(root, kind).ok_or("未检测到版本控制（.git / .svn）")?;
    let files = match r.kind {
        VcsKind::Git => git::changed_since(app, &r.dir, baseline)?,
        VcsKind::Svn => svn::changed_since(app, &r.dir, baseline)?,
        VcsKind::None => return Err("未检测到版本控制（.git / .svn）".into()),
    };
    Ok(remap(&r.scope, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_parse_and_name() {
        assert_eq!(VcsKind::parse("git"), Some(VcsKind::Git));
        assert_eq!(VcsKind::parse("SVN"), Some(VcsKind::Svn));
        assert_eq!(VcsKind::parse(""), None);
        assert_eq!(VcsKind::parse("hg"), None);
        assert_eq!(VcsKind::Git.as_str(), "git");
        assert_eq!(VcsKind::Svn.as_str(), "svn");
        assert_eq!(VcsKind::None.as_str(), "none");
    }

    #[test]
    fn detect_orders_git_first() {
        let root = std::env::temp_dir().join(format!("easyide-vcs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join(".svn")).unwrap();
        assert_eq!(detect_vcs(&root), vec![VcsKind::Git, VcsKind::Svn]);
        assert_eq!(resolve_kind(&root, None), VcsKind::Git);
        assert_eq!(resolve_kind(&root, Some(VcsKind::Svn)), VcsKind::Svn);
        std::fs::remove_dir_all(&root).ok();

        let empty = std::env::temp_dir().join(format!("easyide-vcs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&empty).unwrap();
        assert!(detect_vcs(&empty).is_empty());
        assert_eq!(resolve_kind(&empty, None), VcsKind::None);
        std::fs::remove_dir_all(&empty).ok();
    }

    #[test]
    fn resolves_nested_repo_dir_and_remaps_paths() {
        // 打开 monorepo 根，仓库实际在 packages/web —— 修复前这里直接认不出仓库
        let root = std::env::temp_dir().join(format!("easyide-vcs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("packages/web/.git")).unwrap();

        assert_eq!(detect_vcs(&root), vec![VcsKind::Git]);

        let r = resolve_repo(&root, None).expect("应解析出子目录仓库");
        assert_eq!(r.kind, VcsKind::Git);
        assert_eq!(
            r.scope,
            RepoScope::Child("packages/web".into()),
            "仓库根应是子目录 packages/web"
        );
        assert!(r.dir.ends_with("packages/web"), "命令 cwd 必须落在真实仓库根");

        // 仓库相对路径要补前缀，否则文件树着色对不上
        let files = vec![FileStatus {
            path: "src/App.vue".into(),
            status: "M".into(),
        }];
        let out = remap(&r.scope, files);
        assert_eq!(out[0].path, "packages/web/src/App.vue");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn at_root_scope_leaves_paths_untouched() {
        let root = std::env::temp_dir().join(format!("easyide-vcs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let r = resolve_repo(&root, Some(VcsKind::Git)).unwrap();
        assert_eq!(r.scope, RepoScope::AtRoot);
        let files = vec![FileStatus { path: "a/B.java".into(), status: "M".into() }];
        assert_eq!(remap(&r.scope, files)[0].path, "a/B.java");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn no_repo_yields_none_not_error() {
        let root = std::env::temp_dir().join(format!("easyide-vcs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(resolve_repo(&root, None).is_none());
        // 显式指定也不该 panic
        assert!(resolve_repo(&root, Some(VcsKind::Git)).is_none());
        std::fs::remove_dir_all(&root).ok();
    }
}
