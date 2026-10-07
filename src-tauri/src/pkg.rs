//! 增量包：git 基线 diff → 源码/资源变更 → target/classes 产物映射 → 按模板（classes/war/fat-jar）打 zip。
//! 历史记录存 .easyide/packages.json。

use crate::gitops;
use std::path::Path;
use tauri::AppHandle;

#[derive(serde::Serialize, Clone, Debug)]
pub struct PkgResult {
    /// zip 绝对路径
    pub zip_path: String,
    /// 打进包里的条目（模板前缀后的路径）
    pub entries: Vec<String>,
    /// 有变更但无法映射的文件（如 pom.xml —— 提示需要完整构建）
    pub skipped: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PkgHistoryItem {
    pub time: String,
    pub baseline: String,
    pub template: String,
    pub zip: String,
    pub count: usize,
}

/// 变更路径分类结果
enum ChangedKind {
    /// java 源码 → target/classes 候选
    Java(String),
    /// 资源文件（resources/webapp）→ classes 直拷
    Resource(String),
    /// 无法映射
    Other,
}

/// 单个变更路径的映射（纯函数，便于测试）
/// 返回 (target/classes 内相对路径列表的候选前缀, 模板内相对路径)
fn classify_changed(path: &str) -> ChangedKind {
    let p = path.replace('\\', "/");
    if let Some(idx) = p.find("/src/main/java/") {
        let rel = &p[idx + "/src/main/java/".len()..];
        if rel.ends_with(".java") {
            return ChangedKind::Java(p[..idx].to_string());
        }
    }
    if p.starts_with("src/main/java/") && p.ends_with(".java") {
        return ChangedKind::Java(String::new());
    }
    for prefix in ["/src/main/resources/", "/src/main/webapp/"] {
        if let Some(idx) = p.find(prefix) {
            return ChangedKind::Resource(p[idx + prefix.len()..].to_string());
        }
    }
    if p.starts_with("src/main/resources/") || p.starts_with("src/main/webapp/") {
        let rel = p
            .trim_start_matches("src/main/resources/")
            .trim_start_matches("src/main/webapp/");
        return ChangedKind::Resource(rel.to_string());
    }
    ChangedKind::Other
}

/// java 相对路径 → 产物相对路径（含内部类收集）
fn java_outputs(classes_dir: &Path, java_rel: &str) -> Vec<String> {
    let base = java_rel.trim_end_matches(".java");
    let mut out = vec![format!("{}.class", base.replace('\\', "/"))];
    // 内部类 Foo$1.class / Foo$Bar.class（不匹配 Foo$BarBaz 这种外部同前缀类——需以 $ 后内容无 / 为界，这里按前缀+后缀宽松收集再过滤）
    if let Some(parent) = classes_dir.join(base).parent() {
        let stem = base.rsplit('/').next().unwrap_or(base);
        if let Ok(entries) = std::fs::read_dir(parent) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{}$", stem)) && name.ends_with(".class") {
                    let rel = e
                        .path()
                        .strip_prefix(classes_dir)
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .unwrap_or(name);
                    if !out.contains(&rel) {
                        out.push(rel);
                    }
                }
            }
        }
    }
    out
}

fn template_prefix(template: &str) -> &'static str {
    match template {
        "war" => "WEB-INF/classes/",
        "fatjar" => "BOOT-INF/classes/",
        _ => "",
    }
}

