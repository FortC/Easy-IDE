//! 配方存取与指纹：.easyide/recipes/<id>.json，版本升级时旧版归档到 history/。
//! 磁盘文件是唯一真相源——配方只是它的衍生物，失效即重校验，绝不盲执行。

use super::model::{Recipe, RECIPE_SCHEMA};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn recipes_dir(root: &Path) -> PathBuf {
    root.join(".easyide").join("recipes")
}

pub fn recipe_id(action: &str, variant: &str) -> String {
    let action = action.trim();
    let variant = variant.trim();
    if variant.is_empty() {
        action.to_string()
    } else {
        format!("{}~{}", action, variant)
    }
}

fn file_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '~' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.json", safe)
}

pub fn load(root: &Path, id: &str) -> Option<Recipe> {
    let p = recipes_dir(root).join(file_name(id));
    std::fs::read_to_string(p)
        .ok()
        .and_then(|t| serde_json::from_str::<Recipe>(&t).ok())
}

/// 保存配方；若已存在且版本升级，旧版归档到 recipes/history/
pub fn save(root: &Path, recipe: &Recipe) -> anyhow::Result<()> {
    let dir = recipes_dir(root);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(file_name(&recipe.id()));
    if let Some(old) = load(root, &recipe.id()) {
        if old.meta.version < recipe.meta.version {
            let hist = dir.join("history");
            std::fs::create_dir_all(&hist)?;
            let _ = std::fs::write(
                hist.join(format!("{}.v{}.json", file_name(&recipe.id()).trim_end_matches(".json"), old.meta.version)),
                serde_json::to_string_pretty(&old)?,
            );
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(recipe)?)?;
    Ok(())
}

pub fn delete(root: &Path, id: &str) {
    let _ = std::fs::remove_file(recipes_dir(root).join(file_name(id)));
}

pub fn list(root: &Path) -> Vec<Recipe> {
    let dir = recipes_dir(root);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "json").unwrap_or(false) {
                if let Ok(text) = std::fs::read_to_string(&p) {
                    if let Ok(r) = serde_json::from_str::<Recipe>(&text) {
                        out.push(r);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.id().cmp(&b.id()));
    out
}

// ---------------- 指纹 ----------------

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// 当前平台标识（配方 preconditions.platform 用）
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// 计算强指纹（相对路径 → sha256）；缺失/不可读的文件不计入
pub fn compute_fingerprint(root: &Path, files: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for rel in files {
        let rel_path = Path::new(rel);
        if rel_path.is_absolute() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(root.join(rel_path)) {
            map.insert(rel.clone(), sha256_hex(&bytes));
        }
    }
    map
}

/// 校验指纹：返回（有效, 变化文件清单）。无指纹 = 不设防（用户手建配方）按有效处理。
pub fn check_fingerprint(root: &Path, recipe: &Recipe) -> (bool, Vec<String>) {
    if recipe.preconditions.fingerprint.is_empty() {
        return (true, Vec::new());
    }
    let mut changed = Vec::new();
    for (rel, want) in &recipe.preconditions.fingerprint {
        let is_same = std::fs::read(root.join(rel))
            .map(|bytes| sha256_hex(&bytes) == *want)
            .unwrap_or(false); // 文件消失也算变化
        if !is_same {
            changed.push(rel.clone());
        }
    }
    changed.sort();
    (changed.is_empty(), changed)
}

/// 平台是否匹配（配方平台为空 = 不限）
pub fn platform_matches(recipe: &Recipe) -> bool {
    recipe.preconditions.platform.is_empty() || recipe.preconditions.platform == platform()
}

/// 快速通道执行后的使用记录（次数/最近使用）
pub fn touch_usage(root: &Path, id: &str) {
    if let Some(mut r) = load(root, id) {
        r.meta.usage_count = r.meta.usage_count.saturating_add(1);
        r.meta.last_used_at = now();
        let _ = save(root, &r);
    }
}

/// 执行结束后的结果记录（退出码/平均耗时）
pub fn record_result(root: &Path, id: &str, code: Option<i32>, duration_ms: u64) {
    if let Some(mut r) = load(root, id) {
        r.meta.last_exit_code = code;
        // 滑动平均：首次直接取本次值
        r.meta.avg_duration_ms = if r.meta.avg_duration_ms == 0 {
            duration_ms
        } else {
            (r.meta.avg_duration_ms * 4 + duration_ms) / 5
        };
        let _ = save(root, &r);
    }
}

pub fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

/// 新建配方骨架（供命令层填充）
pub fn new_recipe(action: &str, variant: &str) -> Recipe {
    Recipe {
        schema: RECIPE_SCHEMA,
        action: action.to_string(),
        variant: variant.to_string(),
        shell: "cmd".into(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ez_recipe_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn recipe_id_and_file_name() {
        assert_eq!(recipe_id("run", ""), "run");
        assert_eq!(recipe_id("run", "web"), "run~web");
        assert_eq!(file_name("run~web"), "run~web.json");
    }

    #[test]
    fn save_load_roundtrip_and_history() {
        let root = tmp_root("roundtrip");
        let mut r = new_recipe("run", "");
        r.command = "echo hi".into();
        r.meta.version = 1;
        save(&root, &r).unwrap();
        let got = load(&root, "run").unwrap();
        assert_eq!(got.command, "echo hi");

        // 版本升级 → 旧版归档
        r.command = "echo hi2".into();
        r.meta.version = 2;
        save(&root, &r).unwrap();
        let hist = recipes_dir(&root).join("history").join("run.v1.json");
        assert!(hist.exists(), "旧版应归档到 history/");
        assert_eq!(load(&root, "run").unwrap().command, "echo hi2");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fingerprint_check_detects_change() {
        let root = tmp_root("fingerprint");
        std::fs::write(root.join("pom.xml"), "<project/>").unwrap();
        let mut r = new_recipe("build", "");
        r.preconditions.fingerprint = compute_fingerprint(&root, &["pom.xml".into(), "missing.xml".into()]);
        // missing.xml 不存在 → 不计入指纹
        assert_eq!(r.preconditions.fingerprint.len(), 1);

        let (valid, changed) = check_fingerprint(&root, &r);
        assert!(valid && changed.is_empty());

        std::fs::write(root.join("pom.xml"), "<project>changed</project>").unwrap();
        let (valid, changed) = check_fingerprint(&root, &r);
        assert!(!valid);
        assert_eq!(changed, vec!["pom.xml".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_fingerprint_is_open() {
        let root = tmp_root("open");
        let r = new_recipe("run", "");
        assert!(check_fingerprint(&root, &r).0);
        let _ = std::fs::remove_dir_all(&root);
    }
}
