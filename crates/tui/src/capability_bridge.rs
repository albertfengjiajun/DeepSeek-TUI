use crate::config::{ApiProvider, ProtocolType, ProviderCapability, RequestPayloadMode};
use crate::models::ModelCapabilities;

pub struct CapabilityBridge;

#[derive(Debug, Clone)]
struct ModelPattern {
    pattern: &'static str,
    caps: ModelCapabilities,
}

static DEEPSEEK_MODEL_PATTERNS: &[ModelPattern] = &[
    ModelPattern {
        pattern: "deepseek-v4-pro",
        caps: ModelCapabilities {
            thinking_supported: Some(true),
            supports_tools: None,
            supports_reasoning: Some(true),
            supports_vision: None,
            supports_streaming: None,
            supports_embedding: None,
            context_window: Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS),
            max_output: Some(384_000),
            cache_telemetry_supported: Some(true),
        },
    },
    ModelPattern {
        pattern: "deepseek-v4-flash",
        caps: ModelCapabilities {
            thinking_supported: Some(true),
            supports_tools: None,
            supports_reasoning: Some(false),
            supports_vision: None,
            supports_streaming: None,
            supports_embedding: None,
            context_window: Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS),
            max_output: Some(384_000),
            cache_telemetry_supported: Some(true),
        },
    },
    ModelPattern {
        pattern: "deepseek-reasoner",
        caps: ModelCapabilities {
            thinking_supported: Some(true),
            supports_tools: None,
            supports_reasoning: Some(true),
            supports_vision: None,
            supports_streaming: None,
            supports_embedding: None,
            context_window: Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS),
            max_output: Some(384_000),
            cache_telemetry_supported: Some(true),
        },
    },
    ModelPattern {
        pattern: "deepseek-chat",
        caps: ModelCapabilities {
            thinking_supported: Some(true),
            supports_tools: None,
            supports_reasoning: Some(false),
            supports_vision: None,
            supports_streaming: None,
            supports_embedding: None,
            context_window: Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS),
            max_output: Some(384_000),
            cache_telemetry_supported: Some(true),
        },
    },
];

impl CapabilityBridge {
    #[must_use]
    pub fn provider_to_protocol(provider: ApiProvider) -> ProtocolType {
        match provider {
            ApiProvider::Deepseek
            | ApiProvider::DeepseekCN
            | ApiProvider::NvidiaNim
            | ApiProvider::Openai
            | ApiProvider::Openrouter
            | ApiProvider::Novita
            | ApiProvider::Fireworks
            | ApiProvider::Sglang
            | ApiProvider::Vllm
            | ApiProvider::Ollama => ProtocolType::OpenaiCompatible,
        }
    }

    #[must_use]
    pub fn builtin_model_capabilities(
        provider: ApiProvider,
        resolved_model: &str,
    ) -> ModelCapabilities {
        let model_lower = resolved_model.to_ascii_lowercase();

        if matches!(provider, ApiProvider::Ollama) {
            return ModelCapabilities {
                context_window: Some(8_192),
                max_output: Some(4_096),
                thinking_supported: Some(false),
                cache_telemetry_supported: Some(false),
                ..ModelCapabilities::default()
            };
        }

        if matches!(provider, ApiProvider::Openai) {
            return ModelCapabilities {
                context_window: Some(crate::models::LEGACY_DEEPSEEK_CONTEXT_WINDOW_TOKENS),
                max_output: Some(4_096),
                thinking_supported: Some(false),
                cache_telemetry_supported: Some(false),
                ..ModelCapabilities::default()
            };
        }

        if let Some(caps) = lookup_model_pattern(&model_lower) {
            let cache_telemetry_supported = matches!(
                provider,
                ApiProvider::Deepseek | ApiProvider::DeepseekCN | ApiProvider::NvidiaNim
            );
            return ModelCapabilities {
                cache_telemetry_supported: Some(cache_telemetry_supported),
                ..caps.clone()
            };
        }

        let is_v4 = model_lower.contains("v4-pro")
            || model_lower.contains("v4-flash")
            || model_lower == "deepseek-v4pro"
            || model_lower == "deepseek-v4flash"
            || model_lower == "deepseek-v4"
            || crate::config::deepseek_alias_deprecation(&model_lower).is_some();

        let context_window = if is_v4 {
            Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS)
        } else {
            crate::models::context_window_for_model(resolved_model)
        };

