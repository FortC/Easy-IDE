//! 代码索引数据模型（与前端 types.ts 对应，字段保持 snake_case）。

/// 单个符号：类/接口/枚举/记录/方法/函数/字段
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Symbol {
    /// class | interface | enum | record | method | function | field
    pub kind: String,
    pub name: String,
    /// 1 基行号
    pub line: u32,
    /// 所属类型名（类/接口），顶层符号为 None
    pub container: Option<String>,
}

/// 一个代码文件的符号集合（含缓存校验信息）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FileSymbols {
    pub path: String,
    pub mtime: u64,
    pub size: u64,
    pub symbols: Vec<Symbol>,
}

/// 工作区文件清单条目（Ctrl+P 文件检索用）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CodeFileEntry {
    pub name: String,
    pub mtime: u64,
    pub size: u64,
}

/// 符号检索命中
#[derive(serde::Serialize, Clone, Debug)]
pub struct SymbolHit {
    pub kind: String,
    pub name: String,
    pub line: u32,
    pub container: Option<String>,
    pub path: String,
}

/// 文件名检索命中
#[derive(serde::Serialize, Clone, Debug)]
pub struct CodeHit {
    pub path: String,
    pub name: String,
}

/// 全文检索命中
#[derive(serde::Serialize, Clone, Debug)]
pub struct TextHit {
    pub path: String,
    /// 1 基行号
    pub line_no: u32,
    /// 命中行（去首尾空白，截断 200 字符）
    pub text: String,
}
