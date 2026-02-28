// 公共 API 导出，供测试和外部使用

pub mod client;
pub mod config;
pub mod error;
pub mod server;

// 重新导出常用类型
pub use client::AIClient;
pub use config::AIConfig;
pub use error::{AISearchError, Result};
pub use server::MCPServer;
