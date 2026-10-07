//! 符号提取器：按扩展名走 Java / JS-TS / Python 的行级正则提取。
//! P1 采用正则方案（零依赖、够快）；后续如需精化可换 tree-sitter，接口不变。

use super::model::Symbol;
use regex::Regex;
use std::sync::OnceLock;

/// 提取器支持的代码语言（按扩展名判定，返回语言标识）
pub fn lang_of(path: &str) -> Option<&'static str> {
    let lower = path.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    // 点开头的配置文件（.gitignore 等）rsplit 出来的不是扩展名，直接排除
    if path.rsplit('/').next().map(|n| n.starts_with('.') && !n[1..].contains('.')).unwrap_or(false) {
        return None;
    }
    match ext {
        "java" => Some("java"),
        "js" | "mjs" | "cjs" | "jsx" => Some("js"),
        "ts" | "tsx" | "mts" | "cts" => Some("ts"),
        "py" | "pyw" => Some("py"),
        _ => None,
    }
}

fn re_java_type() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\s*(?:@[\w.]+(?:\([^)]*\))?\s+)*(?:(?:public|private|protected|static|final|abstract|sealed|non-sealed|strictfp)\s+)*(class|interface|enum|record)\s+([A-Za-z_$][\w$]*)",
        )
        .unwrap()
    })
}

fn re_java_method() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\s*(?:(?:public|private|protected|static|final|synchronized|abstract|native|default|strictfp)\s+)*(?:<[^>=]+>\s*)?[\w$.\[\]<>,? ]+?\s+([A-Za-z_$][\w$]*)\s*\([^)]*\)\s*(?:throws\s+[\w$., ]+)?[;{]",
        )
        .unwrap()
    })
}

fn re_js_class() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^\s*(?:export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][\w$]*)")
            .unwrap()
    })
}

fn re_js_interface() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^\s*(?:export\s+)?(?:declare\s+)?(?:interface|type)\s+([A-Za-z_$][\w$]*)")
            .unwrap()
    })
}

fn re_js_function() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s*\*?\s*([A-Za-z_$][\w$]*)",
        )
        .unwrap()
    })
}

fn re_js_arrow() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\s*(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s+)?(?:function\b|\([^)]*\)\s*(?::[^=]+)?=>|[A-Za-z_$][\w$]*\s*=>)",
        )
        .unwrap()
    })
}

fn re_js_method() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\s+(?:(?:public|private|protected|static|readonly|async|get|set|override|abstract)\s+)*([A-Za-z_$][\w$]*)\s*\([^)]*\)\s*(?::\s*[^{]+)?\{",
        )
        .unwrap()
    })
}

fn re_ts_prop() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^\s{2,}(?:(?:public|private|protected)\s+)+(?:readonly\s+)?(?:static\s+)?([A-Za-z_$][\w$]*)\s*[?!]?\s*(?::[^=;]+)?=",
        )
        .unwrap()
    })
}

fn re_py_class() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(\s*)class\s+([A-Za-z_]\w*)").unwrap())
}

fn re_py_def() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(\s*)(?:async\s+)?def\s+([A-Za-z_]\w*)").unwrap())
}

/// 语句关键字：行首命中则跳过（避免把 if/return 等误判为方法定义）
fn is_statement_start(trimmed: &str) -> bool {
    let first = trimmed.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
    matches!(
        first,
        "if" | "for" | "while" | "switch" | "catch" | "return" | "new" | "throw" | "throws"
            | "else" | "do" | "try" | "assert" | "case" | "break" | "continue" | "delete"
            | "await" | "yield" | "function"
    )
}

/// 注释行（Javadoc / 行注释 / Python 注释）
fn is_comment_line(trimmed: &str, lang: &str) -> bool {
    if lang == "py" {
        return trimmed.starts_with('#');
    }
    trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*")
}

fn sym(kind: &str, name: &str, line: usize, container: Option<&str>) -> Symbol {
    Symbol {
        kind: kind.to_string(),
        name: name.to_string(),
        line: line as u32,
        container: container.map(|c| c.to_string()),
    }
}

/// 提取一个文件的符号（lang_of 判定语言后调用）
pub fn extract(path: &str, content: &str) -> Vec<Symbol> {
    match lang_of(path) {
        Some("java") => extract_java(content),
        Some("js") | Some("ts") => extract_js_ts(content),
        Some("py") => extract_python(content),
        _ => vec![],
    }
}

fn extract_java(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    let mut current_class: Option<String> = None;
    for (i, raw) in content.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.is_empty() || is_comment_line(trimmed, "java") {
            continue;
        }
        if let Some(caps) = re_java_type().captures(raw) {
            let kind = &caps[1];
            let name = &caps[2];
            current_class = Some(name.to_string());
            out.push(sym(kind, name, i + 1, None));
            continue;
        }
        if is_statement_start(trimmed) {
            continue;
        }
        if let Some(caps) = re_java_method().captures(raw) {
            let name = &caps[1];
            out.push(sym("method", name, i + 1, current_class.as_deref()));
        }
    }
    out
}