/// 构建增量包
pub fn build(app: &AppHandle, root: &Path, baseline: &str, template: &str) -> Result<PkgResult, String> {
    let changed = gitops::changed_since(app, root, baseline)?;
    if changed.is_empty() {
        return Err("基线之后没有任何变更".into());
    }
    let prefix = template_prefix(template);
    let mut entries: Vec<String> = Vec::new(); // zip 内名称
    let mut files: Vec<(std::path::PathBuf, String)> = Vec::new(); // (磁盘绝对路径, zip 名称)
    let mut skipped: Vec<String> = Vec::new();

    for f in &changed {
        match classify_changed(&f.path) {
            ChangedKind::Java(module) => {
                let java_rel = if module.is_empty() {
                    f.path.trim_start_matches("src/main/java/").to_string()
                } else {
                    let idx = f.path.find("/src/main/java/").unwrap_or(0);
                    f.path[idx + "/src/main/java/".len()..].to_string()
                };
                let classes_dir = if module.is_empty() {
                    root.join("target").join("classes")
                } else {
                    root.join(&module).join("target").join("classes")
                };
                if !classes_dir.exists() {
                    skipped.push(format!("{}（{} 不存在，请先构建）", f.path, classes_dir.display()));
                    continue;
                }
                for out in java_outputs(&classes_dir, &java_rel) {
                    let full = classes_dir.join(&out);
                    if full.exists() {
                        let name = format!("{}{}", prefix, out);
                        if !entries.contains(&name) {
                            entries.push(name.clone());
                            files.push((full, name));
                        }
                    } else {
                        skipped.push(format!("{}（产物缺失）", f.path));
                    }
                }
            }
            ChangedKind::Resource(rel) => {
                // 优先取 target/classes 下的资源副本；webapp 资源直拷源文件
                let from_classes = root.join("target").join("classes").join(&rel);
                let source = root.join(&f.path);
                let full = if from_classes.exists() {
                    from_classes
                } else if source.exists() {
                    source
                } else {
                    skipped.push(format!("{}（源文件缺失）", f.path));
                    continue;
                };
                let name = format!("{}{}", prefix, rel);
                if !entries.contains(&name) {
                    entries.push(name.clone());
                    files.push((full, name));
                }
            }
            ChangedKind::Other => {
                skipped.push(format!("{}（构建配置或不可映射文件，建议完整打包）", f.path));
            }
        }
    }

    if files.is_empty() {
        return Err(format!("没有可打包的产物（变更 {} 个，全部被跳过）", changed.len()));
    }

    // 打 zip 到项目根
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let zip_path = root.join(format!("增量包-{}.zip", stamp));
    let zf = std::fs::File::create(&zip_path).map_err(|e| format!("创建 ZIP 失败：{}", e))?;
    let mut zip = zip::ZipWriter::new(zf);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (full, name) in &files {
        let bytes = std::fs::read(full).map_err(|e| format!("读取 {} 失败：{}", name, e))?;
        zip.start_file(name.clone(), opts)
            .map_err(|e| format!("写入 ZIP 失败：{}", e))?;
        std::io::Write::write_all(&mut zip, &bytes)
            .map_err(|e| format!("写入 ZIP 失败：{}", e))?;
    }
    zip.finish().map_err(|e| format!("完成 ZIP 失败：{}", e))?;

    let result = PkgResult {
        zip_path: zip_path.to_string_lossy().to_string(),
        entries,
        skipped,
    };
    append_history(root, &result, baseline, template);
    Ok(result)
}

fn history_path(root: &Path) -> std::path::PathBuf {
    root.join(".easyide").join("packages.json")
}

fn append_history(root: &Path, result: &PkgResult, baseline: &str, template: &str) {
    let path = history_path(root);
    let mut list: Vec<PkgHistoryItem> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    list.push(PkgHistoryItem {
        time: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        baseline: baseline.to_string(),
        template: template.to_string(),
        zip: result.zip_path.clone(),
        count: result.entries.len(),
    });
    if list.len() > 30 {
        let drain = list.len() - 30;
        list.drain(0..drain);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn history(root: &Path) -> Vec<PkgHistoryItem> {
    std::fs::read_to_string(history_path(root))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_paths() {
        match classify_changed("src/main/java/com/a/Foo.java") {
            ChangedKind::Java(m) => assert_eq!(m, ""),
            _ => panic!("应识别为 java"),
        }
        match classify_changed("mod1/src/main/java/com/a/Foo.java") {
            ChangedKind::Java(m) => assert_eq!(m, "mod1"),
            _ => panic!("应识别为模块 java"),
        }
        match classify_changed("src/main/resources/app.yml") {
            ChangedKind::Resource(r) => assert_eq!(r, "app.yml"),
            _ => panic!("应识别为资源"),
        }
        match classify_changed("web/src/main/webapp/index.html") {
            ChangedKind::Resource(r) => assert_eq!(r, "index.html"),
            _ => panic!("webapp 资源"),
        }
        assert!(matches!(classify_changed("pom.xml"), ChangedKind::Other));
        assert!(matches!(classify_changed("README.md"), ChangedKind::Other));
    }

    #[test]
    fn inner_classes_collected() {
        let root = std::env::temp_dir().join(format!("easyide-pkg-{}", uuid::Uuid::new_v4()));
        let classes = root.join("target").join("classes").join("com").join("demo");
        std::fs::create_dir_all(&classes).unwrap();
        std::fs::write(classes.join("Foo.class"), b"x").unwrap();
        std::fs::write(classes.join("Foo$1.class"), b"x").unwrap();
        std::fs::write(classes.join("Foo$Bar.class"), b"x").unwrap();
        std::fs::write(classes.join("FooBar.class"), b"x").unwrap(); // 不应收集
        let outs = java_outputs(&root.join("target").join("classes"), "com/demo/Foo.java");
        assert!(outs.contains(&"com/demo/Foo.class".to_string()));
        assert!(outs.contains(&"com/demo/Foo$1.class".to_string()));
        assert!(outs.contains(&"com/demo/Foo$Bar.class".to_string()));
        assert!(!outs.iter().any(|o| o.contains("FooBar")));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn template_prefixes() {
        assert_eq!(template_prefix("war"), "WEB-INF/classes/");
        assert_eq!(template_prefix("fatjar"), "BOOT-INF/classes/");
        assert_eq!(template_prefix("classes"), "");
    }
}
