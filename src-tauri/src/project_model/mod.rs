//! 项目模型探测（V2-P0，见《改造报告与方案V2.md》4.1）：
//! 打开工作区时探测项目类型/模块/任务槽，供配方卡预填与指纹文件选择。
//! 纯本地文件读取（顶栏构建文件 + 浅层子目录），毫秒级，无缓存（P1 加失效重建）。

use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct ProjectModule {
    pub name: String,
    /// 相对根目录的模块路径（根模块为 ""）
    pub path: String,
    /// maven-spring-boot | maven | gradle | node | cargo | go | python | cmake | make
    pub kind: String,
    pub entry: String,
    pub port: u16,
}

#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct ProjectTask {
    /// run | build | test | deps | clean（debug 由 run 的调试变体承担）
    pub action: String,
    pub module: String,
    pub kind: String,
    /// 探测建议命令（空 = 无法确定，交给 AI 推理）
    pub suggest: String,
}

#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct ProjectModel {
    /// maven-single | maven-multi | gradle | npm | pnpm | yarn | pip | poetry | uv | cargo | go | cmake | mixed | unknown
    pub r#type: String,
    pub languages: Vec<String>,
    pub modules: Vec<ProjectModule>,
    pub tasks: Vec<ProjectTask>,
    /// 指纹文件（构建文件相对路径），配方强指纹从这里取
    pub fingerprint_files: Vec<String>,
    /// 动作 → 首选建议命令（无建议的动作不出现）
    pub suggests: HashMap<String, String>,
    pub detected_at: String,
}

impl ProjectModel {
    /// 该动作的首选建议命令（第一个非空 suggest）
    pub fn best_suggest(&self, action: &str) -> String {
        self.tasks
            .iter()
            .find(|t| t.action == action && !t.suggest.is_empty())
            .map(|t| t.suggest.clone())
            .unwrap_or_default()
    }
}

const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", ".git", ".svn", ".easyide", ".idea", ".vscode",
    "vendor", "venv", ".venv", "__pycache__", "out", "bin", "obj",
];

const BUILD_FILES: &[&str] = &[
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "CMakeLists.txt",
    "Makefile",
];

/// 探测项目模型
pub fn detect(root: &Path) -> ProjectModel {
    let mut model = ProjectModel {
        detected_at: chrono::Local::now().to_rfc3339(),
        ..Default::default()
    };

    // 1. 根目录构建文件
    let root_kinds = detect_kinds(root);
    // 2. 浅层子目录（深度 1-2）里的构建文件 → 模块
    let mut modules: Vec<ProjectModule> = Vec::new();
    if !root_kinds.is_empty() {
        let mut m = ProjectModule {
            name: root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "root".into()),
            path: String::new(),
            kind: root_kinds[0].clone(),
            ..Default::default()
        };
        enrich_module(root, &mut m);
        modules.push(m);
    }
    collect_submodules(root, root, 2, &mut modules);
    modules.truncate(12);

    // 3. 类型判定
    model.r#type = classify_type(root, &root_kinds, &modules);

    // 4. 指纹文件 = 各模块构建文件
    let mut fps: Vec<String> = Vec::new();
    for m in &modules {
        for bf in BUILD_FILES {
            let rel = if m.path.is_empty() {
                bf.to_string()
            } else {
                format!("{}/{}", m.path, bf)
            };
            if root.join(&rel).is_file() && !fps.contains(&rel) {
                fps.push(rel);
            }
        }
    }
    fps.truncate(20);
    model.fingerprint_files = fps;

    // 5. 任务槽
    for m in &modules {
        let dir = if m.path.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&m.path)
        };
        model.tasks.extend(tasks_for(m, &dir));
    }

    // 6. 语言（浅扫扩展名）
    model.languages = scan_languages(root);
    // 7. 动作 → 建议命令映射（配方卡预填用）
    for action in ["run", "build", "test", "deps", "clean"] {
        let s = model.best_suggest(action);
        if !s.is_empty() {
            model.suggests.insert(action.to_string(), s);
        }
    }
    model.modules = modules;
    model
}

