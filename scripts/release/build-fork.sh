#!/usr/bin/env bash
# Fork 发布构建脚本：
#   1. 临时将 bin name 从 deepseek → dstui
#   2. 临时将版本号从上游 → fork 版本（批量替换所有 Cargo.toml）
#   3. 构建 release
#   4. 还原所有改动（trap EXIT + git checkout）
#
# 版本号替换策略：
#   workspace 根的 version = "X.Y.Z" 和所有依赖声明中的 version = "X.Y.Z"
#   一起替换，一条 sed 搞定。因为 workspace 内 path 依赖的 version 必须
#   与实际 crate 版本一致，cargo 才能解析。
#
# 用法: bash scripts/release/build-fork.sh [target]
#   target 可选，默认为当前 host

set -euo pipefail

FORK_BIN_NAME="dstui"
FORK_VERSION="0.2.0-fork"
UPSTREAM_BIN_NAME="deepseek"

CLI_TOML="crates/cli/Cargo.toml"

# 读取上游版本号
UPSTREAM_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/' | tr -d '\r')
echo "[fork-build] 上游版本: $UPSTREAM_VERSION → Fork 版本: $FORK_VERSION"

# --- 替换 bin name ---
echo "[fork-build] 替换 bin name: $UPSTREAM_BIN_NAME → $FORK_BIN_NAME"
if [ -f "$CLI_TOML" ]; then
    sed -i "s/name = \"$UPSTREAM_BIN_NAME\"/name = \"$FORK_BIN_NAME\"/" "$CLI_TOML"
fi

# --- 替换版本号（所有 Cargo.toml 中的 version = "X.Y.Z"）---
echo "[fork-build] 替换版本号: $UPSTREAM_VERSION → $FORK_VERSION"
find . -name "Cargo.toml" -exec sed -i "s/version = \"$UPSTREAM_VERSION\"/version = \"$FORK_VERSION\"/g" {} +

# --- 还原 trap ---
restore() {
    echo "[fork-build] 还原 bin name: $FORK_BIN_NAME → $UPSTREAM_BIN_NAME"
    if [ -f "$CLI_TOML" ]; then
        sed -i "s/name = \"$FORK_BIN_NAME\"/name = \"$UPSTREAM_BIN_NAME\"/" "$CLI_TOML"
    fi
    echo "[fork-build] 还原版本号: $FORK_VERSION → $UPSTREAM_VERSION"
    find . -name "Cargo.toml" -exec sed -i "s/version = \"$FORK_VERSION\"/version = \"$UPSTREAM_VERSION\"/g" {} +
}
trap restore EXIT

# --- 构建 ---
echo "[fork-build] 开始 cargo build --release"
if [ -n "${1:-}" ]; then
    cargo build --release --target "$1"
    BIN_PATH="target/$1/release/$FORK_BIN_NAME"
else
    cargo build --release
    BIN_PATH="target/release/$FORK_BIN_NAME"
fi

echo "[fork-build] 构建完成: $BIN_PATH (v$FORK_VERSION)"
