//! 仓库根解析：工作区根 ≠ 仓库根（B3）
//!
//! 现状问题：`git::is_repo` / `svn::is_repo` 只判断 `root.join(".git").exists()`，
//! 打开 monorepo 的子目录（如 `packages/web`）时直接返回 false，版本控制面板全废。
//!
//! 本模块提供向下 BFS 定位真实仓库根。
//!
//! ⚠️ 安全边界（用户明确要求）：**只向下探测，绝不向上越过工作区根**。
//! 工作区是用户授权的沙箱，向上访问会读到项目外的文件，属于越权。
//! 因此「仓库根在父层」这种场景一律放弃 —— 打开子模块时应提示用户改开仓库根。
//!
//! SVN 兼容两种布局：
//!  - 1.7+：工作副本根只有一个 `.svn`，子目录没有
//!  - 1.6  ：工作副本根**每个子目录**都有 `.svn`
//! 两者都靠「取最浅命中」自然收敛到工作副本根，无需版本嗅探。

use std::path::{Path, PathBuf};

/// 向下探测的最大深度。`.git` / `.svn` 目录几乎不会更深，4 层足够覆盖
/// 「工作区根 → 单层子目录」这类绝大多数真实场景，同时给递归开销封顶。
const MAX_DEPTH: usize = 4;

/// 探测到的仓库位置
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepoScope {
    /// 仓库根就在工作区根本身（热路径，毫秒级）
    AtRoot,
    /// 仓库根在某个子目录（相对工作区根的路径）
    Child(String),
}

/// 遍历时跳过的目录：与 `code_index::engine::is_excluded_dir` 保持同源语义，
/// 避免「索引跳过但仓库探测要进」的不一致。另含 `.svn`（SVN 1.6 的 `.svn`
/// 是元数据目录，不该被当业务目录下钻）。
fn is_excluded_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".svn" | ".easyide" | "node_modules" | "target" | "build" | "dist" | "out"
            | ".gradle" | "vendor" | "bower_components" | "__pycache__" | ".venv" | "venv"
            | ".next" | ".nuxt" | ".cache" | "cmake-build-debug" | "cmake-build-release"
            | ".idea" | ".vscode" | "$RECYCLE.BIN" | "System Volume Information"
    )
}

/// `marker` 存在性判定。git worktree / submodule 场景下 `.git` 是**文件**而非目录，
/// 所以不能用 `is_dir()`；`.exists()` 同时兼容两种。
fn has_marker(dir: &Path, marker: &str) -> bool {
    dir.join(marker).exists()
}

/// 向下 BFS 找仓库根，取**最浅**命中（并列则按路径字典序取首个，保证结果确定）。
///
/// 返回 `RepoScope`。`None` = 工作区内未找到任何仓库。
pub fn detect_repo_root(root: &Path, marker: &str) -> Option<RepoScope> {
    // 热路径：根目录自身就是仓库根，不做任何遍历（绝大多数项目的快路径）
    if has_marker(root, marker) {
        return Some(RepoScope::AtRoot);
    }
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut best: Option<(usize, PathBuf)> = None;
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> =
        std::collections::VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        if depth >= MAX_DEPTH {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            // 权限不足 / 已被删除：跳过，不影响其他分支
            Err(_) => continue,
        };
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            // 只下钻目录；符号链接不跟随（防环 + 防止逃出工作区）
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_excluded_dir(&name) {
                continue;
            }
            children.push(path);
        }
        // 排序保证「同级并列时取字典序首个」，跨平台结果一致
        children.sort();
        for child in children {
            if has_marker(&child, marker) {
                let d = depth + 1;
                let better = match &best {
                    Some((bd, _)) => d < *bd,
                    None => true,
                };
                if better {
                    best = Some((d, child.clone()));
                }
                // 命中后不再下钻该子树（更深的仓库嵌套极罕见，且会让结果依赖遍历顺序）
                continue;
            }
            queue.push_back((child, depth + 1));
        }
    }

    let (_, abs) = best?;
    // 统一用 canonical 路径做前缀剥离：Windows 下盘符大小写/短名会让 strip_prefix 失配
    let rel = abs
        .canonicalize()
        .unwrap_or_else(|_| abs.clone())
        .strip_prefix(&root_canon)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    if rel.is_empty() {
        Some(RepoScope::AtRoot)
    } else {
        Some(RepoScope::Child(rel))
    }
}