/// 识别某目录下的构建类型（可多种）
fn detect_kinds(dir: &Path) -> Vec<String> {
    let mut kinds = Vec::new();
    if dir.join("pom.xml").is_file() {
        kinds.push("maven".into());
    }
    if dir.join("build.gradle").is_file() || dir.join("build.gradle.kts").is_file() {
        kinds.push("gradle".into());
    }
    if dir.join("package.json").is_file() {
        kinds.push("node".into());
    }
    if dir.join("Cargo.toml").is_file() {
        kinds.push("cargo".into());
    }
    if dir.join("go.mod").is_file() {
        kinds.push("go".into());
    }
    if dir.join("pyproject.toml").is_file()
        || dir.join("requirements.txt").is_file()
        || dir.join("setup.py").is_file()
    {
        kinds.push("python".into());
    }
    if dir.join("CMakeLists.txt").is_file() {
        kinds.push("cmake".into());
    } else if dir.join("Makefile").is_file() {
        kinds.push("make".into());
    }
    kinds
}

fn classify_type(root: &Path, root_kinds: &[String], modules: &[ProjectModule]) -> String {
    let mut kinds: Vec<String> = root_kinds.to_vec();
    for m in modules {
        if !m.path.is_empty() && !kinds.contains(&m.kind) {
            kinds.push(m.kind.clone());
        }
    }
    kinds.sort();
    kinds.dedup();
    if kinds.is_empty() {
        return "unknown".into();
    }
    if kinds.len() > 1 {
        return "mixed".into();
    }
    match kinds[0].as_str() {
        "maven" => {
            let multi = modules.iter().filter(|m| m.kind.starts_with("maven")).count() > 1
                || read(root.join("pom.xml")).map(|t| t.contains("<modules>")).unwrap_or(false);
            if multi {
                "maven-multi".into()
            } else {
                "maven-single".into()
            }
        }
        "node" => node_manager(root).into(),
        "python" => {
            if root.join("poetry.lock").is_file() {
                "poetry".into()
            } else if root.join("uv.lock").is_file() {
                "uv".into()
            } else {
                "pip".into()
            }
        }
        other => other.to_string(),
    }
}

fn node_manager(root: &Path) -> &str {
    if root.join("pnpm-lock.yaml").is_file() || root.join("pnpm-workspace.yaml").is_file() {
        "pnpm"
    } else if root.join("yarn.lock").is_file() {
        "yarn"
    } else {
        "npm"
    }
}

/// 深度 1-2 的子目录含构建文件 → 模块（maven-multi 优先按 <module> 清单）
fn collect_submodules(root: &Path, dir: &Path, depth: usize, out: &mut Vec<ProjectModule>) {
    if depth == 0 {
        return;
    }
    // maven 多模块：按 pom 的 <module> 声明走
    if dir.join("pom.xml").is_file() {
        if let Some(text) = read(dir.join("pom.xml")) {
            if let Ok(re) = Regex::new(r"(?s)<modules>.*?</modules>") {
                if let Some(block) = re.find(&text) {
                    let inner = Regex::new(r"(?s)<module>\s*([^<]+?)\s*</module>").unwrap();
                    for cap in inner.captures_iter(block.as_str()) {
                        let rel = cap[1].trim().to_string();
                        let mod_dir = dir.join(&rel);
                        let name = mod_dir
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| rel.clone());
                        let rel_path = path_rel(root, &mod_dir);
                        if out.iter().any(|m| m.path == rel_path) {
                            continue;
                        }
                        let mut m = ProjectModule {
                            name,
                            path: rel_path,
                            kind: "maven".into(),
                            ..Default::default()
                        };
                        enrich_module(&mod_dir, &mut m);
                        out.push(m);
                    }
                }
            }
        }
        return;
    }
    // 通用：子目录里有构建文件
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            let kinds = detect_kinds(&p);
            if !kinds.is_empty() {
                let rel_path = path_rel(root, &p);
                if out.iter().any(|m| m.path == rel_path) {
                    continue;
                }
                let mut m = ProjectModule {
                    name: p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                    path: rel_path,
                    kind: kinds[0].clone(),
                    ..Default::default()
                };
                enrich_module(&p, &mut m);
                out.push(m);
            } else if depth > 1 {
                collect_submodules(root, &p, depth - 1, out);
            }
        }
    }
}

