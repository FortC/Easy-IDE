//! 代码索引引擎：工作区文本文件清单 + 符号索引 + 并行全文搜索。
//! 与 index::engine 相同的缓存策略：mtime/size 校验增量，缓存可随时删除重建。

use super::extract;
use super::model::{CodeFileEntry, CodeHit, FileSymbols, Symbol, SymbolHit, TextHit};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const CODE_CACHE_VERSION: u32 = 1;

/// 符号提取的文件大小上限（过大的生成文件不值得解析）
const MAX_SYMBOL_FILE_SIZE: u64 = 2 * 1024 * 1024;
/// 全文搜索的文件大小上限
const MAX_SEARCH_FILE_SIZE: u64 = 2 * 1024 * 1024;
/// 全文搜索默认结果上限
const DEFAULT_MAX_HITS: usize = 500;

/// 索引/搜索时跳过的目录（编译产物、依赖、版本库、应用私有目录）
pub fn is_excluded_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".easyide" | "node_modules" | "target" | "build" | "dist" | "out"
            | ".gradle" | "vendor" | "bower_components" | "__pycache__" | ".venv" | "venv"
            | ".next" | ".nuxt" | ".cache" | "cmake-build-debug" | "cmake-build-release"
            | "$RECYCLE.BIN" | "System Volume Information"
    )
}

/// 参与文件清单/全文搜索的文本扩展名（含 md；点开头的配置文件按文件名识别）
fn is_text_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    if let Some(stripped) = lower.strip_prefix('.') {
        // .gitignore / .env / .editorconfig 等：文件名本身以点开头
        if !stripped.contains('.') {
            return true;
        }
    }
    let ext = lower.rsplit('.').next().unwrap_or("");
    matches!(
        ext,
        "md" | "markdown" | "java" | "xml" | "json" | "js" | "mjs" | "cjs" | "jsx" | "ts"
            | "tsx" | "mts" | "py" | "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "html" | "htm"
            | "vue" | "svelte" | "css" | "scss" | "less" | "sql" | "yml" | "yaml" | "toml"
            | "properties" | "ini" | "conf" | "cfg" | "sh" | "bash" | "zsh" | "txt" | "log"
            | "gradle" | "kts" | "bat" | "cmd" | "csv"
    )
}

fn rel_path(root: &Path, full: &Path) -> String {
    full.strip_prefix(root)
        .unwrap_or(full)
        .to_string_lossy()
        .replace('\\', "/")
}

fn mtime_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Default)]
pub struct CodeIndexEngine {
    /// 工作区全部文本文件：rel path -> 条目（Ctrl+P 与全文搜索的候选集）
    pub files: BTreeMap<String, CodeFileEntry>,
    /// 代码文件符号：rel path -> 符号集合
    pub symbols: BTreeMap<String, FileSymbols>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CodeCacheFile {
    version: u32,
    files: Vec<(String, CodeFileEntry)>,
    symbols: Vec<FileSymbols>,
}

fn walk_files(root: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_excluded_dir(&e.file_name().to_string_lossy()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
}

impl CodeIndexEngine {
    /// 全量扫描（无缓存时的冷启动路径；当前主路径为 refresh_against_disk，保留为工具 API）
    #[allow(dead_code)]
    pub fn scan(root: &Path) -> Result<Self> {
        let mut engine = CodeIndexEngine::default();
        for entry in walk_files(root) {
            let rel = rel_path(root, entry.path());
            engine.upsert_file(&rel, entry.path(), entry.metadata().ok());
        }
        Ok(engine)
    }

    /// 打开工作区时的增量校验：缓存命中且 mtime/size 一致则跳过解析；
    /// 同时同步 files 清单（新增/删除/变更）
    pub fn refresh_against_disk(&mut self, root: &Path) {
        let mut disk_files = std::collections::HashSet::new();
        for entry in walk_files(root) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_text_file(&name) {
                continue;
            }
            let rel = rel_path(root, entry.path());
            disk_files.insert(rel.clone());
            let meta = entry.metadata().ok();
            let (mtime, size) = meta
                .as_ref()
                .map(|m| (mtime_of(m), m.len()))
                .unwrap_or((0, 0));
            let unchanged = self.files.get(&rel).map(|f| f.mtime == mtime && f.size == size);
            let sym_unchanged = self.symbols.get(&rel).map(|f| f.mtime == mtime && f.size == size);
            if unchanged != Some(true) {
                self.upsert_file(&rel, entry.path(), meta);
            } else if sym_unchanged != Some(true) && extract::lang_of(&rel).is_some() {
                // 文件清单没变但符号缺失/过期（例如缓存版本升级）
                self.upsert_file(&rel, entry.path(), meta);
            }
        }
        self.files.retain(|k, _| disk_files.contains(k));
        self.symbols.retain(|k, _| disk_files.contains(k));
    }

    /// 读取并索引单个文件（清单 + 符号）；文件不可读则仅记录清单
    fn upsert_file(&mut self, rel: &str, full: &Path, meta: Option<std::fs::Metadata>) {
        let meta = match meta.or_else(|| std::fs::metadata(full).ok()) {
            Some(m) => m,
            None => {
                self.files.remove(rel);
                self.symbols.remove(rel);
                return;
            }
        };
        let name = full
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.to_string());
        let mtime = mtime_of(&meta);
        let size = meta.len();
        self.files.insert(
            rel.to_string(),
            CodeFileEntry {
                name,
                mtime,
                size,
            },
        );
        if extract::lang_of(rel).is_some() && size <= MAX_SYMBOL_FILE_SIZE {
            if let Ok(text) = std::fs::read_to_string(full) {
                let syms = extract::extract(rel, &text);
                self.symbols.insert(
                    rel.to_string(),
                    FileSymbols {
                        path: rel.to_string(),
                        mtime,
                        size,
                        symbols: syms,
                    },
                );
            }
        }
    }

