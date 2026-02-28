use thiserror::Error;

#[derive(Error, Debug)]
pub enum AISearchError {
    #[error("配置错误: {0}")]
    Config(String),
    
    #[error("API 错误 (HTTP {code}): {message}")]
    Api { code: u16, message: String },
    
    #[error("网络错误: {message}\n建议: {suggestion}")]
    Network { message: String, suggestion: String },
    
    #[error("协议错误: {0}")]
    Protocol(String),
    
    #[error("JSON 解析错误: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("HTTP 错误: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AISearchError>;