        let max_output = if is_v4 { Some(384_000) } else { Some(4_096) };
        let thinking_supported = is_v4;
        let supports_reasoning = infer_supports_reasoning_from_model_id(&model_lower);
        let cache_telemetry_supported = matches!(
            provider,
            ApiProvider::Deepseek | ApiProvider::DeepseekCN | ApiProvider::NvidiaNim
        );

        ModelCapabilities {
            thinking_supported: Some(thinking_supported),
            supports_reasoning: Some(supports_reasoning),
            context_window,
            max_output,
            cache_telemetry_supported: Some(cache_telemetry_supported),
            ..ModelCapabilities::default()
        }
    }

    #[must_use]
    pub fn protocol_defaults(protocol: ProtocolType) -> ModelCapabilities {
        match protocol {
            ProtocolType::OpenaiCompatible => ModelCapabilities {
                supports_tools: Some(true),
                supports_streaming: Some(true),
                context_window: Some(128_000),
                max_output: Some(4_096),
                ..ModelCapabilities::default()
            },
            ProtocolType::AnthropicMessages => ModelCapabilities {
                supports_tools: Some(true),
                supports_streaming: Some(true),
                supports_vision: Some(true),
                thinking_supported: Some(false),
                supports_reasoning: Some(false),
                context_window: Some(200_000),
                max_output: Some(8_192),
                ..ModelCapabilities::default()
            },
            ProtocolType::GoogleGemini => ModelCapabilities {
                supports_tools: Some(true),
                supports_streaming: Some(true),
                supports_vision: Some(true),
                supports_embedding: Some(true),
                context_window: Some(1_000_000),
                max_output: Some(8_192),
                ..ModelCapabilities::default()
            },
        }
    }

    #[must_use]
    pub fn resolve_capabilities(
        user_caps: Option<&ModelCapabilities>,
        builtin_defaults: Option<&ModelCapabilities>,
        protocol: ProtocolType,
    ) -> ModelCapabilities {
        let protocol_defaults = Self::protocol_defaults(protocol);
        let with_builtin = match builtin_defaults {
            Some(bd) => bd.merge_with_defaults(&protocol_defaults),
            None => protocol_defaults,
        };
        match user_caps {
            Some(uc) => uc.merge_with_defaults(&with_builtin),
            None => with_builtin,
        }
    }

    #[must_use]
    pub fn request_payload_mode(protocol: ProtocolType) -> RequestPayloadMode {
        match protocol {
            ProtocolType::OpenaiCompatible => RequestPayloadMode::ChatCompletions,
            ProtocolType::AnthropicMessages => RequestPayloadMode::AnthropicMessages,
            ProtocolType::GoogleGemini => RequestPayloadMode::GoogleGemini,
        }
    }

    #[must_use]
    pub fn to_provider_capability(
        provider: ApiProvider,
        resolved_model: &str,
        caps: &ModelCapabilities,
        protocol: ProtocolType,
        alias_deprecation: Option<crate::config::ModelAliasDeprecation>,
    ) -> ProviderCapability {
        ProviderCapability {
            provider,
            resolved_model: resolved_model.to_string(),
            context_window: caps.context_window.unwrap_or(128_000),
            max_output: caps.max_output.unwrap_or(4_096),
            thinking_supported: caps.thinking_supported.unwrap_or(false),
            cache_telemetry_supported: caps.cache_telemetry_supported.unwrap_or(false),
            request_payload_mode: Self::request_payload_mode(protocol),
            alias_deprecation,
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn requires_reasoning(caps: &ModelCapabilities) -> bool {
        caps.thinking_supported.unwrap_or(false) || caps.supports_reasoning.unwrap_or(false)
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn should_replay_reasoning(caps: &ModelCapabilities, effort: Option<&str>) -> bool {
        if effort
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "off" | "disabled" | "none" | "false"
                )
            })
            .unwrap_or(false)
        {
            return false;
        }
        Self::requires_reasoning(caps)
    }
}

