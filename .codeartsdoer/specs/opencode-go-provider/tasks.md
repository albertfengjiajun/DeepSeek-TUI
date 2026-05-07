# OpenCode Go 供应商集成 — 编码任务规划

## 1. ApiProvider 枚举扩展

- [ ] 在 `crates/tui/src/config.rs` 中添加两个常量：`DEFAULT_OPENCODE_GO_MODEL`（值 `"deepseek-v4-pro"`）和 `DEFAULT_OPENCODE_GO_BASE_URL`（值 `"https://opencode.ai/zen/go/v1"`），放置在现有供应商常量块之后
- [ ] 在 `ApiProvider` 枚举中新增 `OpencodeGo` 变体，位于 `Vllm` 之后
- [ ] 在 `ApiProvider::parse()` 中添加分支 `"opencode-go" | "opencode_go" => Some(Self::OpencodeGo)`
- [ ] 在 `ApiProvider::as_str()` 中添加分支 `Self::OpencodeGo => "opencode-go"`
- [ ] 在 `ApiProvider::display_name()` 中添加分支 `Self::OpencodeGo => "OpenCode Go"`
- [ ] 在 `ApiProvider::all()` 列表末尾追加 `Self::OpencodeGo`
- [ ] 验收：`cargo check` 通过，编译器报出的所有 `non-exhaustive patterns` 错误已记录

## 2. ProvidersConfig 结构体与配置合并

- [ ] 在 `ProvidersConfig` 结构体中新增字段 `#[serde(default)] pub opencode_go: ProviderConfig`，位于 `vllm` 之后
- [ ] 在 `provider_config_for()` 的 match 中添加分支 `ApiProvider::OpencodeGo => &providers.opencode_go`
- [ ] 在 `default_model()` 的默认模型 match 中添加分支 `ApiProvider::OpencodeGo => DEFAULT_OPENCODE_GO_MODEL`
- [ ] 在 `merge_providers()` 函数的 `ProvidersConfig` 构造中添加 `opencode_go: merge_provider_config(base.opencode_go, override_cfg.opencode_go)`
- [ ] 在环境变量模型覆盖解析处（`apply_env_overrides` 中针对各供应商的 model 环境变量覆盖逻辑），添加 `OpencodeGo` 分支处理 `OPENCODE_GO_MODEL` 环境变量
- [ ] 验收：`cargo check` 通过

## 3. Base URL 与 API Key 路由

- [ ] 在 `deepseek_base_url()` 的 `root_base` match 中添加分支 `ApiProvider::OpencodeGo => None`
- [ ] 在 `deepseek_base_url()` 的默认 URL match 中添加分支 `ApiProvider::OpencodeGo => DEFAULT_OPENCODE_GO_BASE_URL`
- [ ] 在 `deepseek_api_key()` 的 slot match 中添加分支 `ApiProvider::OpencodeGo => "opencode-go"`
- [ ] 在 `deepseek_api_key()` 的错误消息 match 中添加 `ApiProvider::OpencodeGo` 分支，提示信息包含 `opencode-go`、`OPENCODE_GO_API_KEY`、`providers.opencode_go`
- [ ] 验收：`cargo check` 通过

## 4. has_api_key_for 与 save_api_key_for 扩展

- [ ] 在 `has_api_key_for()` 的 env_var match 中添加分支 `ApiProvider::OpencodeGo => "OPENCODE_GO_API_KEY"`
- [ ] 在 `save_api_key_for()` 的 table_name match 中添加分支 `ApiProvider::OpencodeGo => "providers.opencode_go"`
- [ ] 在 `save_api_key_for()` 的 key_inside match 中添加分支 `ApiProvider::OpencodeGo => "opencode_go"`
- [ ] 验收：`cargo check` 通过

## 5. 模型映射与规范化

- [ ] 在 `model_for_provider()` 函数的 match 中添加前缀去除分支：`(ApiProvider::OpencodeGo, model) if model.starts_with("opencode-go/") => model.strip_prefix("opencode-go/").unwrap_or(model).to_string()`（放置在现有具体模型重映射之后、`_ => normalized` 之前）
- [ ] 在 `normalize_model_name()` 函数中扩展验证逻辑：允许 `"opencode-go/"` 前缀的模型名和已知 OpenCode Go 模型前缀（`glm-`、`kimi-`、`mimo-`、`minimax-`、`qwen`）通过验证
- [ ] 验收：`cargo check` 通过

## 6. 能力矩阵

