"""AI Search MCP Server - Python Wrapper for Rust Binary"""

import os
import platform
import subprocess
import sys
from pathlib import Path

__version__ = "1.2.0"


def get_binary_name():
    """获取当前平台的二进制文件名"""
    system = platform.system().lower()
    
    if system == "windows":
        return "ai-search-mcp.exe"
    elif system == "linux":
        return "ai-search-mcp-linux"
    elif system == "darwin":
        return "ai-search-mcp-macos"
    else:
        raise RuntimeError(f"不支持的操作系统: {system}")


def get_binary_path():
    """获取二进制文件的完整路径"""
    binary_name = get_binary_name()
    binary_path = Path(__file__).parent / "bin" / binary_name
    
    if not binary_path.exists():
        raise FileNotFoundError(
            f"未找到二进制文件: {binary_path}\n"
            f"请确保已正确安装 ai-search-mcp 包"
        )
    
    # 确保二进制文件有执行权限（Unix 系统）
    if platform.system() != "Windows":
        os.chmod(binary_path, 0o755)
    
    return binary_path


def main():
    """主入口函数，调用 Rust 二进制文件"""
    try:
        binary_path = get_binary_path()
        
        # 直接执行二进制文件，传递所有参数
        result = subprocess.run(
            [str(binary_path)] + sys.argv[1:],
            stdin=sys.stdin,
            stdout=sys.stdout,
            stderr=sys.stderr,
        )
        
        sys.exit(result.returncode)
        
    except FileNotFoundError as e:
        print(f"错误: {e}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"执行失败: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