#[must_use]
fn lookup_model_pattern(model_lower: &str) -> Option<&'static ModelCapabilities> {
    for entry in DEEPSEEK_MODEL_PATTERNS {
        if model_lower.contains(entry.pattern) || model_lower == entry.pattern {
            return Some(&entry.caps);
        }
    }
    None
}

#[must_use]
#[allow(clippy::manual_c_bools)]
fn infer_supports_reasoning_from_model_id(model_lower: &str) -> bool {
    model_lower.contains("deepseek-v4")
        || model_lower.contains("reasoner")
        || model_lower.contains("-reasoning")
        || model_lower.contains("-thinking")
        || has_deepseek_r_series_marker(model_lower)
}

#[must_use]
fn has_deepseek_r_series_marker(model_lower: &str) -> bool {
    const PREFIX: &str = "deepseek-r";
    model_lower.match_indices(PREFIX).any(|(idx, _)| {
        model_lower[idx + PREFIX.len()..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApiProvider;

    #[test]
    fn provider_to_protocol_maps_all_builtins_to_openai_compatible() {
        for provider in [
            ApiProvider::Deepseek,
            ApiProvider::DeepseekCN,
            ApiProvider::NvidiaNim,
            ApiProvider::Openai,
            ApiProvider::Openrouter,
            ApiProvider::Novita,
            ApiProvider::Fireworks,
            ApiProvider::Sglang,
            ApiProvider::Vllm,
            ApiProvider::Ollama,
        ] {
            assert_eq!(
                CapabilityBridge::provider_to_protocol(provider),
                ProtocolType::OpenaiCompatible
            );
        }
    }

    #[test]
    fn builtin_model_capabilities_deepseek_v4_pro() {
        let caps =
            CapabilityBridge::builtin_model_capabilities(ApiProvider::Deepseek, "deepseek-v4-pro");
        assert_eq!(caps.thinking_supported, Some(true));
        assert_eq!(caps.supports_reasoning, Some(true));
        assert_eq!(
            caps.context_window,
            Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS)
        );
        assert_eq!(caps.max_output, Some(384_000));
        assert_eq!(caps.cache_telemetry_supported, Some(true));
    }

    #[test]
    fn builtin_model_capabilities_deepseek_v4_flash() {
        let caps = CapabilityBridge::builtin_model_capabilities(
            ApiProvider::Deepseek,
            "deepseek-v4-flash",
        );
        assert_eq!(caps.thinking_supported, Some(true));
        assert_eq!(
            caps.context_window,
            Some(crate::models::DEEPSEEK_V4_CONTEXT_WINDOW_TOKENS)
        );
        assert_eq!(caps.max_output, Some(384_000));
    }

    #[test]
    fn builtin_model_capabilities_ollama() {
        let caps = CapabilityBridge::builtin_model_capabilities(ApiProvider::Ollama, "llama3");
        assert_eq!(caps.context_window, Some(8_192));
        assert_eq!(caps.thinking_supported, Some(false));
        assert_eq!(caps.cache_telemetry_supported, Some(false));
    }

    #[test]
    fn builtin_model_capabilities_openai() {
        let caps = CapabilityBridge::builtin_model_capabilities(ApiProvider::Openai, "gpt-4o");
        assert_eq!(
            caps.context_window,
            Some(crate::models::LEGACY_DEEPSEEK_CONTEXT_WINDOW_TOKENS)
        );
        assert_eq!(caps.thinking_supported, Some(false));
    }

    #[test]
    fn infer_supports_reasoning_from_model_id() {
        assert!(super::infer_supports_reasoning_from_model_id(
            "deepseek-v4-pro"
        ));
        assert!(super::infer_supports_reasoning_from_model_id(
            "deepseek-v4-flash"
        ));
        assert!(super::infer_supports_reasoning_from_model_id(
            "deepseek-reasoner"
        ));
        assert!(super::infer_supports_reasoning_from_model_id(
            "qwen-reasoning"
        ));
        assert!(super::infer_supports_reasoning_from_model_id(
            "qwq-thinking"
        ));
        assert!(super::infer_supports_reasoning_from_model_id("deepseek-r1"));
        assert!(!super::infer_supports_reasoning_from_model_id("gpt-4o"));
        assert!(!super::infer_supports_reasoning_from_model_id(
            "claude-3-opus"
        ));
    }

    #[test]
    fn requires_reasoning_from_caps() {
        let caps_with_thinking = ModelCapabilities {
            thinking_supported: Some(true),
            ..ModelCapabilities::default()
        };
        assert!(CapabilityBridge::requires_reasoning(&caps_with_thinking));

        let caps_with_reasoning = ModelCapabilities {
            supports_reasoning: Some(true),
            ..ModelCapabilities::default()
        };
        assert!(CapabilityBridge::requires_reasoning(&caps_with_reasoning));

        let caps_none = ModelCapabilities::default();
        assert!(!CapabilityBridge::requires_reasoning(&caps_none));
    }

    #[test]
    fn should_replay_reasoning_respects_effort_off() {
        let caps = ModelCapabilities {
            thinking_supported: Some(true),
            ..ModelCapabilities::default()
        };
        assert!(!CapabilityBridge::should_replay_reasoning(
            &caps,
            Some("off")
        ));
        assert!(!CapabilityBridge::should_replay_reasoning(
            &caps,
            Some("disabled")
        ));
        assert!(CapabilityBridge::should_replay_reasoning(
            &caps,
            Some("high")
        ));
        assert!(CapabilityBridge::should_replay_reasoning(&caps, None));
    }

    #[test]
    fn resolve_capabilities_three_tier_merge() {
        let protocol_defaults = CapabilityBridge::protocol_defaults(ProtocolType::OpenaiCompatible);
        let user_caps = ModelCapabilities {
            context_window: Some(50_000),
            ..ModelCapabilities::default()
        };
        let resolved = CapabilityBridge::resolve_capabilities(
            Some(&user_caps),
            None,
            ProtocolType::OpenaiCompatible,
        );
        assert_eq!(resolved.context_window, Some(50_000));
        assert_eq!(resolved.supports_tools, protocol_defaults.supports_tools);
        assert_eq!(
            resolved.supports_streaming,
            protocol_defaults.supports_streaming
        );
    }

    #[test]
    fn lookup_model_pattern_matches_known_models() {
        assert!(super::lookup_model_pattern("deepseek-v4-pro").is_some());
        assert!(super::lookup_model_pattern("deepseek-v4-flash").is_some());
        assert!(super::lookup_model_pattern("deepseek-reasoner").is_some());
        assert!(super::lookup_model_pattern("deepseek-chat").is_some());
    }

    #[test]
    fn lookup_model_pattern_returns_none_for_unknown() {
        assert!(super::lookup_model_pattern("gpt-4o").is_none());
        assert!(super::lookup_model_pattern("claude-3-opus").is_none());
        assert!(super::lookup_model_pattern("llama3").is_none());
    }

    #[test]
    fn lookup_model_pattern_v4_pro_caps() {
        let caps = super::lookup_model_pattern("deepseek-v4-pro").unwrap();
        assert_eq!(caps.thinking_supported, Some(true));
        assert_eq!(caps.supports_reasoning, Some(true));
        assert_eq!(caps.max_output, Some(384_000));
    }

    #[test]
    fn lookup_model_pattern_v4_flash_caps() {
        let caps = super::lookup_model_pattern("deepseek-v4-flash").unwrap();
        assert_eq!(caps.thinking_supported, Some(true));
        assert_eq!(caps.supports_reasoning, Some(false));
    }
}
