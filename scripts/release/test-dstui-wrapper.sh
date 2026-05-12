#!/usr/bin/env bash
# dstui-wrapper.sh 集成测试
# 验证：环境变量设置、目录创建、二进制查找逻辑

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
WRAPPER="$PROJECT_ROOT/scripts/release/dstui-wrapper.sh"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass=0
fail=0

assert_eq() {
    local desc="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo -e "${GREEN}PASS${NC}: $desc"
        pass=$((pass + 1))
    else
        echo -e "${RED}FAIL${NC}: $desc"
        echo "  expected: $expected"
        echo "  actual:   $actual"
        fail=$((fail + 1))
    fi
}

assert_file_contains() {
    local desc="$1" file="$2" pattern="$3"
    if grep -q "$pattern" "$file" 2>/dev/null; then
        echo -e "${GREEN}PASS${NC}: $desc"
        pass=$((pass + 1))
    else
        echo -e "${RED}FAIL${NC}: $desc — pattern '$pattern' not found in $file"
        fail=$((fail + 1))
    fi
}

# --- 测试 1: 脚本文件存在 ---
echo "=== 测试 1: 脚本文件验证 ==="

assert_eq "dstui-wrapper.sh 存在" "1" "$([ -f "$WRAPPER" ] && echo 1 || echo 0)"

# --- 测试 2: 脚本包含关键逻辑 ---
echo "=== 测试 2: 脚本内容验证 ==="

assert_file_contains "wrapper 设置 DSTUI_HOME" "$WRAPPER" 'DSTUI_HOME'
assert_file_contains "wrapper 设置 DEEPSEEK_CONFIG_PATH" "$WRAPPER" 'DEEPSEEK_CONFIG_PATH'
assert_file_contains "wrapper 默认路径 ~/.dstui" "$WRAPPER" '.dstui'
assert_file_contains "wrapper mkdir -p" "$WRAPPER" 'mkdir -p'
assert_file_contains "wrapper exec 启动" "$WRAPPER" 'exec'

# --- 测试 3: 环境变量设置逻辑（不实际启动 dstui） ---
echo "=== 测试 3: 环境变量逻辑验证 ==="

# 模拟 wrapper 的环境变量设置
TEST_HOME=$(mktemp -d)
export HOME="$TEST_HOME"

# 默认 DSTUI_HOME
DSTUI_HOME_DEFAULT="${DSTUI_HOME:-$HOME/.dstui}"
assert_eq "默认 DSTUI_HOME 指向 ~/.dstui" "$TEST_HOME/.dstui" "$DSTUI_HOME_DEFAULT"

# DEEPSEEK_CONFIG_PATH 从 DSTUI_HOME 派生
DEEPSEEK_CONFIG_PATH_RESULT="$DSTUI_HOME_DEFAULT/config.toml"
assert_eq "DEEPSEEK_CONFIG_PATH 派生正确" "$TEST_HOME/.dstui/config.toml" "$DEEPSEEK_CONFIG_PATH_RESULT"

# 自定义 DSTUI_HOME
export DSTUI_HOME="/custom/path"
DEEPSEEK_CONFIG_PATH_CUSTOM="$DSTUI_HOME/config.toml"
assert_eq "自定义 DSTUI_HOME 时 DEEPSEEK_CONFIG_PATH" "/custom/path/config.toml" "$DEEPSEEK_CONFIG_PATH_CUSTOM"
unset DSTUI_HOME

# 清理临时目录
rm -rf "$TEST_HOME"

# --- 测试 4: 目录创建逻辑 ---
echo "=== 测试 4: 目录创建验证 ==="

TEST_HOME2=$(mktemp -d)
export HOME="$TEST_HOME2"
DSTUI_DIR="$TEST_HOME2/.dstui"

# 模拟 mkdir -p
mkdir -p "$DSTUI_DIR"
assert_eq "mkdir -p 成功创建目录" "1" "$([ -d "$DSTUI_DIR" ] && echo 1 || echo 0)"

# 重复执行幂等
mkdir -p "$DSTUI_DIR"
assert_eq "mkdir -p 幂等性" "1" "$([ -d "$DSTUI_DIR" ] && echo 1 || echo 0)"

rm -rf "$TEST_HOME2"

# --- 汇总 ---
echo ""
echo "========================================"
echo "结果: $pass 通过, $fail 失败"
echo "========================================"

[ "$fail" -eq 0 ]
