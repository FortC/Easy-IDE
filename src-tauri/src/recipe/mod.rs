//! 命令配方系统（V2 核心，见《改造报告与方案V2.md》4.2）：
//! AI 第一次推理出的执行命令沉淀为配方（.easyide/recipes/），后续执行走本地快速通道（零 AI）。
//! 失效判定 = 平台 + 强指纹（构建文件 hash）；失效走「AI 增量确认 / 重推理」（前端编排）。

pub mod execute;
pub mod model;
pub mod saved;
pub mod store;
