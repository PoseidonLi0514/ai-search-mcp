use crate::config::AIConfig;
use crate::error::{AISearchError, Result};
use chrono::Local;
use futures::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};

static THINKING_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)<think(?:ing)?>.*?</think(?:ing)?>").unwrap()
});

static WHITESPACE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\n\s*\n").unwrap()
});

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<Message>,
    delta: Option<Delta>,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Clone)]
pub struct AIClient {
    config: AIConfig,
    client: Client,
}

impl AIClient {
    pub fn new(config: AIConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()?;
        
        Ok(Self { config, client })
    }
    
    pub async fn search(&self, query: &str) -> Result<String> {
        let is_sub_query = query.starts_with("[SUB_QUERY]");
        
        if is_sub_query {
            let actual_query = query.trim_start_matches("[SUB_QUERY]").trim();
            info!("子查询直接搜索: {}", actual_query);
            return self.call_api(actual_query).await;
        }
        
        if self.config.max_query_plan > 1 {
            info!("多维度搜索: 并发执行 {} 个子查询", self.config.max_query_plan);
            
            // 1. 拆分查询
            let sub_queries = self.split_query(query, self.config.max_query_plan).await?;
            info!("拆分完成: {:?}", sub_queries);
            
            // 2. 并发执行所有子查询
            info!("开始并发执行 {} 个子查询", sub_queries.len());
            let start_time = std::time::Instant::now();
            
            // 预先创建所有任务句柄，确保同时启动
            let mut search_futures = Vec::with_capacity(sub_queries.len());
            for (i, sub_query) in sub_queries.iter().enumerate() {
                let query = sub_query.clone();
                let client = self.clone();
                let task = tokio::spawn(async move {
                    let result = client.search_internal(&query).await;
                    result
                });
                info!("已启动子查询 {}", i + 1);
                search_futures.push(task);
            }
            
            let results: Vec<Result<String>> = futures::future::join_all(search_futures)
                .await
                .into_iter()
                .map(|r| r.unwrap_or_else(|e| Err(AISearchError::Network {
                    message: format!("任务执行失败: {}", e),
                    suggestion: "请重试".into(),
                })))
                .collect();
            let elapsed = start_time.elapsed();
            
            let success_count = results.iter().filter(|r| r.is_ok()).count();
            let fail_count = results.iter().filter(|r| r.is_err()).count();
            info!("并发执行完成: 成功 {}, 失败 {}, 总耗时 {:?}", success_count, fail_count, elapsed);
            
            // 3. 直接返回所有结果（不整合）
            let mut output = String::new();
            for (i, result) in results.into_iter().enumerate() {
                // 提取子问题（去掉 [SUB_QUERY] 前缀）
                let sub_question = sub_queries.get(i)
                    .map(|q| q.trim_start_matches("[SUB_QUERY]").trim())
                    .unwrap_or("未知");
                
                match result {
                    Ok(content) => {
                        output.push_str(&format!("## 子查询 {} 结果\n\n**子问题**: {}\n\n{}\n\n", i + 1, sub_question, content));
                    }
                    Err(e) => {
                        error!("子查询 {} 失败 (查询: {}): {}", i + 1, sub_question, e);
                        output.push_str(&format!("## 子查询 {} 失败\n\n**子问题**: {}\n\n**错误**: {}\n\n", i + 1, sub_question, e));
                    }
                }
            }
            
            if output.is_empty() {
                return Err(AISearchError::Protocol("所有子查询都失败了".into()));
            }
            
            return Ok(output);
        }
        
        info!("直接搜索: {}", query);
        self.call_api(query).await
    }
    
    /// 内部搜索方法，用于递归调用
    async fn search_internal(&self, query: &str) -> Result<String> {
        let is_sub_query = query.starts_with("[SUB_QUERY]");
        
        if is_sub_query {
            let actual_query = query.trim_start_matches("[SUB_QUERY]").trim();
            info!("子查询直接搜索: {}", actual_query);
            return self.call_api(actual_query).await;
        }
        
        info!("直接搜索: {}", query);
        self.call_api(query).await
    }
    
    /// 使用自定义系统提示词调用 API
    async fn call_api_with_custom_prompt(&self, query: &str, custom_prompt: &str) -> Result<String> {
        self.call_api_with_model(query, custom_prompt, &self.config.model_id).await
    }
    