fn extract_js_ts(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    let mut current_type: Option<String> = None;
    for (i, raw) in content.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.is_empty() || is_comment_line(trimmed, "js") {
            continue;
        }
        if let Some(caps) = re_js_class().captures(raw) {
            let name = &caps[1];
            current_type = Some(name.to_string());
            out.push(sym("class", name, i + 1, None));
            continue;
        }
        if let Some(caps) = re_js_interface().captures(raw) {
            let name = &caps[1];
            current_type = Some(name.to_string());
            out.push(sym("interface", name, i + 1, None));
            continue;
        }
        if let Some(caps) = re_js_function().captures(raw) {
            out.push(sym("function", &caps[1], i + 1, None));
            continue;
        }
        if let Some(caps) = re_js_arrow().captures(raw) {
            out.push(sym("function", &caps[1], i + 1, None));
            continue;
        }
        if is_statement_start(trimmed) {
            continue;
        }
        if let Some(caps) = re_js_method().captures(raw) {
            let name = &caps[1];
            out.push(sym("method", name, i + 1, current_type.as_deref()));
            continue;
        }
        if let Some(caps) = re_ts_prop().captures(raw) {
            out.push(sym("field", &caps[1], i + 1, current_type.as_deref()));
        }
    }
    out
}

fn extract_python(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    // (缩进, 类名) 栈：def 的容器 = 缩进更小的最近 class
    let mut class_stack: Vec<(usize, String)> = Vec::new();
    for (i, raw) in content.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.is_empty() || is_comment_line(trimmed, "py") {
            continue;
        }
        if let Some(caps) = re_py_class().captures(raw) {
            let indent = caps[1].len();
            let name = &caps[2];
            while let Some((top, _)) = class_stack.last() {
                if *top >= indent {
                    class_stack.pop();
                } else {
                    break;
                }
            }
            class_stack.push((indent, name.to_string()));
            out.push(sym("class", name, i + 1, None));
            continue;
        }
        if let Some(caps) = re_py_def().captures(raw) {
            let indent = caps[1].len();
            while let Some((top, _)) = class_stack.last() {
                if *top >= indent {
                    class_stack.pop();
                } else {
                    break;
                }
            }
            let container = class_stack.last().map(|(_, n)| n.clone());
            out.push(sym("function", &caps[2], i + 1, container.as_deref()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_extraction() {
        let src = r#"package com.demo;

import java.util.List;

/**
 * 用户服务
 */
public class UserService extends BaseService implements AutoCloseable {
    private final UserRepository repo;
    public static int COUNT = 0;

    @Override
    public void close() {
    }

    public List<User> findUsers(String keyword) {
        return repo.find(keyword);
    }

    private static class Inner {
        void run() { }
    }
}
"#;
        let syms = extract_java(src);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"UserService"));
        assert!(names.contains(&"Inner"));
        assert!(names.contains(&"close"));
        assert!(names.contains(&"findUsers"));
        assert!(names.contains(&"run"));
        let us = syms.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(us.kind, "class");
        assert_eq!(us.line, 8);
        let fu = syms.iter().find(|s| s.name == "findUsers").unwrap();
        assert_eq!(fu.kind, "method");
        assert_eq!(fu.container.as_deref(), Some("UserService"));
        let run = syms.iter().find(|s| s.name == "run").unwrap();
        assert_eq!(run.container.as_deref(), Some("Inner"));
        // 语句行不误判
        assert!(!names.contains(&"if"));
        assert!(!names.contains(&"return"));
    }

    #[test]
    fn js_ts_extraction() {
        let src = r#"import { foo } from "./a";

export class UserMapper {
    private cache: Map<string, number> = new Map();
    public find(id: string): User | null {
        return null;
    }
}

export interface Options {
    debug: boolean;
}

export function main(): void {
    const helper = (x: number) => x + 1;
    for (let i = 0; i < 3; i++) { foo(i); }
}

export type Result = { ok: boolean };

const arrow = async (a: string) => a;
"#;
        let syms = extract_js_ts(src);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"UserMapper"));
        assert!(names.contains(&"Options"));
        assert!(names.contains(&"Result"));
        assert!(names.contains(&"main"));
        assert!(names.contains(&"find"));
        assert!(names.contains(&"arrow"));
        assert!(names.contains(&"cache"));
        // 语句不误判
        assert!(!names.contains(&"foo"));
        let find = syms.iter().find(|s| s.name == "find").unwrap();
        assert_eq!(find.container.as_deref(), Some("UserMapper"));
        let cache = syms.iter().find(|s| s.name == "cache").unwrap();
        assert_eq!(cache.kind, "field");
    }

    #[test]
    fn python_extraction() {
        let src = r#"import os

class Service:
    def __init__(self):
        self.x = 1

    async def run(self, name: str) -> None:
        def inner():
            pass
        return name

def top_level(a, b):
    return a + b
"#;
        let syms = extract_python(src);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Service"));
        assert!(names.contains(&"__init__"));
        assert!(names.contains(&"run"));
        assert!(names.contains(&"inner"));
        assert!(names.contains(&"top_level"));
        let run = syms.iter().find(|s| s.name == "run").unwrap();
        assert_eq!(run.container.as_deref(), Some("Service"));
        let tl = syms.iter().find(|s| s.name == "top_level").unwrap();
        assert!(tl.container.is_none());
    }

    #[test]
    fn lang_detect() {
        assert_eq!(lang_of("src/main/java/A.java"), Some("java"));
        assert_eq!(lang_of("a/b/c.tsx"), Some("ts"));
        assert_eq!(lang_of("x.py"), Some("py"));
        assert_eq!(lang_of(".gitignore"), None);
        assert_eq!(lang_of("pom.xml"), None);
    }
}
