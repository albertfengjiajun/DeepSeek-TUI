# 添加自定义模型提供商功能 — 现状分析报告

## 版本信息

| 项目 | 值 |
|------|-----|
| 项目版本 | v0.8.20 |
| 当前提交 | f183501 fix(client): route non-beta paths from beta base to v1 (#1174) |
| 分支 | codearts/custom-model-provider |
| Rust 版本 | 1.93.0 |
| Edition | 2024 |

---

## 一、总体结论

DeepSeek-TUI 已实现了一套较为完整的多提供商支持体系，但采用**编译期枚举扩展模式**。用户无法在运行时或通过配置动态添加一个全新的、不在代码枚举中的提供商。要实现"用户运行时动态添加自定义提供商"功能，需引入全新的动态注册机制。

---

## 二、与上次分析（v0.8.16）的变化对比

| 对比项 | v0.8.16 (f97604c) | v0.8.20 (f183501) | 变化 |
|--------|-------------------|-------------------|------|
| ApiProvider 变体数 | 10 | 10 | **无变化** |
| ProviderKind 变体数 | 9 | 9 | **无变化** |
| ProvidersConfig | 静态结构体，10个固定字段 | 静态结构体，10个固定字段 | **无变化** |
| ProviderRegistry/CustomProvider | 不存在 | 不存在 | **无变化** |
| 动态 TOML 段落 | 不支持 | 不支持 | **无变化** |
| TUI 添加自定义提供商 | 不支持 | 不支持 | **无变化** |

v0.8.17~v0.8.20 期间的提供商相关修复（bug fix，无结构性变化）：
- `a0ef401` fix(config): prefer provider API keys over root key
- `4aee8a1` fix(config): preserve OpenRouter custom endpoint models
- `5c112b4` fix(config): default deepseek-cn to official api.deepseek.com
- `f183501` fix(client): route non-beta paths from beta base to v1

---

## 三、已实现的多提供商支持

### 3.1 预定义提供商列表（10个）

**文件**: `crates/tui/src/config.rs:64-75`

```rust
pub enum ApiProvider {
    Deepseek,
    DeepseekCN,
    NvidiaNim,
    Openai,        // OpenAI-compatible 通用端点
    Openrouter,
    Novita,
    Fireworks,
    Sglang,        // 自托管 OpenAI 兼容
    Vllm,          // 自托管 vLLM
    Ollama,        // 自托管 Ollama
}
```

`ApiProvider::parse()` 方法（第79-95行）只识别上述固定名称，未知名称返回 `None`。

**文件**: `crates/config/src/lib.rs:43-54` 存在对应的 `ProviderKind` 枚举（缺少 `DeepseekCN`），两套枚举需保持同步。

### 3.2 每个提供商的配置结构

**文件**: `crates/tui/src/config.rs:1022-1043`

```rust
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub http_headers: Option<HashMap<String, String>>,
}

pub struct ProvidersConfig {
    pub deepseek: ProviderConfig,
    pub deepseek_cn: ProviderConfig,
    pub nvidia_nim: ProviderConfig,
    pub openai: ProviderConfig,
    pub openrouter: ProviderConfig,
    pub novita: ProviderConfig,
    pub fireworks: ProviderConfig,
    pub sglang: ProviderConfig,
    pub vllm: ProviderConfig,
    pub ollama: ProviderConfig,
}
```

`ProvidersConfig` 是静态结构体，字段在编译期固定，无法通过 TOML 配置添加新的提供商段落。

### 3.3 配置文件示例

**文件**: `config.example.toml:168-208`

每个预定义提供商支持 `[providers.<name>]` 段落，包含 `api_key`、`base_url`、`model`、`http_headers` 字段。

### 3.4 提供商能力矩阵

**文件**: `crates/tui/src/config.rs:157-178`

```rust
pub struct ProviderCapability {
    pub provider: ApiProvider,
    pub resolved_model: String,
    pub context_window: u32,
    pub max_output: u32,
    pub thinking_supported: bool,
    pub cache_telemetry_supported: bool,
    pub request_payload_mode: RequestPayloadMode,
    pub alias_deprecation: Option<ModelAliasDeprecation>,
}
```

`provider_capability()` 函数通过 match 分支为每个提供商+模型组合返回静态能力信息。

### 3.5 TUI 提供商选择器

**文件**: `crates/tui/src/tui/provider_picker.rs`

完整的模态弹窗 UI，两阶段交互：
1. 列表阶段 — 显示所有 `ApiProvider::all()` 中的提供商
2. 密钥输入阶段 — 对于未配置密钥的提供商，转入内联 API Key 输入

关键代码（第49-53行）：
```rust
let providers: Vec<(ApiProvider, bool)> = ApiProvider::all()
    .iter()
    .map(|p| (*p, has_api_key_for(config, *p)))
    .collect();
```

### 3.6 `/provider` 命令

**文件**: `crates/tui/src/commands/provider.rs`

支持 `/provider openai`、`/provider ollama qwen2.5-coder:7b` 等用法。

### 3.7 CLI 支持

**文件**: `crates/cli/src/lib.rs`

- `--provider <ProviderArg>` 命令行参数
- `deepseek auth set --provider <name> --api-key <key>`
- `deepseek auth status` / `deepseek auth list`
- `deepseek model list --provider <name>`

---

## 四、OpenAI-Compatible 提供商 — 最接近"自定义"的功能

`ApiProvider::Openai` 是当前最灵活的选项：

| 特性 | 支持情况 |
|------|---------|
| 自定义 base_url | 通过 `OPENAI_BASE_URL` 环境变量或 `[providers.openai]` 配置 |
| 自定义 model ID | 直接透传，不做规范化 |
| 自定义 HTTP 头 | 支持 `http_headers` |
| 自定义显示名称 | **不支持**，固定显示 "OpenAI-compatible" |
| 多端点并行 | **不支持**，只有一个 `[providers.openai]` 段落 |
| 能力声明 | 保守默认值（context: 128K, max_output: 4096, thinking: false） |

---

## 五、未实现的功能（关键差距）

| # | 缺失功能 | 说明 |
|---|---------|------|
| 1 | 动态提供商注册 | 不存在 `ProviderRegistry`/`CustomProvider`/`register_provider` 机制 |
| 2 | 用户自定义提供商 UI | 无法在 TUI 中"添加新提供商" |
| 3 | 动态配置结构 | `ProvidersConfig` 和 `ApiProvider` 均为编译期固定 |
| 4 | 动态 TOML 段落 | 不支持 `[providers."my-custom"]` 格式 |
| 5 | 穷举匹配扩展 | 所有 match 分支需手动扩展，无法自动处理新提供商 |

---

## 六、新增提供商当前需修改的文件清单

| 阶段 | 文件 | 修改内容 |
|------|------|---------|
| 1 | `crates/tui/src/config.rs` | `ApiProvider` 枚举 + 常量 + `parse()/as_str()/display_name()/all()` |
| 2 | `crates/config/src/lib.rs` | `ProviderKind` 枚举 + 所有 match 分支 |
| 3 | `crates/tui/src/config.rs` | `ProvidersConfig` 字段 + `get_value/set_value` + 运行时选项 |
| 4 | `crates/tui/src/config.rs` | `provider_capability()` + `has_api_key_for()` |
| 5 | `provider_picker.rs` + `commands/provider.rs` | UI 映射 + 错误提示 |
| 6 | `config.rs` + `config.example.toml` | 环境变量覆盖 + 配置示例 |

---

## 七、建议实现方向

### 方案A：动态提供商注册（推荐）

引入 `CustomProvider` 结构体和 `ProviderRegistry`，支持：

- TOML 配置 `[providers.custom.<name>]` 动态段落
- TUI 中"添加自定义提供商"界面
- 运行时注册/切换/删除自定义提供商
- 自定义显示名称、base_url、API key、模型列表
- 与预定义提供商统一调度

核心数据结构设计：

```rust
pub struct CustomProvider {
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub http_headers: Option<HashMap<String, String>>,
    pub context_window: Option<u32>,
    pub max_output: Option<u32>,
    pub thinking_supported: Option<bool>,
}

pub struct ProviderRegistry {
    pub builtin: Vec<ApiProvider>,
    pub custom: HashMap<String, CustomProvider>,
}
```

### 方案B：简化版 — 扩展 OpenAI-Compatible

支持多个 `[providers.openai_compatible.<name>]` 段落，每个段落可自定义显示名称和 base_url，复用 OpenAI 兼容协议。改动较小但灵活性有限。

---

## 八、风险与注意事项

1. **向后兼容**：动态提供商的 TOML 配置需与现有 `[providers.<builtin>]` 段落兼容，不能破坏现有配置文件
2. **枚举统一**：`ApiProvider` 和 `ProviderKind` 两套枚举需同步改造，或合并为统一接口
3. **能力矩阵**：自定义提供商的能力信息需由用户配置或自动探测
4. **Provider Picker UI**：需支持动态列表渲染和自定义提供商的增删操作
5. **API 兼容性**：所有自定义提供商必须兼容 OpenAI Chat Completions API 格式
