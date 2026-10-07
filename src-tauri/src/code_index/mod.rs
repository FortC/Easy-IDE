//! 代码索引模块：工作区文本文件清单 + 符号（类/方法/函数）提取与检索。
//! 与 index/（笔记索引）平行，遵循同一套约定：
//! 磁盘是唯一真相源，缓存只是加速器（mtime/size 校验，可随时删除重建）。

pub mod engine;
pub mod extract;
pub mod model;
