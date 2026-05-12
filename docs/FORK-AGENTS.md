# Fork 核心指令 — DeepSeek-TUI

> 本文件为 AI 助手提供 fork 开发的核心指令，确保与上游 `Hmbown/DeepSeek-TUI` 完全独立、可持续共存。

---

## 核心指令

### 语言要求
- **所有交互、文档、注释使用简体中文，专业术语保留英文原文**
- 示例：使用 `ProviderRegistry`（而非"提供商注册表"）进行 custom provider 的注册与解析
- 代码中的 `//` 注释使用简体中文，`///` doc comment 使用英文（Rust doc 惯例）

### 核心原则：最小化上游文件修改

**扩展项目时，优先新增文件而非修改上游已有文件。** 这降低与 upstream 同步时的 merge conflict 风险，保持 fork 可维护性。

### 指导规则

| # | 规则 | 理由 |
|---|------|------|
| 1 | **新功能放新文件** | 新 module、trait、struct、function 尽量放在独立新文件中 |
| 2 | **扩展而非替换** | 为已有类型添加功能时，优先级：新 wrapper type > trait extension > 添加 field/method > 重写已有代码 |
| 3 | **可见性变更可接受** | `pub(super)` → `pub(crate)` 风险低，unlikely to conflict |
| 4 | **新增 struct field 可接受** | 添加 `Option<NewType>` 字段并标注 `#[serde(default)]`，属于 additive、backward-compatible |
| 5 | **函数重写高风险** | 重写已有函数（如 `provider_capability()`）是最后手段。若不可避免，立即委托到新文件 |
| 6 | **文档放独立文件** | fork 扩展的架构/设计文档放在 `docs/<feature>-architecture.md`，不修改上游的 `docs/ARCHITECTURE.md` |

### 上游冲突不可避免时

部分改动必须修改上游文件（如在 `main.rs` 注册新 module，在 `ui.rs` 添加 handler 分支）：

- 保持修改**尽可能小且局部化**
- 添加注释引用扩展文档：`// 参见 docs/custom-model-provider-architecture.md`
- 频繁 rebase 以尽早发现冲突

---

## 与上游完全独立

### CLI 命令改名（发布时处理，不修改源码）

上游安装后在终端的命令是 `deepseek`。**本 fork 必须使用不同的命令名，避免覆盖上游安装。**

**策略：源码中保持 `deepseek` 不变，仅在发布构建时通过脚本改名为 `dstui`，构建完成后还原。**

- **fork 命令名**：`dstui`（DeepSeek-TUI 缩写，简短且不与上游冲突）
- **发布构建脚本** `scripts/release/build-fork.sh`（已创建）：
  - sed 临时替换 Cargo.toml 中的 `name = "deepseek"` → `name = "dstui"`
  - 执行 `cargo build --release`
  - trap EXIT 确保即使构建失败也还原 Cargo.toml
- **优势**：
  - 源码零改动，与上游同步无冲突
  - `cargo test`、`cargo clippy` 在开发时仍用 `deepseek` 名称
  - 仅发布产物名为 `dstui`，安装后不覆盖上游
- **用户安装**：`cargo install --git https://github.com/albertfengjiajun/DeepSeek-TUI --bin dstui` 或直接下载 Release 中的 `dstui` 二进制

### 版本发布独立

- **版本号体系独立于上游**：上游使用 `v0.8.x` 系列，本 fork 使用 `v0.1.0-fork` 起始
- **Cargo package version 独立维护**：在各 `Cargo.toml` 中独立递增
- **发布渠道独立**：
  - 不向上游 `crates.io` 发布（crate name 不同）
  - GitHub Release 在 fork 仓库独立发布
  - `cargo install` 使用 `--git` 或 `--path` 方式，不与上游 crate 混淆
- **CHANGELOG 独立**：`docs/FORK-CHANGELOG.md`，不修改上游 `CHANGELOG.md`

### 配置目录隔离（运行时处理，不修改源码）

- 上游默认配置目录：`~/.deepseek/`
- **fork 默认配置目录**：`~/.dstui/`（避免读写上游的 config/session/data）
- **策略：不修改源码中的路径，通过启动 wrapper 脚本设置环境变量**
  - wrapper 脚本 `scripts/release/dstui-wrapper.sh`（已创建）
  - 设置 `DSTUI_HOME`（默认 `~/.dstui`）和 `DEEPSEEK_CONFIG_PATH`
  - 自动创建 `~/.dstui/` 目录
  - exec 启动 `dstui` 二进制
