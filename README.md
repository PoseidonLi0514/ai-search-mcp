# AI Search MCP Server

[![PyPI version](https://badge.fury.io/py/ai-search-mcp.svg)](https://badge.fury.io/py/ai-search-mcp)
[![Python versions](https://img.shields.io/pypi/pyversions/ai-search-mcp.svg)](https://pypi.org/project/ai-search-mcp/)
[![License](https://img.shields.io/pypi/l/ai-search-mcp.svg)](https://github.com/lianwusuoai/ai-search-mcp/blob/main/LICENSE)

通用 AI 搜索 MCP 服务器，支持任何兼容 OpenAI API 格式的 AI 模型进行联网搜索。

## 特性

- ✅ 支持任何 OpenAI API 兼容的模型
- ✅ 支持流式和非流式响应
- ✅ 自动过滤 AI 思考内容
- ✅ **自动时间注入**：每次搜索自动注入当前时间，提升时间相关查询准确性
- ✅ **增强系统提示词**：内置优化的搜索策略和输出要求
- ✅ **多维度搜索**：自动拆分复杂查询为多个子问题并行搜索，结果更全面
- ✅ **智能重试机制**：自动重试失败的请求，提升成功率
- ✅ 完全可配置（支持自定义系统提示词）
- ✅ Windows 平台完美支持中文显示

## 安装

```bash
pip install ai-search-mcp
```

**说明**：
- 推荐使用 `uvx` 运行（无需安装，自动使用最新版本）
- 使用 `pip` 安装后需手动更新：`pip install --upgrade ai-search-mcp`

## 快速开始

编辑配置文件（Kiro IDE: `.kiro/settings/mcp.json` | Claude Desktop: `claude_desktop_config.json`）:

```json
{
  "mcpServers": {
    "ai-search": {
      "command": "uvx",
      "args": ["ai-search-mcp"],
      "env": {
        // 必需配置
        "AI_API_URL": "http://localhost:10000",
        "AI_API_KEY": "your-api-key",
        "AI_MODEL_ID": "Grok",
        // 可选配置
        "AI_TIMEOUT": "60",                    // 超时时间（秒），复杂查询建议 120
        "AI_STREAM": "true",                   // 是否启用流式响应
        "AI_FILTER_THINKING": "true",          // 是否过滤思考内容
        "AI_RETRY_COUNT": "1",                 // 重试次数（默认 1）
        "AI_LOG_LEVEL": "INFO",                // 日志级别（DEBUG/INFO/WARNING/ERROR）
        "AI_MAX_QUERY_PLAN": "3",              // 多维度搜索提示数（1=提示直接搜索，>1=提示拆分成N个子问题）
        // 自定义提示词（必须保留 {current_time} 占位符）
        "AI_SYSTEM_PROMPT": "你是搜索助手。当前时间: {current_time}。请提供准确答案并标注来源。"
      }
    }
  }
}
```

## 工具说明

### `web_search` - 网络搜索

**输入**：`{"query": "搜索内容"}`

**多维度搜索**（由 `AI_MAX_QUERY_PLAN` 控制）：
- `= 1`：直接返回搜索结果
- `> 1`：首次调用返回拆分要求，AI 需拆分成 N 个子问题并行搜索（子问题加 `[SUB_QUERY]` 前缀防止套娃），然后整合结果

详细示例见下方"多维度搜索示例"章节。

---

## 配置说明

### 环境变量

| 变量 | 必需 | 默认值 | 说明 |
|------|------|--------|------|
| `AI_API_URL` | ✅ | - | AI API 地址 |
| `AI_API_KEY` | ✅ | - | API 密钥 |
| `AI_MODEL_ID` | ✅ | - | 模型 ID |
| `AI_TIMEOUT` | ❌ | `60` | 超时时间（秒），复杂查询建议 120 |
| `AI_STREAM` | ❌ | `true` | 是否启用流式响应 |
| `AI_FILTER_THINKING` | ❌ | `true` | 是否过滤思考内容 |
| `AI_RETRY_COUNT` | ❌ | `1` | 重试次数（0 = 不重试） |
| `AI_LOG_LEVEL` | ❌ | `INFO` | 日志级别（DEBUG/INFO/WARNING/ERROR） |
| `AI_MAX_QUERY_PLAN` | ❌ | `3` | 复杂查询拆分维度数（建议 3-7） |
| `AI_SYSTEM_PROMPT` | ❌ | 见下方 | 自定义系统提示词 |

### 默认系统提示词

内置优化的提示词，包含自动时间注入、搜索策略指导、输出质量要求：

```
你是一个专业的搜索助手,擅长联网搜索并提供准确、详细的答案。

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
- 时间相关信息必须基于上述当前时间判断
```

### 自定义提示词示例

**重要**：必须保留 `{current_time}` 占位符

```json
// 简化版
"AI_SYSTEM_PROMPT": "你是搜索助手。当前时间: {current_time}。请提供准确答案并标注来源。"

// 技术文档专用
"AI_SYSTEM_PROMPT": "你是技术文档搜索专家。当前时间: {current_time}。专注于官方文档、GitHub 仓库和技术博客，提供代码示例并标注版本信息。"
```

---

## 多维度搜索示例

### 简单查询（AI_MAX_QUERY_PLAN = 1）
用户：Python 是什么  
→ AI 调用：`web_search("Python 是什么")`  
→ MCP 返回：直接返回搜索结果

### 复杂查询（AI_MAX_QUERY_PLAN = 3）
用户：春节北京到上海高铁票价  
→ AI 首次调用：`web_search("春节北京到上海高铁票价")`  
→ MCP 返回：拆分要求（提示拆成 3 个子问题）  
→ AI 并行调用：
```python
web_search("[SUB_QUERY] 春节北京到上海直达高铁票价")
web_search("[SUB_QUERY] 北京到上海中转方案票价对比")
web_search("[SUB_QUERY] 北京周边站点到上海买长乘短策略")
```
→ MCP 返回：每个子查询的搜索结果（`[SUB_QUERY]` 前缀防止再次拆分）  
→ AI 整合：自动整合 3 个结果，返回完整答案

---

## 支持的服务

任何兼容 OpenAI API 格式的服务都可以使用，例如：

- Grok（本地部署）
- OpenAI（GPT-4、GPT-3.5）
- 本地模型（Ollama、LM Studio）
- 其他兼容服务

## 命令行工具

```bash
# 查看版本
ai-search-mcp --version

# 验证配置
ai-search-mcp --validate-config
```

## 开发

```bash
git clone https://github.com/lianwusuoai/ai-search-mcp.git
cd ai-search-mcp
pip install -e .
```

## 许可证

MIT License

## 链接

- [GitHub](https://github.com/lianwusuoai/ai-search-mcp)
- [PyPI](https://pypi.org/project/ai-search-mcp/)
- [问题反馈](https://github.com/lianwusuoai/ai-search-mcp/issues)