    /// 单文件更新（监听器触发）：返回是否有变化
    pub fn update_file(&mut self, root: &Path, rel: &str) -> bool {
        let full = root.join(rel);
        if !full.exists() {
            let had = self.files.remove(rel).is_some() | self.symbols.remove(rel).is_some();
            return had;
        }
        let name = full
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !is_text_file(&name) {
            return false;
        }
        let meta = std::fs::metadata(&full).ok();
        let (mtime, size) = meta
            .as_ref()
            .map(|m| (mtime_of(m), m.len()))
            .unwrap_or((0, 0));
        let unchanged = self
            .files
            .get(rel)
            .map(|f| f.mtime == mtime && f.size == size)
            .unwrap_or(false);
        if unchanged {
            return false;
        }
        self.upsert_file(rel, &full, meta);
        true
    }

    /// 该路径是否归代码索引管（watcher 分流用）
    pub fn is_trackable(&self, rel: &str) -> bool {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        is_text_file(name)
    }

    pub fn save_cache(&self, cache_path: &Path) {
        let cache = CodeCacheFile {
            version: CODE_CACHE_VERSION,
            files: self.files.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            symbols: self.symbols.values().cloned().collect(),
        };
        if let Ok(json) = serde_json::to_string(&cache) {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let tmp = cache_path.with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, cache_path);
            }
        }
    }

    pub fn load_cache(cache_path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(cache_path).ok()?;
        let cache: CodeCacheFile = serde_json::from_str(&text).ok()?;
        if cache.version != CODE_CACHE_VERSION {
            return None;
        }
        Some(CodeIndexEngine {
            files: cache.files.into_iter().collect(),
            symbols: cache
                .symbols
                .into_iter()
                .map(|f| (f.path.clone(), f))
                .collect(),
        })
    }

    pub fn cache_path(config_dir: &Path, vault_root: &Path) -> std::path::PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(vault_root.to_string_lossy().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        config_dir.join("cache").join("code-index").join(format!("{}.json", &hash[..24]))
    }

    /// 符号检索：名称匹配（大小写不敏感），前缀 > 词边界 > 包含；可按 kind 过滤
    pub fn search_symbols(
        &self,
        query: &str,
        kinds: Option<&[String]>,
        limit: usize,
    ) -> Vec<SymbolHit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        let mut hits: Vec<(u8, SymbolHit)> = Vec::new();
        for (path, fs) in &self.symbols {
            for s in &fs.symbols {
                if let Some(ks) = kinds {
                    if !ks.iter().any(|k| k == &s.kind) {
                        continue;
                    }
                }
                let name_lower = s.name.to_lowercase();
                let rank = if name_lower.starts_with(&q) {
                    0
                } else if word_boundary_contains(&s.name, &name_lower, &q) {
                    1
                } else if name_lower.contains(&q) {
                    2
                } else {
                    continue;
                };
                hits.push((
                    rank,
                    SymbolHit {
                        kind: s.kind.clone(),
                        name: s.name.clone(),
                        line: s.line,
                        container: s.container.clone(),
                        path: path.clone(),
                    },
                ));
            }
        }
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.name.len().cmp(&b.1.name.len()))
                .then(a.1.path.cmp(&b.1.path))
        });
        hits.into_iter().take(limit).map(|(_, h)| h).collect()
    }

    /// 文件名检索：名称优先（前缀 > 包含），路径包含兜底
    pub fn search_files(&self, query: &str, limit: usize) -> Vec<CodeHit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        let mut hits: Vec<(u8, CodeHit)> = Vec::new();
        for (path, entry) in &self.files {
            let name_lower = entry.name.to_lowercase();
            let stem_lower = name_lower.split('.').next().unwrap_or(&name_lower);
            let rank = if stem_lower.starts_with(&q) {
                0
            } else if name_lower.contains(&q) {
                1
            } else if path.to_lowercase().contains(&q) {
                2
            } else {
                continue;
            };
            hits.push((
                rank,
                CodeHit {
                    path: path.clone(),
                    name: entry.name.clone(),
                },
            ));
        }
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.name.len().cmp(&b.1.name.len()))
                .then(a.1.path.cmp(&b.1.path))
        });
        hits.into_iter().take(limit).map(|(_, h)| h).collect()
    }

    /// 单文件符号（大纲面板）
    pub fn file_symbols(&self, path: &str) -> Vec<Symbol> {
        self.symbols
            .get(path)
            .map(|f| f.symbols.clone())
            .unwrap_or_default()
    }

    /// 全文搜索的候选文件（≤ 上限大小）
    pub fn search_candidates(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|(_, e)| e.size <= MAX_SEARCH_FILE_SIZE)
            .map(|(k, _)| k.clone())
            .collect()
    }
}