/// 把 `RepoScope` 解析成实际执行命令的目录。
/// `AtRoot` 返回原 root（保持引用，不做多余 IO）。
pub fn resolve_dir(root: &Path, scope: &RepoScope) -> PathBuf {
    match scope {
        RepoScope::AtRoot => root.to_path_buf(),
        RepoScope::Child(rel) => root.join(rel),
    }
}

/// 路径重映射：仓库命令返回的路径是**仓库根相对**，需要转成**工作区根相对**
/// 才能和文件树 / 文件列表对上（`stores/vault.ts` 的 vcsFiles 着色依赖这个）。
///
/// `Child(rel)` 场景下仓库根比工作区根深，文件树里的 `a/b.java` 在 git status
/// 里可能是 `b.java`（若 `a/` 就是仓库根），必须补前缀。
pub fn to_workspace_path(scope: &RepoScope, repo_rel: &str) -> String {
    match scope {
        RepoScope::AtRoot => repo_rel.replace('\\', "/"),
        RepoScope::Child(rel) => {
            let p = repo_rel.replace('\\', "/");
            let p = p.trim_start_matches("./");
            format!("{}/{}", rel, p).replace("//", "/")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("easyide-repo-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn at_root_is_fast_path() {
        let root = tmp();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        assert_eq!(detect_repo_root(&root, ".git"), Some(RepoScope::AtRoot));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn finds_child_repo() {
        let root = tmp();
        std::fs::create_dir_all(root.join("packages/web/.git")).unwrap();
        assert_eq!(
            detect_repo_root(&root, ".git"),
            Some(RepoScope::Child("packages/web".into()))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn picks_shallowest_when_nested() {
        let root = tmp();
        // 外层命中：不应被内层更深的命中盖掉
        std::fs::create_dir_all(root.join("a/.git")).unwrap();
        std::fs::create_dir_all(root.join("a/b/.git")).unwrap();
        assert_eq!(detect_repo_root(&root, ".git"), Some(RepoScope::Child("a".into())));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_worktree_marker_is_file() {
        let root = tmp();
        std::fs::create_dir_all(root.join("svc")).unwrap();
        std::fs::write(root.join("svc/.git"), "gitdir: ../.git/worktrees/svc").unwrap();
        assert_eq!(
            detect_repo_root(&root, ".git"),
            Some(RepoScope::Child("svc".into()))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn svn16_per_dir_marker_resolves_to_shallowest() {
        // SVN 1.6：每个子目录都有 .svn。若工作区根没有 .svn 而子目录有，
        // 说明工作区根本身不是工作副本根，最浅命中才是 —— 这正是 BFS 该收敛的结果。
        let root = tmp();
        std::fs::create_dir_all(root.join("wc/src/main/.svn")).unwrap();
        std::fs::create_dir_all(root.join("wc/src/main/java/.svn")).unwrap();
        assert_eq!(
            detect_repo_root(&root, ".svn"),
            Some(RepoScope::Child("wc/src/main".into())),
            "应取最浅的 .svn，而非更深的 java 子目录"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn svn17_single_root_marker() {
        // SVN 1.7+：只有工作副本根有 .svn，走 AtRoot 快路径
        let root = tmp();
        std::fs::create_dir_all(root.join(".svn")).unwrap();
        assert_eq!(detect_repo_root(&root, ".svn"), Some(RepoScope::AtRoot));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skips_excluded_dirs() {
        let root = tmp();
        std::fs::create_dir_all(root.join("target/.git")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg/.git")).unwrap();
        assert_eq!(detect_repo_root(&root, ".git"), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn respects_depth_cap() {
        let root = tmp();
        // 5 层深的仓库应被MAX_DEPTH=4 拦掉
        let deep = root.join("a/b/c/d/e/.git");
        std::fs::create_dir_all(deep).unwrap();
        assert_eq!(detect_repo_root(&root, ".git"), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn workspace_path_remap() {
        assert_eq!(to_workspace_path(&RepoScope::AtRoot, "a/B.java"), "a/B.java");
        assert_eq!(
            to_workspace_path(&RepoScope::Child("pkg/web".into()), "B.java"),
            "pkg/web/B.java"
        );
        assert_eq!(
            to_workspace_path(&RepoScope::Child("pkg/web".into()), "./B.java"),
            "pkg/web/B.java"
        );
    }

    #[test]
    fn resolve_dir_joins_child() {
        let root = Path::new("C:/ws");
        assert_eq!(resolve_dir(root, &RepoScope::AtRoot), Path::new("C:/ws"));
        assert_eq!(
            resolve_dir(root, &RepoScope::Child("pkg".into())),
            Path::new("C:/ws/pkg")
        );
    }
}