fn path_rel(root: &Path, full: &Path) -> String {
    full.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

/// 模块补充信息：spring-boot 标记 / 入口（package.json name / scripts）
fn enrich_module(dir: &Path, m: &mut ProjectModule) {
    match m.kind.as_str() {
        "maven" => {
            if let Some(text) = read(dir.join("pom.xml")) {
                if text.contains("spring-boot-maven-plugin") {
                    m.kind = "maven-spring-boot".into();
                }
                if let Ok(re) = Regex::new(r"(?s)<artifactId>\s*([^<]+?)\s*</artifactId>") {
                    if let Some(cap) = re.captures(&text) {
                        m.entry = cap[1].trim().to_string();
                    }
                }
            }
        }
        "node" => {
            if let Some(text) = read(dir.join("package.json")) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    m.entry = v
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                }
            }
        }
        _ => {}
    }
}

/// 任务槽 + 建议命令（只有把握十足才给 suggest，其余留空给 AI）
fn tasks_for(m: &ProjectModule, dir: &Path) -> Vec<ProjectTask> {
    let mut out = Vec::new();
    let mut push = |action: &str, suggest: &str| {
        out.push(ProjectTask {
            action: action.into(),
            module: m.path.clone(),
            kind: m.kind.clone(),
            suggest: suggest.into(),
        })
    };
    match m.kind.as_str() {
        "maven-spring-boot" => {
            push("run", "mvn spring-boot:run");
            push("build", "mvn package -DskipTests");
            push("test", "mvn test");
            push("deps", "mvn dependency:resolve");
            push("clean", "mvn clean");
        }
        "maven" => {
            push("run", "");
            push("build", "mvn package -DskipTests");
            push("test", "mvn test");
            push("deps", "mvn dependency:resolve");
            push("clean", "mvn clean");
        }
        "gradle" => {
            let gw = gradle_wrapper(dir);
            push("run", "");
            push("build", &format!("{} build -x test", gw));
            push("test", &format!("{} test", gw));
            push("deps", &format!("{} dependencies", gw));
            push("clean", &format!("{} clean", gw));
        }
        "node" => {
            for (action, suggest) in node_tasks(dir) {
                push(&action, &suggest);
            }
        }
        "cargo" => {
            push("run", "cargo run");
            push("build", "cargo build");
            push("test", "cargo test");
            push("deps", "cargo fetch");
            push("clean", "cargo clean");
        }
        "go" => {
            push("run", "go run .");
            push("build", "go build ./...");
            push("test", "go test ./...");
            push("deps", "go mod download");
            push("clean", "");
        }
        "python" => {
            push("run", "");
            push("build", "");
            push("test", "");
            push("deps", "pip install -r requirements.txt");
            push("clean", "");
        }
        _ => {}
    }
    out
}

fn gradle_wrapper(dir: &Path) -> String {
    // 仅判断模块目录 wrapper（Windows 优先 .bat），命令在 cmd 下可直接执行
    if dir.join("gradlew.bat").is_file() || dir.join("gradlew").is_file() {
        "gradlew".to_string()
    } else {
        "gradle".to_string()
    }
}

/// node 任务：包管理器前缀（按 lockfile）+ package.json scripts
fn node_tasks(dir: &Path) -> Vec<(String, String)> {
    let scripts = read(dir.join("package.json"))
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("scripts").and_then(|s| s.as_object()).cloned())
        .unwrap_or_default();
    let has = |k: &str| scripts.contains_key(k);
    let mgr = if dir.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if dir.join("yarn.lock").is_file() {
        "yarn"
    } else {
        "npm"
    };
    let run_cmd = if has("dev") {
        format!("{} run dev", mgr)
    } else if has("start") {
        format!("{} run start", mgr)
    } else {
        String::new()
    };
    vec![
        ("run".into(), run_cmd),
        ("build".into(), if has("build") { format!("{} run build", mgr) } else { String::new() }),
        ("test".into(), if has("test") { format!("{} test", mgr) } else { String::new() }),
        ("deps".into(), format!("{} install", mgr)),
        ("clean".into(), String::new()),
    ]
}