/// 词边界包含：query 出现在 camelHump 或 _/$ 分词边界之后（如 "service" 命中 "UserService"）
/// name 为原始大小写，name_lower 为其小写形式，q 已小写
fn word_boundary_contains(name: &str, name_lower: &str, q: &str) -> bool {
    if q.is_empty() {
        return false;
    }
    let orig = name.as_bytes();
    let mut start = 0;
    while let Some(pos) = name_lower[start..].find(q) {
        let at = start + pos;
        let boundary_before = at == 0
            || orig.get(at - 1) == Some(&b'_')
            || orig.get(at - 1) == Some(&b'$')
            || (orig.get(at - 1).is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                && orig.get(at).is_some_and(|c| c.is_ascii_uppercase()));
        if boundary_before {
            return true;
        }
        start = at + 1;
    }
    false
}

/// 并行全文搜索：候选文件跨线程分片，全局命中数封顶
pub fn parallel_text_search(
    root: &Path,
    candidates: &[String],
    query: &str,
    case_sensitive: bool,
    max_hits: usize,
) -> Vec<TextHit> {
    if candidates.is_empty() || query.is_empty() {
        return vec![];
    }
    let max_hits = if max_hits == 0 { DEFAULT_MAX_HITS } else { max_hits };
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(candidates.len());
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    let total = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    // 闭包按引用共享（&AtomicUsize 是 Copy，可被多个线程闭包捕获）
    let total = &total;
    let stop = &stop;

    // 跨步分片：文件列表按路径排序，跨步取数比连续切块负载更均衡
    let mut all: Vec<TextHit> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|tid| {
                let files: Vec<&String> = candidates
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % threads == tid)
                    .map(|(_, f)| f)
                    .collect();
                let root = root.to_path_buf();
                let needle = needle.clone();
                s.spawn(move || -> Vec<TextHit> {
                    let mut out = Vec::new();
                    for rel in files {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let full = root.join(rel);
                        let Ok(bytes) = std::fs::read(&full) else { continue };
                        let text = String::from_utf8_lossy(&bytes);
                        for (i, line) in text.lines().enumerate() {
                            let hit = if case_sensitive {
                                line.contains(&needle)
                            } else {
                                line.to_lowercase().contains(&needle)
                            };
                            if hit {
                                let trimmed = line.trim();
                                let text_out: String = if trimmed.chars().count() > 200 {
                                    trimmed.chars().take(200).collect()
                                } else {
                                    trimmed.to_string()
                                };
                                out.push(TextHit {
                                    path: rel.to_string(),
                                    line_no: (i + 1) as u32,
                                    text: text_out,
                                });
                                if total.fetch_add(1, Ordering::Relaxed) + 1 >= max_hits {
                                    stop.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    all.sort_by(|a, b| a.path.cmp(&b.path).then(a.line_no.cmp(&b.line_no)));
    all.truncate(max_hits);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 临时目录里搭一个迷你项目，跑通 扫描→排除→符号检索→文件检索→全文搜索 全链路
    #[test]
    fn scan_search_full_chain() {
        let root = std::env::temp_dir().join(format!("easyide-idx-{}", uuid::Uuid::new_v4()));
        let src = root.join("src").join("main");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::write(
            src.join("UserService.java"),
            "package com.demo;
public class UserService {
    public void resetAll() { }
}
",
        )
        .unwrap();
        std::fs::write(src.join("helper.ts"), "export const calc = (a: number) => a * 2;
").unwrap();
        // 应被排除的目录
        std::fs::write(root.join("node_modules").join("skip.js"), "class ShouldNotAppear {}
").unwrap();
        // 全文搜索目标（含中文）
        std::fs::write(src.join("notes.md"), "这里有一个独特的锚点词长颈鹿
").unwrap();

        let mut engine = CodeIndexEngine::scan(&root).unwrap();

        // 文件清单：包含目标文件，排除 node_modules
        assert!(engine.files.contains_key("src/main/UserService.java"));
        assert!(engine.files.contains_key("src/main/helper.ts"));
        assert!(engine.files.contains_key("src/main/notes.md"));
        assert!(!engine.files.contains_key("node_modules/skip.js"));

        // 符号：类与方法
        let hits = engine.search_symbols("userservice", None, 10);
        assert!(hits.iter().any(|h| h.name == "UserService" && h.kind == "class"));
        let hits = engine.search_symbols("resetall", None, 10);
        assert!(hits.iter().any(|h| h.name == "resetAll" && h.kind == "method"));
        let hits = engine.search_symbols("shouldnotappear", None, 10);
        assert!(hits.is_empty(), "排除目录内的符号不应出现");

        // 驼峰词边界：calc 命中
        let hits = engine.search_symbols("calc", None, 10);
        assert!(hits.iter().any(|h| h.name == "calc"));

        // 文件检索
        let hits = engine.search_files("userservice", 10);
        assert!(hits.iter().any(|h| h.path == "src/main/UserService.java"));
        let hits = engine.search_files("skip", 10);
        assert!(hits.is_empty());

        // 全文搜索（并行线程池）
        let candidates = engine.search_candidates();
        let hits = parallel_text_search(&root, &candidates, "长颈鹿", false, 100);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "src/main/notes.md");
        assert_eq!(hits[0].line_no, 1);

        // 缓存 round-trip
        let cache = root.join("cache.json");
        engine.save_cache(&cache);
        let loaded = CodeIndexEngine::load_cache(&cache).unwrap();
        assert_eq!(loaded.files.len(), engine.files.len());
        assert!(loaded.symbols.contains_key("src/main/UserService.java"));

        // 增量更新：改一个文件 → 只该文件重解析；删除 → 移除
        std::fs::write(src.join("helper.ts"), "export function compute() { return 1; }
").unwrap();
        assert!(engine.update_file(&root, "src/main/helper.ts"));
        assert!(!engine.update_file(&root, "src/main/helper.ts"), "无变化不应触发");
        std::fs::remove_file(src.join("helper.ts")).unwrap();
        assert!(engine.update_file(&root, "src/main/helper.ts"));
        let fresh = CodeIndexEngine::scan(&root).unwrap();
        assert!(!fresh.files.contains_key("src/main/helper.ts"));

        std::fs::remove_dir_all(&root).ok();
    }
}
