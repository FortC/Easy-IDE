//! B7-B 档：本地模型端点探测。
//!
//! 明确不做「应用内下载 GGUF 权重」：那意味着数百 MB 包体 + 推理引擎绑定，
//! 与桌面 IDE 的骨架定位不符。用户想本地化就跑 Ollama，这里只负责**把端点接进来并告诉用户通没通**。

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 探测结果（前端设置页直接展示）
#[derive(Serialize, Clone, Debug, Default)]
pub struct LocalLlmProbe {
    /// 端点是否可达
    pub reachable: bool,
    /// 端点上可用的模型名列表（Ollama `/api/tags` 或 OpenAI 兼容 `/models`）
    pub models: Vec<String>,
    /// 探测耗时（毫秒），让用户看到「本地就是快」
    pub elapsed_ms: u128,
    /// 失败原因（可达=false 时必有值，直接展示，别让用户猜）
    pub error: String,
}

#[derive(Deserialize)]
struct OllamaTags {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct OpenAiModels {
    #[serde(default)]
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    #[serde(default)]
    id: String,
}

/// 探测一个本地 OpenAI 兼容端点。
///
/// 依次尝试两种路径，因为 Ollama 同时提供 `/v1/models`（OpenAI 兼容）和
/// `/api/tags`（原生），而 llama.cpp server 只有前者；先原生后兼容能少一层路径拼接猜测。
/// 短超时是刻意的：本机端点 2s 不回基本就是没起，用户不该干等。
pub fn probe(base: &str) -> LocalLlmProbe {
    let started = std::time::Instant::now();
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return LocalLlmProbe {
            error: "本地端点地址为空".into(),
            ..Default::default()
        };
    }
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return LocalLlmProbe {
                error: format!("HTTP 客户端创建失败：{}", e),
                ..Default::default()
            }
        }
    };

    // base 通常以 /v1 结尾；Ollama 原生标签接口在 /api/tags，去掉尾部 /v1 再拼
    let root = base.strip_suffix("/v1").unwrap_or(base);

    // ① Ollama 原生
    let native = format!("{}/api/tags", root);
    if let Ok(resp) = client.get(&native).send() {
        if let Ok(t) = resp.json::<OllamaTags>() {
            let models: Vec<String> = t
                .models
                .into_iter()
                .map(|m| m.name)
                .filter(|s| !s.is_empty())
                .collect();
            return LocalLlmProbe {
                reachable: true,
                models,
                elapsed_ms: started.elapsed().as_millis(),
                error: String::new(),
            };
        }
    }

    // ② OpenAI 兼容
    let compat = format!("{}/models", base);
    match client.get(&compat).send() {
        Ok(resp) => {
            let elapsed = started.elapsed().as_millis();
            if let Ok(t) = resp.json::<OpenAiModels>() {
                let models: Vec<String> = t
                    .data
                    .into_iter()
                    .map(|m| m.id)
                    .filter(|s| !s.is_empty())
                    .collect();
                LocalLlmProbe {
                    reachable: true,
                    models,
                    elapsed_ms: elapsed,
                    error: String::new(),
                }
            } else {
                // 端点活着但不是我们认识的格式 —— 明确说出来比"没找到"有用
                LocalLlmProbe {
                    reachable: false,
                    elapsed_ms: elapsed,
                    error: "端点可达，但返回的不是 Ollama / OpenAI 兼容模型列表".into(),
                    ..Default::default()
                }
            }
        }
        Err(e) => LocalLlmProbe {
            error: format!(
                "连不上 {}（{}）。请确认本地模型服务已启动（Ollama 默认端口 11434）",
                base,
                e
            ),
            elapsed_ms: started.elapsed().as_millis(),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_base_reports_error_not_panic() {
        let p = probe("");
        assert!(!p.reachable);
        assert!(!p.error.is_empty());
    }

    #[test]
    fn unreachable_port_reports_hint() {
        // 1 端口必然拒绝连接；只验证不 panic 且给出可读原因，不依赖网络
        let p = probe("http://127.0.0.1:1/v1");
        assert!(!p.reachable);
        assert!(!p.error.is_empty());
    }

    #[test]
    fn deserializes_ollama_tags_shape() {
        let t: OllamaTags = serde_json::from_str(
            r#"{"models":[{"name":"qwen2.5-coder:7b"},{"name":"llama3:8b"}]}"#,
        )
        .unwrap();
        let names: Vec<&str> = t.models.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["qwen2.5-coder:7b", "llama3:8b"]);
    }

    #[test]
    fn deserializes_openai_models_shape() {
        let t: OpenAiModels =
            serde_json::from_str(r#"{"data":[{"id":"local-model"},{"id":"other"}]}"#).unwrap();
        assert_eq!(t.data.len(), 2);
        assert_eq!(t.data[0].id, "local-model");
    }

    #[test]
    fn tolerates_missing_fields() {
        // 端点返回空对象时不该崩，应退化成空模型列表
        let t: OllamaTags = serde_json::from_str("{}").unwrap();
        assert!(t.models.is_empty());
        let t2: OpenAiModels = serde_json::from_str("{}").unwrap();
        assert!(t2.data.is_empty());
    }
}
