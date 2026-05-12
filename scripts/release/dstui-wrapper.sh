#!/usr/bin/env bash
# Fork 启动 wrapper：通过环境变量将配置目录指向 ~/.dstui/
# 安装时将此脚本与 dstui 二进制放在同一目录，或添加到 PATH

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DSTUI_BIN="${SCRIPT_DIR}/dstui"

# 如果没有找到 dstui，尝试 PATH
if [ ! -x "$DSTUI_BIN" ]; then
    DSTUI_BIN="$(command -v dstui 2>/dev/null || true)"
fi

if [ -z "$DSTUI_BIN" ]; then
    echo "错误: 找不到 dstui 二进制" >&2
    exit 1
fi

# 设置 fork 专属配置路径
DSTUI_HOME="${DSTUI_HOME:-$HOME/.dstui}"
export DEEPSEEK_CONFIG_PATH="${DSTUI_HOME}/config.toml"

# 确保 ~/.dstui/ 目录存在
mkdir -p "$DSTUI_HOME"

exec "$DSTUI_BIN" "$@"