    /// 使用指定模型和自定义系统提示词调用 API
    async fn call_api_with_model(&self, query: &str, custom_prompt: &str, model_id: &str) -> Result<String> {
        let retryable_codes = [401, 402, 403, 408, 429, 500, 501, 502, 503, 504];
        let mut last_error = None;
        
        for attempt in 0..=self.config.retry_count {
            match self.try_request_with_model(query, custom_prompt, model_id).await {
                Ok(result) => {
                    let filtered = if self.config.filter_thinking {
                        filter_thinking_content(&result)
                    } else {
                        result
                    };
                    return Ok(filtered);
                }
                Err(e) => {
                    if let AISearchError::Api { code, .. } = &e {
                        if retryable_codes.contains(code) && attempt < self.config.retry_count {
                            warn!("请求失败 (HTTP {}), 重试 {}/{}", code, attempt + 1, self.config.retry_count);
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                    
                    if attempt < self.config.retry_count {
                        warn!("请求失败, 重试 {}/{}", attempt + 1, self.config.retry_count);
                        sleep(Duration::from_secs(1)).await;
                        last_error = Some(e);
                        continue;
                    }
                    
                    return Err(e);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| AISearchError::Network {
            message: "未知错误".into(),
            suggestion: "请检查配置".into(),
        }))
    }

    async fn call_api(&self, query: &str) -> Result<String> {
        let retryable_codes = [401, 402, 403, 408, 429, 500, 501, 502, 503, 504];
        let mut last_error = None;
        
        for attempt in 0..=self.config.retry_count {
            match self.try_request(query).await {
                Ok(result) => {
                    let filtered = if self.config.filter_thinking {
                        filter_thinking_content(&result)
                    } else {
                        result
                    };
                    return Ok(filtered);
                }
                Err(e) => {
                    if let AISearchError::Api { code, .. } = &e {
                        if retryable_codes.contains(code) && attempt < self.config.retry_count {
                            warn!("请求失败 (HTTP {}), 重试 {}/{}", code, attempt + 1, self.config.retry_count);
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                    
                    if attempt < self.config.retry_count {
                        warn!("请求失败, 重试 {}/{}", attempt + 1, self.config.retry_count);
                        sleep(Duration::from_secs(1)).await;
                        last_error = Some(e);
                        continue;
                    }
                    
                    return Err(e);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| AISearchError::Network {
            message: "未知错误".into(),
            suggestion: "请检查配置".into(),
        }))
    }
    

    async fn try_request_with_model(&self, query: &str, custom_prompt: &str, model_id: &str) -> Result<String> {
        let endpoint = self.build_endpoint();
        let body = self.build_request_body_with_model(query, custom_prompt, model_id);
        
        let response = self.client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        let status = response.status();
        
        if !status.is_success() {
            let detail = match status.as_u16() {
                401 => "认证失败,请检查 API_KEY 是否正确".to_string(),
                429 => "请求过于频繁,建议稍后重试或切换 API 渠道".to_string(),
                code if code >= 500 => "服务器错误,请稍后重试".to_string(),
                _ => response.text().await.unwrap_or_default(),
            };
            
            return Err(AISearchError::Api {
                code: status.as_u16(),
                message: detail,
            });
        }
        
        if self.config.stream {
            self.handle_streaming_response(response).await
        } else {
            self.handle_json_response(response).await
        }
    }

    async fn try_request(&self, query: &str) -> Result<String> {
        let endpoint = self.build_endpoint();
        let body = self.build_request_body(query);
        
        let response = self.client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        let status = response.status();
        
        if !status.is_success() {
            let detail = match status.as_u16() {
                401 => "认证失败,请检查 API_KEY 是否正确".to_string(),
                429 => "请求过于频繁,建议稍后重试或切换 API 渠道".to_string(),
                code if code >= 500 => "服务器错误,请稍后重试".to_string(),
                _ => response.text().await.unwrap_or_default(),
            };
            
            return Err(AISearchError::Api {
                code: status.as_u16(),
                message: detail,
            });
        }
        
        if self.config.stream {
            self.handle_streaming_response(response).await
        } else {
            self.handle_json_response(response).await
        }
    }
    
    async fn handle_streaming_response(&self, response: reqwest::Response) -> Result<String> {
        const MAX_BUFFER_SIZE: usize = 10 * 1024 * 1024; // 10MB
        
        let mut stream = response.bytes_stream();
        let mut chunks = Vec::new();
        let mut buffer = String::new();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let new_content = String::from_utf8_lossy(&chunk);
            
            if buffer.len() + new_content.len() > MAX_BUFFER_SIZE {
                return Err(AISearchError::Protocol(
                    format!("响应过大，超过 {} MB 限制", MAX_BUFFER_SIZE / 1024 / 1024)
                ));
            }
            
            buffer.push_str(&new_content);
            
            for line in buffer.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data.trim() == "[DONE]" {
                        continue;
                    }
                    
                    if let Ok(parsed) = serde_json::from_str::<ChatResponse>(data) {
                        if let Some(choice) = parsed.choices.first() {
                            if let Some(delta) = &choice.delta {
                                if let Some(content) = &delta.content {
                                    chunks.push(content.clone());
                                }
                            }
                        }
                    }
                }
            }
            
            if let Some(last_newline) = buffer.rfind('\n') {
                buffer = buffer[last_newline + 1..].to_string();
            }
        }
        
        Ok(chunks.join(""))
    }
    
    async fn handle_json_response(&self, response: reqwest::Response) -> Result<String> {
        let result: ChatResponse = response.json().await?;
        
        result.choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .ok_or_else(|| AISearchError::Protocol("响应格式错误".into()))
    }
    
    fn build_endpoint(&self) -> String {
        let mut url = self.config.api_url.clone();
        if !url.ends_with("/v1/chat/completions") {
            if url.ends_with('/') {
                url.push_str("v1/chat/completions");
            } else {
                url.push_str("/v1/chat/completions");
            }
        }
        url
    }
    

    fn build_request_body_with_model(&self, query: &str, custom_prompt: &str, model_id: &str) -> ChatRequest {
        ChatRequest {
            model: model_id.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: custom_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: query.to_string(),
                },
            ],
            stream: self.config.stream,
        }
    }

    fn build_request_body(&self, query: &str) -> ChatRequest {
        let current_time = Local::now().format("%Y-%m-%d %H:%M:%S %A").to_string();
        let system_prompt = self.config.system_prompt.replace("{current_time}", &current_time);
        
        ChatRequest {
            model: self.config.model_id.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: query.to_string(),
                },
            ],
            stream: self.config.stream,
        }
    }
    
    /// 调用 AI 模型将查询拆分成多个子问题
    async fn split_query(&self, query: &str, count: u32) -> Result<Vec<String>> {
        let split_prompt = format!(
            r#"将查询拆分成 {} 个子问题，返回 JSON 数组。

查询: {}

只返回 JSON 数组，格式: ["子问题1", "子问题2", "子问题3"]"#,
            count, query
        );
        
        let system_prompt = "你是查询拆分助手。只返回 JSON 数组，不要任何解释、标记或其他文本。直接输出 JSON 数组。";
        
        // 使用分析模型（如果配置了）或默认模型
        let response = if let Some(analysis_model) = &self.config.analysis_model_id {
            info!("使用分析模型拆分查询: {}", analysis_model);
            self.call_api_with_model(&split_prompt, system_prompt, analysis_model).await?
        } else {
            info!("使用默认模型拆分查询: {}", self.config.model_id);
            self.call_api_with_custom_prompt(&split_prompt, system_prompt).await?
        };
        
        info!("AI 返回的原始响应: {}", response);
        
        // 先尝试过滤 thinking 标签
        let filtered = filter_thinking_content(&response);
        
        // 如果过滤后为空，使用原始响应
        let content = if filtered.is_empty() {
            warn!("过滤后内容为空，使用原始响应");
            &response
        } else {
            &filtered
        };
        
        info!("处理后的响应: {}", content);
        
        // 清理响应，移除可能的 markdown 代码块标记
        let cleaned = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        
        info!("清理后的响应: {}", cleaned);
        
        // 解析 JSON 数组
        let sub_queries: Vec<String> = serde_json::from_str(cleaned)
            .map_err(|e| {
                error!("JSON 解析失败，原始响应: {}", filtered);
                AISearchError::Protocol(format!("解析子查询失败: {}，响应内容: {}", e, cleaned))
            })?;
        
        if sub_queries.is_empty() {
            return Err(AISearchError::Protocol("未能拆分出任何子查询".into()));
        }
        
        if sub_queries.len() != count as usize {
            warn!("期望 {} 个子查询，实际得到 {}，继续执行", count, sub_queries.len());
        }
        
        // 为每个子查询添加 [SUB_QUERY] 前缀
        let prefixed_queries: Vec<String> = sub_queries
            .into_iter()
            .map(|q| format!("[SUB_QUERY] {}", q))
            .collect();
        
        Ok(prefixed_queries)
    }
}

fn filter_thinking_content(content: &str) -> String {
    let content = THINKING_PATTERN.replace_all(content, "");
    let content = WHITESPACE_PATTERN.replace_all(&content, "\n\n");
    content.trim().to_string()
}
