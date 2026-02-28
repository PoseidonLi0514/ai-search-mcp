"""从 Cargo.toml 读取版本号"""
import re
from pathlib import Path

# 从 Cargo.toml 读取版本
_cargo_toml = Path(__file__).parent.parent / "Cargo.toml"
if _cargo_toml.exists():
    _content = _cargo_toml.read_text(encoding="utf-8")
    _match = re.search(r'^version\s*=\s*"([^"]+)"', _content, re.MULTILINE)
    if _match:
        __version__ = _match.group(1)
    else:
        __version__ = "0.0.0"
else:
    __version__ = "0.0.0"
