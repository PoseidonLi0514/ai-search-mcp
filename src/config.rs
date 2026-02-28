use crate::error::{AISearchError, Result};
use serde::{Deserialize, Serialize};
use std::env;

const DEFAULT_TIMEOUT: u64 = 60;
const DEFAULT_RETRY_COUNT: u32 = 1;
const DEFAULT_MAX_QUERY_PLAN: u32 = 1;

const DEFAULT_SYSTEM_PROMPT: &str = r#"你是一个专业的搜索助手,擅长联网搜索并提供准确、详细的答案。

当前时间: {current_time}

搜索策略:
1. 优先使用最新、权威的信息源
2. 对于时间敏感的查询,明确标注信息的时间
3. 提供多个来源的信息进行交叉验证
4. 对于技术问题,优先参考官方文档和最新版本

输出要求:
- 直接回答用户问题
- 时间相关信息必须基于上述当前时间判断"#;

#[derive(Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub api_url: String,
    pub api_key: String,
    pub model_id: String,
    pub analysis_model_id: Option<String>,
    pub system_prompt: String,
    pub timeout: u64,
    pub stream: bool,
    pub filter_thinking: bool,
    pub retry_count: u32,
    pub log_level: String,
    pub max_query_plan: u32,
}

impl std::fmt::Debug for AIConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIConfig")
            .field("api_url", &self.api_url)
            .field("api_key", &"***REDACTED***")
            .field("model_id", &self.model_id)
            .field("analysis_model_id", &self.analysis_model_id)
            .field("system_prompt", &"<omitted>")
            .field("timeout", &self.timeout)
            .field("stream", &self.stream)
            .field("filter_thinking", &self.filter_thinking)
            .field("retry_count", &self.retry_count)
            .field("log_level", &self.log_level)
            .field("max_query_plan", &self.max_query_plan)
            .finish()
    }
}

impl AIConfig {
    pub fn from_env() -> Result<Self> {
        let api_url = env::var("AI_API_URL")
            .map_err(|_| AISearchError::Config("缺少 AI_API_URL 环境变量".into()))?;
        
        let api_key = env::var("AI_API_KEY")
            .map_err(|_| AISearchError::Config("缺少 AI_API_KEY 环境变量".into()))?;
        
        let model_id = env::var("AI_MODEL_ID")
            .map_err(|_| AISearchError::Config("缺少 AI_MODEL_ID 环境变量".into()))?;
        
        let analysis_model_id = env::var("AI_ANALYSIS_MODEL_ID").ok();
        
        let system_prompt = env::var("AI_SYSTEM_PROMPT")
            .unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string());
        
        let timeout = env::var("AI_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT);
        
        let stream = env::var("AI_STREAM")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(true);
        
        let filter_thinking = env::var("AI_FILTER_THINKING")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(true);
        
        let retry_count = env::var("AI_RETRY_COUNT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RETRY_COUNT);
        
        let log_level = env::var("AI_LOG_LEVEL")
            .unwrap_or_else(|_| "INFO".to_string())
            .to_uppercase();
        
        let max_query_plan = env::var("AI_MAX_QUERY_PLAN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_QUERY_PLAN);
        
        let config = Self {
            api_url,
            api_key,
            model_id,
            analysis_model_id,
            system_prompt,
            timeout,
            stream,
            filter_thinking,
            retry_count,
            log_level,
            max_query_plan,
        };
        
        config.validate()?;
        Ok(config)
    }
    
    fn validate(&self) -> Result<()> {
        if !self.api_url.starts_with("http://") && !self.api_url.starts_with("https://") {
            return Err(AISearchError::Config(
                format!("API URL 必须以 http:// 或 https:// 开头: {}", self.api_url)
            ));
        }
        
        if self.timeout < 1 || self.timeout > 300 {
            return Err(AISearchError::Config(
                format!("超时时间必须在 1-300 秒之间: {}", self.timeout)
            ));
        }
        
        Ok(())
    }
}