- [ ] 新增 `opencode_go_capability()` 辅助函数，返回 `ProviderCapability`，内部调用三个子函数
- [ ] 新增 `opencode_go_context_window()` 函数：1M 上下文（deepseek-v4-*、mimo-*、qwen*）、256K（kimi-*）、200K（glm-*、minimax-m2.7）、192K（minimax-m2.5）、默认 128K
- [ ] 新增 `opencode_go_max_output()` 函数：按精确匹配和前缀匹配返回各模型的最大输出 token 值
- [ ] 新增 `opencode_go_thinking_supported()` 函数：除 mimo-v2-omni 和 mimo-v2.5 外均为 true
- [ ] 在 `provider_capability()` 函数顶部添加 OpencodeGo 分支：`if provider == ApiProvider::OpencodeGo { return opencode_go_capability(resolved_model); }`
- [ ] 验收：`cargo check` 通过

## 7. secrets 环境变量映射

- [ ] 在 `crates/secrets/src/lib.rs` 的 `env_for()` 函数中添加分支 `"opencode-go" | "opencode_go" => &["OPENCODE_GO_API_KEY"]`（放置在 `"vllm" | "v-llm"` 之后、`"openai"` 之前）
- [ ] 验收：`cargo check` 通过

## 8. 供应商选择器与命令消息

- [ ] 在 `crates/tui/src/tui/provider_picker.rs` 的 `env_var_for()` match 中添加分支 `ApiProvider::OpencodeGo => "OPENCODE_GO_API_KEY"`
- [ ] 在 `crates/tui/src/commands/provider.rs` 的错误消息中更新供应商列表，将 `"or vllm."` 改为 `"vllm, or opencode-go."`
- [ ] 验收：`cargo check` 通过

## 9. models.rs 可选扩展

- [ ] 在 `crates/tui/src/models.rs` 的 `context_window_for_model()` 函数中，在 `claude` 分支之后、`None` 之前添加 OpenCode Go 模型前缀识别：`glm-` → 204_800、`kimi-` → 262_144、`mimo-` → 1_048_576、`minimax-` → 204_800、`qwen` → 1_048_576
- [ ] 验收：`cargo check` 通过

## 10. 测试更新

- [ ] 更新 `provider_picker.rs` 中的 `picker_lists_all_seven_providers` 测试：在 `vec!` 列表末尾追加 `"OpenCode Go"`，并考虑将函数重命名为 `picker_lists_all_providers`
- [ ] 在 `config.rs` 测试模块中新增 `parse_opencode_go_aliases`：验证 `parse("opencode-go")` 和 `parse("opencode_go")` 返回 `Some(OpencodeGo)`
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_as_str`：验证 `OpencodeGo.as_str()` 返回 `"opencode-go"`
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_display_name`：验证 `OpencodeGo.display_name()` 返回 `"OpenCode Go"`
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_default_model_and_base_url`：验证默认模型为 `"deepseek-v4-pro"`，默认 URL 为 `"https://opencode.ai/zen/go/v1"`
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_model_for_provider_strips_prefix`：验证 `model_for_provider(OpencodeGo, "opencode-go/glm-5.1")` 返回 `"glm-5.1"`
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_model_for_provider_passthrough`：验证 `model_for_provider(OpencodeGo, "deepseek-v4-pro")` 透传
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_capability_v4_pro`：验证 context_window=1_048_576、max_output=393_216、thinking=true、cache=true
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_capability_glm51`：验证 context_window=204_800、max_output=131_072、thinking=true、cache=false
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_capability_kimi_k26`：验证 context_window=262_144、max_output=98_304、thinking=true
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_capability_mimo_v2_omni_no_thinking`：验证 thinking=false
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_capability_minimax_m25`：验证 context_window=196_608、max_output=32_768
- [ ] 在 `config.rs` 测试模块中新增 `opencode_go_capability_qwen36_plus`：验证 context_window=1_048_576、max_output=65_536
- [ ] 在 `secrets` 测试模块中新增 `env_for_opencode_go`：验证 `env_for("opencode-go")` 在设置 `OPENCODE_GO_API_KEY` 环境变量后返回正确值
- [ ] 在 `provider_picker` 测试模块中新增 `picker_includes_opencode_go`：验证 `ApiProvider::all()` 包含 `OpencodeGo`
- [ ] 在 `provider.rs` 测试模块中新增 `switch_to_opencode_go_emits_action`：验证 `/provider opencode-go` 返回 `SwitchProvider { provider: OpencodeGo, model: None }`
- [ ] 在 `provider.rs` 测试模块中新增 `unknown_provider_error_includes_opencode_go`：验证错误消息包含 `"opencode-go"`
- [ ] 验收：`cargo test --workspace --all-features` 全部通过

## 11. 最终验证

- [ ] 运行 `cargo clippy --workspace --all-targets --all-features`，确认无新增警告
- [ ] 运行 `cargo fmt --all --check`，确认代码格式符合规范
- [ ] 确认 `cargo build` 在稳定版 Rust 1.88+ 上编译通过，无 nightly 特性
- [ ] 验证现有供应商（DeepSeek、NVIDIA NIM、OpenRouter 等）的行为未被影响
