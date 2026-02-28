#!/usr/bin/env python3
"""构建脚本：编译 Rust 并复制二进制文件到 Python 包"""

import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


def get_binary_name():
    """获取目标平台的二进制文件名"""
    system = platform.system().lower()
    if system == "windows":
        return "ai-search-mcp.exe"
    elif system == "linux":
        return "ai-search-mcp-linux"
    elif system == "darwin":
        return "ai-search-mcp-macos"
    else:
        raise RuntimeError(f"不支持的操作系统: {system}")


def build_rust():
    """编译 Rust 项目"""
    print("正在编译 Rust 项目...")
    result = subprocess.run(
        ["cargo", "build", "--release"],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        print(f"Rust 编译失败:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)
    
    print("Rust 编译成功")


def copy_binary():
    """复制编译好的二进制文件到 Python 包目录"""
    project_root = Path(__file__).parent
    
    # 源文件路径
    if platform.system() == "Windows":
        src_binary = project_root / "target" / "release" / "ai-search-mcp.exe"
    else:
        src_binary = project_root / "target" / "release" / "ai-search-mcp"
    
    # 目标路径
    bin_dir = project_root / "ai_search_mcp" / "bin"
    bin_dir.mkdir(exist_ok=True)
    
    dst_binary = bin_dir / get_binary_name()
    
    if not src_binary.exists():
        print(f"错误: 找不到编译后的二进制文件: {src_binary}", file=sys.stderr)
        sys.exit(1)
    
    print(f"复制 {src_binary} -> {dst_binary}")
    shutil.copy2(src_binary, dst_binary)
    
    # Unix 系统设置执行权限
    if platform.system() != "Windows":
        os.chmod(dst_binary, 0o755)
    
    print("二进制文件复制成功")


def main():
    """主函数"""
    build_rust()
    copy_binary()
    print("\n构建完成！")


if __name__ == "__main__":
    main()
