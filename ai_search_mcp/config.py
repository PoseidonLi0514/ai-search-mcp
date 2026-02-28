"""配置管理模块"""
import os
from dataclasses import dataclass
from typing import Optional

from .exceptions import MissingConfigError, InvalidConfigError

# 常量定义
MIN_TIMEOUT = 1
MAX_TIMEOUT = 300
DEFAULT_TIMEOUT = 60  # 默认 60 秒，复杂查询建议在配置中设置为 120
DEFAULT_RETRY_COUNT = 1  # 默认重试 1 次（总共 2 次请求）
DEFAULT_LOG_LEVEL = "INFO"
DEFAULT_MAX_QUERY_PLAN = 1

DEFAULT_SYSTEM_PROMPT = """你是一个专业的搜索助手,擅长联网搜索并提供准确、详细的答案。

当前时间: {current_time}

搜索策略:
1. 优先使用最新、权威的信息源
2. 对于时间敏感的查询,明确标注信息的时间
3. 提供多个来源的信息进行交叉验证
4. 对于技术问题,优先参考官方文档和最新版本

输出要求:
- 直接回答用户问题,避免冗余
- 标注关键信息的来源 [来源](URL)
- 对于复杂问题,提供结构化的答案
- 时间相关信息必须基于上述当前时间判断"""


@dataclass
class AIConfig:
    """AI API 配置"""
    api_url: str
    api_key: str
    model_id: str
    system_prompt: str = DEFAULT_SYSTEM_PROMPT
    timeout: int = DEFAULT_TIMEOUT
    stream: bool = True
    filter_thinking: bool = True
    retry_count: int = DEFAULT_RETRY_COUNT
    log_level: str = DEFAULT_LOG_LEVEL
    max_query_plan: int = DEFAULT_MAX_QUERY_PLAN
    
    def validate(self) -> None:
        """
        验证配置有效性
        
        Raises:
            InvalidConfigError: 配置无效
            MissingConfigError: 缺少必需配置
        """
        if not (self.api_url.startswith('http://') or self.api_url.startswith('https://')):
            raise InvalidConfigError(
                f"API URL 必须以 http:// 或 https:// 开头: {self.api_url}",
                "示例: http://localhost:10000 或 https://api.example.com"
            )
        
        if self.timeout < MIN_TIMEOUT or self.timeout > MAX_TIMEOUT:
            raise InvalidConfigError(
                f"超时时间必须在 {MIN_TIMEOUT}-{MAX_TIMEOUT} 秒之间: {self.timeout}",
                "建议设置为 30-120 秒"
            )
        
        if not self.api_key:
            raise MissingConfigError('api_key', 'AI_API_KEY=your-api-key')
        if not self.model_id:
            raise MissingConfigError('model_id', 'AI_MODEL_ID=Grok')
    
    def to_dict(self) -> dict:
        """转换为字典"""
        return {
            'api_url': self.api_url,
            'api_key': self.api_key,
            'model_id': self.model_id,
            'system_prompt': self.system_prompt,
            'timeout': self.timeout,
            'stream': self.stream,
            'filter_thinking': self.filter_thinking,
            'retry_count': self.retry_count,
            'log_level': self.log_level,
            'max_query_plan': self.max_query_plan
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> 'AIConfig':
        """
        从字典创建配置对象
        
        Args:
            data: 配置字典
            
        Returns:
            AIConfig 实例
        """
        return cls(
            api_url=data.get('api_url', ''),
            api_key=data.get('api_key', ''),
            model_id=data.get('model_id', ''),
            system_prompt=data.get('system_prompt', DEFAULT_SYSTEM_PROMPT),
            timeout=data.get('timeout', DEFAULT_TIMEOUT),
            stream=data.get('stream', True),
            filter_thinking=data.get('filter_thinking', True),
            retry_count=data.get('retry_count', DEFAULT_RETRY_COUNT),
            log_level=data.get('log_level', DEFAULT_LOG_LEVEL),
            max_query_plan=data.get('max_query_plan', DEFAULT_MAX_QUERY_PLAN)
        )


def load_from_env() -> dict:
    """从环境变量加载配置"""
    config = {}
    
    if os.getenv('AI_API_URL'):
        config['api_url'] = os.getenv('AI_API_URL')
    if os.getenv('AI_API_KEY'):
        config['api_key'] = os.getenv('AI_API_KEY')
    if os.getenv('AI_MODEL_ID'):
        config['model_id'] = os.getenv('AI_MODEL_ID')
    if os.getenv('AI_SYSTEM_PROMPT'):
        config['system_prompt'] = os.getenv('AI_SYSTEM_PROMPT')
    if os.getenv('AI_TIMEOUT'):
        config['timeout'] = int(os.getenv('AI_TIMEOUT'))
    if os.getenv('AI_STREAM'):
        config['stream'] = os.getenv('AI_STREAM', 'true').lower() == 'true'
    if os.getenv('AI_FILTER_THINKING'):
        config['filter_thinking'] = os.getenv('AI_FILTER_THINKING', 'true').lower() == 'true'
    if os.getenv('AI_RETRY_COUNT'):
        config['retry_count'] = int(os.getenv('AI_RETRY_COUNT'))
    if os.getenv('AI_LOG_LEVEL'):
        config['log_level'] = os.getenv('AI_LOG_LEVEL').upper()
    if os.getenv('AI_MAX_QUERY_PLAN'):
        config['max_query_plan'] = int(os.getenv('AI_MAX_QUERY_PLAN'))
    
    return config


def load_config(config_file: Optional[str] = None) -> AIConfig:
    """
    加载配置，优先级：配置文件 > 环境变量
    
    Args:
        config_file: 配置文件路径（可选）
        
    Returns:
        AIConfig 实例
        
    Raises:
        MissingConfigError: 配置无效或缺失必需项
    """
    env_config = load_from_env()
    
    if not env_config.get('api_url'):
        raise MissingConfigError('AI_API_URL', 'AI_API_URL=http://localhost:10000')
    if not env_config.get('api_key'):
        raise MissingConfigError('AI_API_KEY', 'AI_API_KEY=your-api-key')
    if not env_config.get('model_id'):
        raise MissingConfigError('AI_MODEL_ID', 'AI_MODEL_ID=Grok')
    
    config = AIConfig.from_dict(env_config)
    config.validate()
    
    return config
