//! 内置规则分类器（移植自插件 RuleClassifier）：
//! 路径/标题/摘要关键词打分推断类别，无需联网，结果不持久化。

/// 9 类 × 关键词表（顺序即类别顺序）
fn rules() -> &'static [(&'static str, &'static [&'static str])] {
    static RULES: &[(&str, &[&str])] = &[
        ("项目说明", &[
            "readme", "说明", "introduction", "intro", "overview", "homepage", "项目介绍", "关于项目",
        ]),
        ("设计文档", &[
            "design", "设计", "架构", "architecture", "方案", "scheme", "blueprint", "概要设计",
            "详细设计", "domain", "时序", "er图", "数据模型", "原型",
        ]),
        ("开发文档", &[
            "changelog", "变更", "release", "发布", "deploy", "部署", "install", "安装", "build",
            "构建", "规范", "standard", "coding", "convention", "贡献", "contribute", "开发指南",
            "dev", "debug", "调试", "脚本",
        ]),
        ("接口文档", &[
            "api", "接口", "rest", "graphql", "endpoint", "openapi", "swagger", "proto",
            "protobuf", "grpc", "webhook",
        ]),
        ("教程指南", &[
            "tutorial", "教程", "guide", "指南", "howto", "how-to", "demo", "示例", "example",
            "sample", "walkthrough", "入门", "quickstart", "快速开始", "cookbook", "手册",
        ]),
        ("会议记录", &[
            "meeting", "会议", "纪要", "minutes", "周报", "日报", "月报", "standup", "复盘", "retro",
        ]),
        ("计划任务", &[
            "todo", "待办", "task", "任务", "计划", "plan", "roadmap", "路线", "milestone",
            "里程碑", "backlog", "schedule", "排期", "sprint", "okr",
        ]),
        ("笔记备忘", &[
            "note", "笔记", "备忘", "memo", "journal", "手记", "tips", "经验", "draft", "草稿",
            "scratch", "learn", "学习", "知识库", "摘录",
        ]),
        ("翻译内容", &[
            "translate", "翻译", "translation", "译文", "i18n", "本地化", "localization", "双语",
        ]),
    ];
    RULES
}

/// 分类：路径命中 +2 分、标题/摘要命中 +0.5 分，总分 ≥1 才算命中
pub fn classify(path: &str, title: Option<&str>, excerpt: &str) -> Option<String> {
    let path_lower = path.to_lowercase();
    let text = format!(
        "{} {}",
        title.unwrap_or("").to_lowercase(),
        excerpt.to_lowercase()
    );
    let mut best: Option<&str> = None;
    let mut best_score = 0.0f32;
    for (cat, keywords) in rules() {
        let mut score = 0.0f32;
        for kw in *keywords {
            if hit(&path_lower, kw) {
                score += 2.0;
            }
            if hit(&text, kw) {
                score += 0.5;
            }
        }
        if score > best_score {
            best_score = score;
            best = Some(cat);
        }
    }
    if best_score >= 1.0 {
        best.map(|s| s.to_string())
    } else {
        None
    }
}

/// 包含匹配；ASCII 关键词做词边界检查（避免 rapid 误命中 api）
fn hit(text: &str, kw: &str) -> bool {
    if text.is_empty() || kw.is_empty() {
        return false;
    }
    let ascii = kw.chars().all(|c| c.is_ascii());
    let mut from = 0;
    while let Some(pos) = text[from..].find(kw) {
        let at = from + pos;
        let end = at + kw.len();
        if !ascii {
            return true;
        }
        let b = text.as_bytes();
        let before_ok = at == 0 || !is_word_char(b[at - 1]);
        let after_ok = end >= b.len() || !is_word_char(b[end]);
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

fn is_word_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

/// 提取第一个一级/二级标题（移植 MdText.title）
pub fn md_title(content: &str) -> Option<String> {
    for line in content.split('\n').take(100) {
        let t = line.trim();
        if t.starts_with("# ") || t.starts_with("## ") {
            let v = t.trim_start_matches('#').trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// 压缩空白后的前 max 字符摘要（移植 MdText.excerpt）
pub fn md_excerpt(content: &str, max: usize) -> String {
    let mut flat: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    flat = flat.replace('#', "").replace('*', "").replace('`', "");
    if flat.chars().count() > max {
        flat.chars().take(max).collect()
    } else {
        flat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_classify_basics() {
        assert_eq!(
            classify("docs/api.md", Some("接口参考"), ""),
            Some("接口文档".to_string())
        );
        assert_eq!(
            classify("notes/meeting.md", None, ""),
            Some("会议记录".to_string())
        );
        // 词边界：rapid 不命中 api
        assert_eq!(classify("src/rapid-dev.md", None, "rapid development"), None);
        // 中文关键词直接包含（路径命中 +2 直接达标）
        assert_eq!(
            classify("deploy/上线文档.md", None, ""),
            Some("开发文档".to_string())
        );
        // 同类别两个标题关键词累计 1 分达标
        assert_eq!(
            classify("a.md", Some("部署与安装手册"), ""),
            Some("开发文档".to_string())
        );
    }

    #[test]
    fn title_excerpt() {
        let c = "---\ntitle: x\n---\n\n# 我的标题\n\n正文  内容\n";
        assert_eq!(md_title(c).as_deref(), Some("我的标题"));
        assert_eq!(md_excerpt("abc   def", 10), "abc def");
    }
}