/// 语言统计：浅扫源码扩展名（跳过重型目录，上限 3000 文件）
fn scan_languages(root: &Path) -> Vec<String> {
    let ext_lang: HashMap<&str, &str> = HashMap::from([
        ("java", "Java"), ("kt", "Kotlin"), ("py", "Python"), ("go", "Go"), ("rs", "Rust"),
        ("ts", "TypeScript"), ("tsx", "TypeScript"), ("js", "JavaScript"), ("jsx", "JavaScript"),
        ("vue", "Vue"), ("html", "HTML"), ("css", "CSS"), ("scss", "SCSS"),
        ("xml", "XML"), ("yml", "YAML"), ("yaml", "YAML"), ("json", "JSON"), ("sql", "SQL"),
        ("sh", "Shell"), ("bat", "Shell"), ("cs", "C#"), ("cpp", "C++"), ("c", "C"), ("h", "C/C++"),
        ("php", "PHP"), ("rb", "Ruby"), ("swift", "Swift"), ("dart", "Dart"),
    ]);
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut visited = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if visited >= 3000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            if visited >= 3000 {
                break;
            }
            let p = e.path();
            let name = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if p.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push(p);
                }
                continue;
            }
            visited += 1;
            if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
                if let Some(lang) = ext_lang.get(ext) {
                    *counts.entry(lang).or_default() += 1;
                }
            }
        }
    }
    let mut langs: Vec<(usize, &str)> = counts.into_iter().map(|(k, v)| (v, k)).collect();
    langs.sort_by(|a, b| b.0.cmp(&a.0));
    langs.truncate(8);
    langs.into_iter().map(|(_, l)| l.to_string()).collect()
}

fn read<P: AsRef<Path>>(p: P) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_project(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("ez_proj_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn detect_npm_project() {
        let root = tmp_project("npm");
        std::fs::write(
            root.join("package.json"),
            r#"{"name":"demo","scripts":{"dev":"vite","build":"vite build","test":"vitest"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: 9").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("main.ts"), "export {};").unwrap();

        let model = detect(&root);
        assert_eq!(model.r#type, "pnpm");
        assert_eq!(model.best_suggest("run"), "pnpm run dev");
        assert_eq!(model.best_suggest("build"), "pnpm run build");
        assert!(model.fingerprint_files.contains(&"package.json".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detect_spring_boot_single() {
        let root = tmp_project("spring");
        let pom = r#"<project><artifactId>demo-app</artifactId>
            <build><plugins><plugin><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build>
            </project>"#;
        std::fs::write(root.join("pom.xml"), pom).unwrap();
        let model = detect(&root);
        assert_eq!(model.r#type, "maven-single");
        assert_eq!(model.best_suggest("run"), "mvn spring-boot:run");
        assert_eq!(model.best_suggest("deps"), "mvn dependency:resolve");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detect_maven_multi_and_mixed() {
        let root = tmp_project("multi");
        std::fs::write(
            root.join("pom.xml"),
            r#"<project><modules><module>server</module><module>common</module></modules></project>"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("server")).unwrap();
        std::fs::write(root.join("server").join("pom.xml"), "<project/>").unwrap();
        std::fs::create_dir_all(root.join("common")).unwrap();
        std::fs::write(root.join("common").join("pom.xml"), "<project/>").unwrap();
        std::fs::write(root.join("package.json"), r#"{"name":"web"}"#).unwrap();

        let model = detect(&root);
        assert_eq!(model.r#type, "mixed"); // maven + node
        assert!(model.modules.len() >= 3, "根 + server + common，实际 {}", model.modules.len());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn detect_unknown_empty_dir() {
        let root = tmp_project("empty");
        let model = detect(&root);
        assert_eq!(model.r#type, "unknown");
        let _ = std::fs::remove_dir_all(&root);
    }
}
