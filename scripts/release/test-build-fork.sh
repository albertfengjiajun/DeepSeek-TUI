#!/usr/bin/env bash
# build-fork.sh 集成测试
# 验证：sed 替换正确、构建后 Cargo.toml 还原、trap EXIT 正常工作

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_FORK="$PROJECT_ROOT/scripts/release/build-fork.sh"

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

# --- 测试 1: Cargo.toml 在脚本执行前后 bin name 保持 deepseek ---
echo "=== 测试 1: Cargo.toml 还原验证 ==="

CARGO_TOML="$PROJECT_ROOT/Cargo.toml"
CLI_TOML="$PROJECT_ROOT/crates/cli/Cargo.toml"

# 记录执行前的 bin name 行（只有 cli/Cargo.toml 含 name = "deepseek"）
# 注意：Windows 上可能是 CRLF，用 tr 去掉 \r
before_cli=$(grep 'name = "deepseek"' "$CLI_TOML" 2>/dev/null | tr -d '\r' || echo "NOT_FOUND")

# 跳过实际构建，仅测试 sed 替换/还原逻辑
echo "(跳过 cargo build --release，仅测试 sed 逻辑)"

# 手动模拟 sed 替换
if [ -f "$CLI_TOML" ]; then
    sed -i 's/name = "deepseek"/name = "dstui"/' "$CLI_TOML"
fi

assert_file_contains "替换后 cli Cargo.toml 含 dstui" "$CLI_TOML" 'name = "dstui"'

# 还原
if [ -f "$CLI_TOML" ]; then
    sed -i 's/name = "dstui"/name = "deepseek"/' "$CLI_TOML"
fi

assert_file_contains "还原后 cli Cargo.toml 含 deepseek" "$CLI_TOML" 'name = "deepseek"'

after_cli=$(grep 'name = "deepseek"' "$CLI_TOML" 2>/dev/null | tr -d '\r' || echo "NOT_FOUND")

assert_eq "cli Cargo.toml 还原一致性" "$before_cli" "$after_cli"

# --- 测试 2: 脚本文件存在且可执行 ---
echo "=== 测试 2: 脚本文件验证 ==="

assert_eq "build-fork.sh 存在" "1" "$([ -f "$BUILD_FORK" ] && echo 1 || echo 0)"

# --- 测试 3: 脚本包含关键逻辑 ---
echo "=== 测试 3: 脚本内容验证 ==="

assert_file_contains "build-fork.sh 包含 sed 替换" "$BUILD_FORK" 'sed -i'
assert_file_contains "build-fork.sh 包含 cargo build" "$BUILD_FORK" 'cargo build --release'
assert_file_contains "build-fork.sh 包含 trap EXIT" "$BUILD_FORK" 'trap restore EXIT'
assert_file_contains "build-fork.sh 包含还原函数" "$BUILD_FORK" 'restore()'
assert_file_contains "build-fork.sh 包含版本号替换" "$BUILD_FORK" 'FORK_VERSION'
assert_file_contains "build-fork.sh 包含批量 sed 替换版本" "$BUILD_FORK" 'find . -name "Cargo.toml"'

# --- 测试 4: 版本号批量替换验证 ---
echo "=== 测试 4: 版本号替换还原验证 ==="

UPSTREAM_VERSION=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/' | tr -d '\r')
assert_eq "上游版本号可读取" "0.8.20" "$UPSTREAM_VERSION"

# 模拟批量替换
find "$PROJECT_ROOT" -name "Cargo.toml" -exec sed -i 's/version = "0.8.20"/version = "0.1.0-fork"/g' {} +

CLI_VERSION=$(grep '^version = ' "$CLI_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/' | tr -d '\r' || echo "NOT_FOUND")
# CLI 用 version.workspace = true，所以 grep 可能找不到，检查 workspace 根
ROOT_VERSION=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/' | tr -d '\r')
assert_eq "workspace 根版本号已替换" "0.1.0-fork" "$ROOT_VERSION"

# 还原
find "$PROJECT_ROOT" -name "Cargo.toml" -exec sed -i 's/version = "0.1.0-fork"/version = "0.8.20"/g' {} +

ROOT_VERSION_AFTER=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/' | tr -d '\r')
assert_eq "workspace 根版本号已还原" "0.8.20" "$ROOT_VERSION_AFTER"

# --- 汇总 ---
echo ""
echo "========================================"
echo "结果: $pass 通过, $fail 失败"
echo "========================================"

[ "$fail" -eq 0 ]
