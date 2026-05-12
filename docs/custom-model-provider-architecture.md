# 自定义模型提供商架构扩展

> 本文档是 `docs/ARCHITECTURE.md` 的扩展，描述自定义模型提供商功能。独立维护以最小化对上游文件的改动。

---

## LLM 集成扩展

### 自定义模型提供商

TUI 支持在内置提供商之外定义用户自定义提供商，通过 `config.toml` 中的 `[providers.<name>]` 配置（含 `protocol` 字段即视为自定义提供商，不含则为内置提供商覆盖）。支持三种 API protocol：

- **`openai_compatible`** — 复用现有 `DeepSeekClient` Chat Completions 路径
- **`anthropic_messages`** — Anthropic `/v1/messages` 请求格式，SSE 流式
- **`google_gemini`** — Google Gemini `generateContent` 请求格式，NDJSON 流式

核心模块（均为新建文件，不修改已有文件）：

| 模块 | 用途 |
|------|------|
| `provider_registry.rs` | `ProviderRegistry` — 注册、解析、查询 builtin 和 custom provider。持有 `ResolvedProvider` 条目，包含 protocol、base URL、API key 和 `ModelCapabilities` |
| `capability_bridge.rs` | `CapabilityBridge` — 将 `ModelCapabilities`（Option-based、用户可配）桥接到 `ProviderCapability`（具体值）。提供三级解析：用户配置 → 内置默认 → protocol 默认。同时映射 `ApiProvider` → `ProtocolType` 及模型名称启发式 → 能力标记 |
| `capability_filter.rs` | `CapabilityFilter` — 基于已解析能力剥离请求中不支持的字段（如不支持 tool 的模型上去掉 tool_calls） |
| `protocol_adapter.rs` | `ProtocolAdapter` trait + `ProtocolAdapterFactory` — 将非流式和流式请求分发到正确的 protocol handler |

`config.rs` 中的 `provider_capability()` 函数现在委托到 `CapabilityBridge` 而非硬编码模型名称匹配，使能力系统对自定义提供商可扩展。

### 配置扩展

在 `### Configuration` 部分新增以下条目：

- **`provider_registry.rs`** — Provider 注册表，管理 builtin + custom provider，TOML 加载
- **`capability_bridge.rs`** — 能力解析桥接（用户 → 内置 → protocol 默认）
- **`capability_filter.rs`** — 基于已解析能力的请求过滤
- **`protocol_adapter.rs`** — Protocol 适配器 trait 和工厂（OpenAI / Anthropic / Gemini）

---

## 自定义提供商配置

在 `config.toml` 中添加 `[providers.<name>]` 段来定义自定义提供商（含 `protocol` 字段）。每条至少需要 `base_url`；`protocol` 省略时默认为 `openai_compatible`。

```toml
[providers.my-gpt]
protocol = "openai_compatible"      # 含 protocol → 自定义提供商
base_url = "https://api.my-gpt.com/v1"
api_key = "sk-..."
model = "gpt-4o"
display_name = "My GPT Proxy"

[providers.claude-direct]
protocol = "anthropic_messages"
base_url = "https://api.anthropic.com"
api_key = "sk-ant-..."
model = "claude-sonnet-4-20250514"

[providers.gemini-direct]
protocol = "google_gemini"
base_url = "https://generativelanguage.googleapis.com"
api_key = "AIza..."
model = "gemini-2.0-flash"
```

可选能力覆盖（省略时均使用 protocol 默认值）：

```toml
[providers.my-gpt]
base_url = "https://api.my-gpt.com/v1"
[providers.my-gpt.capabilities]
thinking_supported = false
supports_tools = true
context_window = 128000
max_output = 4096
```

自定义提供商与内置提供商一同出现在 `/provider` 选择器中。选中后切换活跃的 API endpoint、model 和 request payload 格式。API key 持久化到同一 `config.toml` 的 `[providers.<name>]` 下。

---

## 对上游现有文件的修改

以下对上游文件的修改是必要的，已尽量保持最小化和增量式：

| 文件 | 修改类型 | 描述 |
|------|----------|------|
| `config.rs` | 新增 enum 变体 | `ProtocolType`、`RequestPayloadMode` 各扩展 2 个变体 |
| `config.rs` | 新增 struct 字段 | `ProvidersConfig` 上添加 `#[serde(flatten)] custom: HashMap<String, ProviderTomlConfig>` |
| `config.rs` | 重构函数 | `provider_capability()` 委托到 `CapabilityBridge` |
| `config.rs` | 可见性变更 | `deepseek_alias_deprecation()` → `pub(crate)` |
| `config.rs` | 新增函数 | `save_api_key_for_custom_provider()` |
| `models.rs` | 新增 struct | `ModelCapabilities` 含 9 个 Option 字段 |
| `client/chat.rs` | 可见性变更 | 3 个函数 `pub(super)` → `pub(crate)` |
| `client/chat.rs` | 新增函数 | 提取 `openai_sse_stream_from_response()` |
| `client.rs` | 可见性变更 | 子模块 `anthropic`、`chat`、`gemini` → `pub(crate)` |
| `main.rs` | 新增模块注册 | `capability_bridge`、`capability_filter`、`provider_registry`、`protocol_adapter` |
| `tui/app.rs` | 新增字段 | `active_provider_name`、`provider_registry`（含懒初始化） |
| `tui/provider_picker.rs` | 重写 | 基于 `ProviderRegistry` 的动态列表 |
| `tui/ui.rs` | 新增函数 | `switch_to_custom_provider()` |
| `tui/ui.rs` | 修改 handler | `ProviderPickerApplied` / `ApiKeySubmitted` → custom 路径 |
| `tui/views/mod.rs` | 变更类型 | `ViewEvent` provider 字段 → `String` |
| `cli/lib.rs` | 新增子命令 | `Provider(TuiPassthroughArgs)` |