- **优势**：源码零改动，仅通过环境变量在运行时覆盖配置路径，rebase 时零冲突

---

## 上游同步策略

### 仓库结构

```
origin   → https://github.com/albertfengjiajun/DeepSeek-TUI.git  (fork)
upstream → https://github.com/Hmbown/DeepSeek-TUI.git             (upstream)
```

### 定期同步工作流

```bash
# 1. 拉取上游变更
git fetch upstream

# 2. 将 feature branch rebase 到 upstream/main
git checkout codearts/custom-model-provider
git rebase upstream/main

# 3. 解决冲突（重点关注"冲突高危文件"表中列出的文件）

# 4. Force-push rebase 后的分支
git push origin codearts/custom-model-provider --force-with-lease

# 5. 完整验证
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
```

### 冲突高危文件

| 文件 | 风险等级 | 原因 |
|------|----------|------|
| `crates/tui/src/config.rs` | **高** | 上游频繁添加 provider-specific 逻辑；`provider_capability()` 重构和 `ProvidersConfig.custom` flatten 字段可能冲突 |
| `crates/tui/src/tui/ui.rs` | **高** | 大文件，UI 变更频繁；`switch_to_custom_provider()` 和 `ProviderPickerApplied` handler 可能冲突 |
| `crates/tui/src/main.rs` | **中** | module 注册属 additive，但上游可能重组 |
| `crates/tui/src/client/chat.rs` | **中** | 可见性变更和函数提取可能冲突 |
| `crates/tui/src/models.rs` | **低** | 纯新增 `ModelCapabilities` struct，unlikely to conflict |
| `crates/tui/src/tui/app.rs` | **低** | 纯新增字段，unlikely to conflict |

### 零冲突文件（新建文件）

以下文件从零创建，永远不会与上游冲突：

- `crates/tui/src/capability_bridge.rs`
- `crates/tui/src/capability_filter.rs`
- `crates/tui/src/protocol_adapter.rs`
- `crates/tui/src/provider_registry.rs`
- `crates/tui/src/client/anthropic.rs`
- `crates/tui/src/client/gemini.rs`
- `docs/custom-model-provider-architecture.md`
- `docs/FORK-AGENTS.md`（本文件）

---

## 未来重构机会

以下对上游文件的修改可在后续迭代中进一步隔离：

| 当前位置 | 改动内容 | 可行重构 |
|----------|----------|----------|
| `models.rs` | `ModelCapabilities` struct（+65 行，纯新增） | 移到独立 `model_capabilities.rs` 文件 |
| `config.rs` | `save_api_key_for_custom_provider()` | 移到 `provider_registry.rs` 或新 `custom_provider_persist.rs` |
| `config.rs` | `provider_capability()` 重构 | 已委托到 `CapabilityBridge`，无需进一步隔离 |

**决策**：这些重构 ROI 低（纯 addition 不引发冲突；persist 函数独立）。若上游开始频繁修改 `models.rs` 或 `save_api_key_for` 区域，则重新评估。

---

## 分支命名规范

- Fork feature branch：`codearts/<feature-name>`（如 `codearts/custom-model-provider`）
- `codearts` 前缀区分 fork 分支与上游分支

---

## 当前功能：自定义模型提供商（Custom Model Provider）

- **分支**：`codearts/custom-model-provider`
- **基线**：`upstream/main` at `f183501`
- **架构文档**：`docs/custom-model-provider-architecture.md`
- **规格文档**：`.codeartsdoer/specs/custom-model-provider/`

### 新增内容

1. 三种 API protocol：`openai_compatible`、`anthropic_messages`、`google_gemini`
2. `ProviderRegistry`：统一管理 builtin + custom provider
3. `CapabilityBridge`：三级能力解析（user config → builtin defaults → protocol defaults）
4. `ProtocolAdapter` trait：协议特定的 request dispatch
5. TOML `[providers.<name>]` 配置格式（含 `protocol` 字段即自定义提供商）
6. 完整的 provider 切换支持（picker 中包含 custom provider）
7. Custom provider 的 API key 持久化
8. 16 个新增单元测试
