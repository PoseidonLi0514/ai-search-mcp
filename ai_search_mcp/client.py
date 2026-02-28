"""AI 客户端模块"""
import re
import json
import time
import logging
from typing import Optional
from datetime import datetime
import requests

from .config import AIConfig
from .exceptions import APIError, NetworkError, TimeoutError

# 配置日志
logger = logging.getLogger(__name__)

# 常量定义
SSE_DATA_PREFIX = 'data: '
SSE_DATA_PREFIX_LEN = len(SSE_DATA_PREFIX)
SSE_DONE_MESSAGE = '[DONE]'
RETRY_DELAY = 1  # 重试延迟（秒）

# 预编译正则表达式
THINKING_PATTERN = re.compile(
    r'<think(?:ing)?>.*?</think(?:ing)?>',
    re.DOTALL | re.IGNORECASE
)
WHITESPACE_PATTERN = re.compile(r'\n\s*\n')


class AIClient:
    """
    AI API 客户端
    
    支持上下文管理器协议，自动管理资源。
    
    Example:
        with AIClient(config) as client:
            result = client.search("query")
    """
    
    def __init__(self, config: AIConfig):
        """
        初始化客户端
        
        Args:
            config: AI 配置对象
        """
        self.config = config
        self.session = requests.Session()
        self.session.headers.update({
            'Authorization': f'Bearer {config.api_key}',
            'Content-Type': 'application/json'
        })
    
    def __enter__(self) -> 'AIClient':
        """进入上下文管理器"""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """退出上下文管理器，关闭 session"""
        self.close()
    
    def close(self) -> None:
        """关闭 session 释放资源"""
        if self.session:
            self.session.close()
    
    def search(self, query: str) -> str:
        """
        执行搜索
        
        Args:
            query: 搜索查询内容
            
        Returns:
            搜索结果文本或拆分要求
            
        Raises:
            APIError: API 调用失败
            NetworkError: 网络连接失败
            TimeoutError: 请求超时
        """
        # 检查是否是子查询（防止套娃拆分）
        is_sub_query = query.startswith('[SUB_QUERY]')
        
        if is_sub_query:
            # 子查询：移除标记，直接搜索
            actual_query = query[len('[SUB_QUERY]'):].strip()
            logger.info(f"[子查询] 直接搜索: {actual_query}")
            body = self._build_request_body(actual_query)
            return self._call_api(body, filter_thinking=self.config.filter_thinking)
        
        # 原始查询：根据 max_query_plan 决定行为
        if self.config.max_query_plan > 1:
            # 要求拆分
            logger.info(f"[多维度搜索] 要求拆分成 {self.config.max_query_plan} 个子问题")
            return f"""请将以下查询拆分成 {self.config.max_query_plan} 个不同角度的子问题，然后并行调用 web_search 进行搜索。

原始查询: {query}

拆分要求:
1. 拆分成 {self.config.max_query_plan} 个子问题
2. 每个子问题从不同角度切入，互补覆盖原查询
3. 每个子问题前加上 [SUB_QUERY] 标记（防止套娃拆分）

调用示例:
web_search("[SUB_QUERY] 子问题1")
web_search("[SUB_QUERY] 子问题2")
web_search("[SUB_QUERY] 子问题3")

然后自行整合 {self.config.max_query_plan} 个搜索结果，给出完整答案。"""
        else:
            # 直接搜索
            logger.info(f"[直接搜索] {query}")
            body = self._build_request_body(query)
            return self._call_api(body, filter_thinking=self.config.filter_thinking)
    

    def _call_api(self, body: dict, filter_thinking: bool = True) -> str:
        """
        通用 API 调用方法（带重试）
        
        注意：retry_count 表示"额外重试次数"，总请求次数 = 1 + retry_count
        例如 retry_count=1 时，会先尝试 1 次，失败后再重试 1 次，共 2 次请求
        
        Args:
            body: 请求体
            filter_thinking: 是否过滤思考内容
            
        Returns:
            AI 返回的文本
            
        Raises:
            APIError: API 调用失败
            NetworkError: 网络连接失败
            TimeoutError: 请求超时
        """
        retryable_codes = {408, 429, 500, 502, 503, 504}
        last_error = None
        
        for attempt in range(self.config.retry_count + 1):
            try:
                endpoint = self._build_endpoint()
                
                response = self.session.post(
                    endpoint,
                    json=body,
                    stream=body.get('stream', False),
                    timeout=self.config.timeout
                )
                
                if response.status_code == 200:
                    if body.get('stream'):
                        result = self._handle_streaming_response(response)
                    else:
                        result = self._handle_json_response(response)
                    
                    if filter_thinking:
                        result = self._filter_thinking_content(result)
                    
                    return result
                
                # 可重试的错误
                if response.status_code in retryable_codes and attempt < self.config.retry_count:
                    logger.warning(f"请求失败 (HTTP {response.status_code}), 重试 {attempt + 1}/{self.config.retry_count}")
                    time.sleep(RETRY_DELAY)
                    continue
                
                # 不可重试的错误或重试次数用尽
                detail = response.text
                if response.status_code == 401:
                    detail = "认证失败,请检查 API_KEY 是否正确"
                elif response.status_code == 429:
                    detail = "请求过于频繁,建议稍后重试或切换 API 渠道"
                elif response.status_code >= 500:
                    detail = "服务器错误,请稍后重试"
                
                raise APIError(response.status_code, detail)
                
            except requests.exceptions.ConnectionError as e:
                # 只显示域名，避免泄露完整 URL
                from urllib.parse import urlparse
                domain = urlparse(self.config.api_url).netloc or "未知服务器"
                last_error = NetworkError(
                    f"无法连接到 API 服务器: {domain}",
                    "请检查: 1) API 地址是否正确 2) 网络连接是否正常 3) 服务器是否运行"
                )
                if attempt < self.config.retry_count:
                    logger.warning(f"网络连接失败, 重试 {attempt + 1}/{self.config.retry_count}")
                    time.sleep(RETRY_DELAY)
                    continue
                raise last_error
                
            except requests.exceptions.Timeout:
                last_error = TimeoutError(self.config.timeout)
                if attempt < self.config.retry_count:
                    logger.warning(f"请求超时, 重试 {attempt + 1}/{self.config.retry_count}")
                    time.sleep(RETRY_DELAY)
                    continue
                raise last_error
                
            except (APIError, NetworkError, TimeoutError):
                raise
            except Exception as e:
                raise NetworkError(f"请求失败: {str(e)}", "请检查网络连接和配置")
        
        # 理论上不会到这里
        if last_error:
            raise last_error
        raise NetworkError("未知错误", "请检查配置")
    
    def _build_endpoint(self) -> str:
        """构建 API 端点 URL"""
        api_url = self.config.api_url
        if not api_url.endswith('/v1/chat/completions'):
            if api_url.endswith('/'):
                api_url += 'v1/chat/completions'
            else:
                api_url += '/v1/chat/completions'
        return api_url
    
    def _build_request_body(self, query: str, system_prompt: Optional[str] = None) -> dict:
        """构建请求体"""
        # 自动注入当前时间
        current_time = datetime.now().strftime("%Y-%m-%d %H:%M:%S %A")
        
        # 使用传入的 system_prompt 或默认的
        if system_prompt is None:
            system_prompt = self.config.system_prompt.format(current_time=current_time)
        
        return {
            'model': self.config.model_id,
            'messages': [
                {
                    'role': 'system',
                    'content': system_prompt
                },
                {
                    'role': 'user',
                    'content': query
                }
            ],
            'stream': self.config.stream
        }
    
    def _handle_streaming_response(self, response: requests.Response) -> str:
        """处理流式响应"""
        chunks = []
        response.encoding = 'utf-8'
        for line in response.iter_lines(decode_unicode=True):
            if line and line.startswith('data: '):
                content = self._parse_sse_line(line)
                if content:
                    chunks.append(content)
        return ''.join(chunks)
    
    def _handle_json_response(self, response: requests.Response) -> str:
        """处理 JSON 响应"""
        try:
            result = response.json()
            return result['choices'][0]['message']['content']
        except (json.JSONDecodeError, KeyError) as e:
            raise APIError(
                response.status_code,
                f"响应格式错误: {str(e)}"
            )
    
    def _parse_sse_line(self, line: str) -> Optional[str]:
        """
        解析 SSE 数据行
        
        Args:
            line: SSE 数据行
            
        Returns:
            提取的内容，如果无内容则返回 None
        """
        data_str = line[SSE_DATA_PREFIX_LEN:]  # 移除 'data: ' 前缀
        if data_str.strip() == SSE_DONE_MESSAGE:
            return None
        
        try:
            data = json.loads(data_str)
            if 'choices' in data and len(data['choices']) > 0:
                delta = data['choices'][0].get('delta', {})
                return delta.get('content', '')
        except json.JSONDecodeError:
            pass
        
        return None
    
    def _filter_thinking_content(self, content: str) -> str:
        """
        过滤思考内容
        
        移除 <think>...</think> 和 <thinking>...</thinking> 标签及其内容
        
        Args:
            content: 原始内容
            
        Returns:
            过滤后的内容
        """
        # 使用预编译的正则表达式
        content = THINKING_PATTERN.sub('', content)
        # 清理多余的空白
        content = WHITESPACE_PATTERN.sub('\n\n', content)
        return content.strip()
