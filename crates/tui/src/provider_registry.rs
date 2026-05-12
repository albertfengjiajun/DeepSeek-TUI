use crate::config::ProtocolType;
use crate::models::ModelCapabilities;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const DEEPSEEK_DEFAULT_BASE_URL: &str = "https://api.deepseek.com/beta";
pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const OPENROUTER_DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const NVIDIA_NIM_DEFAULT_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";
pub const NOVITA_DEFAULT_BASE_URL: &str = "https://api.novita.ai/v3/openai";
pub const FIREWORKS_DEFAULT_BASE_URL: &str = "https://api.fireworks.ai/inference/v1";
pub const SGLANG_DEFAULT_BASE_URL: &str = "http://localhost:30000/v1";
pub const VLLM_DEFAULT_BASE_URL: &str = "http://localhost:8000/v1";
pub const OLLAMA_DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub name: String,
    pub display_name: Option<String>,
    pub protocol: Option<ProtocolType>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub http_headers: Option<HashMap<String, String>>,
    pub capabilities: Option<ModelCapabilities>,
    #[serde(skip)]
    #[allow(dead_code)]
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderTomlConfig {
    pub protocol: Option<String>,
    pub display_name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub http_headers: Option<HashMap<String, String>>,
    pub capabilities: Option<ModelCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersToml {
    #[serde(flatten)]
    pub entries: HashMap<String, ProviderTomlConfig>,
}

#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub name: String,
    pub display_name: String,
    pub protocol: ProtocolType,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub http_headers: HashMap<String, String>,
    pub capabilities: ModelCapabilities,
    pub is_builtin: bool,
    pub overridden_fields: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BuiltinDefaults {
    pub protocol: ProtocolType,
    pub base_url: String,
    pub default_model: Option<String>,
    pub capabilities: ModelCapabilities,
    pub display_name: String,
}

struct CompileTimeDefaults {
    defaults: HashMap<String, BuiltinDefaults>,
}

impl CompileTimeDefaults {
    fn new() -> Self {
        let mut defaults = HashMap::new();

        let deepseek_caps = ModelCapabilities {
            thinking_supported: Some(true),
            supports_tools: Some(true),
            supports_reasoning: Some(true),
            supports_streaming: Some(true),
            context_window: Some(1_000_000),
            max_output: Some(384_000),
            cache_telemetry_supported: Some(true),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "deepseek".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: DEEPSEEK_DEFAULT_BASE_URL.into(),
                default_model: Some("deepseek-v4-pro".into()),
                capabilities: deepseek_caps.clone(),
                display_name: "DeepSeek".into(),
            },
        );
        defaults.insert(
            "deepseek-cn".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: DEEPSEEK_DEFAULT_BASE_URL.into(),
                default_model: Some("deepseek-v4-pro".into()),
                capabilities: deepseek_caps,
                display_name: "DeepSeek (CN)".into(),
            },
        );

        let openai_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            supports_vision: Some(true),
            context_window: Some(128_000),
            max_output: Some(16_384),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "openai".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: OPENAI_DEFAULT_BASE_URL.into(),
                default_model: Some("gpt-4.1".into()),
                capabilities: openai_caps,
                display_name: "OpenAI".into(),
            },
        );

        let openrouter_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            context_window: Some(128_000),
            max_output: Some(4_096),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "openrouter".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: OPENROUTER_DEFAULT_BASE_URL.into(),
                default_model: Some("deepseek/deepseek-v4-pro".into()),
                capabilities: openrouter_caps,
                display_name: "OpenRouter".into(),
            },
        );

        let nvidia_caps = ModelCapabilities {
            thinking_supported: Some(true),
            supports_tools: Some(true),
            supports_streaming: Some(true),
            cache_telemetry_supported: Some(true),
            context_window: Some(1_000_000),
            max_output: Some(384_000),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "nvidia-nim".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: NVIDIA_NIM_DEFAULT_BASE_URL.into(),
                default_model: Some("deepseek-ai/deepseek-v4-pro".into()),
                capabilities: nvidia_caps,
                display_name: "NVIDIA NIM".into(),
            },
        );

        let novita_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            context_window: Some(128_000),
            max_output: Some(4_096),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "novita".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: NOVITA_DEFAULT_BASE_URL.into(),
                default_model: Some("deepseek/deepseek-v4-pro".into()),
                capabilities: novita_caps,
                display_name: "Novita".into(),
            },
        );

        let fireworks_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            context_window: Some(128_000),
            max_output: Some(4_096),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "fireworks".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: FIREWORKS_DEFAULT_BASE_URL.into(),
                default_model: Some("accounts/fireworks/models/deepseek-v4-pro".into()),
                capabilities: fireworks_caps,
                display_name: "Fireworks".into(),
            },
        );

        let sglang_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            context_window: Some(128_000),
            max_output: Some(4_096),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "sglang".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: SGLANG_DEFAULT_BASE_URL.into(),
                default_model: None,
                capabilities: sglang_caps,
                display_name: "SGLang".into(),
            },
        );

        let vllm_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            context_window: Some(128_000),
            max_output: Some(4_096),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "vllm".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: VLLM_DEFAULT_BASE_URL.into(),
                default_model: None,
                capabilities: vllm_caps,
                display_name: "vLLM".into(),
            },
        );

        let ollama_caps = ModelCapabilities {
            supports_tools: Some(true),
            supports_streaming: Some(true),
            context_window: Some(8_192),
            max_output: Some(4_096),
            ..ModelCapabilities::default()
        };
        defaults.insert(
            "ollama".into(),
            BuiltinDefaults {
                protocol: ProtocolType::OpenaiCompatible,
                base_url: OLLAMA_DEFAULT_BASE_URL.into(),
                default_model: None,
                capabilities: ollama_caps,
                display_name: "Ollama".into(),
            },
        );

        Self { defaults }
    }

    fn get(&self, name: &str) -> Option<&BuiltinDefaults> {
        self.defaults.get(name)
    }

    fn all_names(&self) -> Vec<&str> {
        self.defaults.keys().map(|s| s.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum UnregisterOutcome {
    Removed,
    ClearedOverride,
    CannotRemoveBuiltin,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CapValue {
    Bool(bool),
    U32(u32),
}

#[allow(dead_code)]
pub struct ProviderRegistry {
    providers: HashMap<String, ResolvedProvider>,
    builtin_defaults: CompileTimeDefaults,
    active_provider: String,
}

#[allow(dead_code)]
impl ProviderRegistry {
    pub fn new() -> Self {
        let builtin_defaults = CompileTimeDefaults::new();
        let mut providers = HashMap::new();

        for name in builtin_defaults.all_names() {
            let bd = builtin_defaults.get(name).unwrap();
            providers.insert(
                name.to_string(),
                ResolvedProvider {
                    name: name.to_string(),
                    display_name: bd.display_name.clone(),
                    protocol: bd.protocol,
                    base_url: bd.base_url.clone(),
                    api_key: None,
                    default_model: bd.default_model.clone(),
                    http_headers: HashMap::new(),
                    capabilities: bd.capabilities.clone(),
                    is_builtin: true,
                    overridden_fields: HashSet::new(),
                },
            );
        }

        Self {
            providers,
            builtin_defaults,
            active_provider: "deepseek".to_string(),
        }
    }

    pub fn register(&mut self, entry: ProviderEntry) -> Result<()> {
        if self.providers.contains_key(&entry.name) {
            bail!("provider '{}' already exists", entry.name);
        }
        if !Self::is_name_valid(&entry.name) {
            bail!(
                "invalid provider name '{}': must be 1-63 chars, lowercase letters/digits/hyphens, start with a letter",
                entry.name
            );
        }
        let protocol = entry.protocol.context_else(|| {
            format!(
                "user-defined provider '{}' must specify a protocol",
                entry.name
            )
        })?;
        let base_url = entry.base_url.context_else(|| {
            format!(
                "user-defined provider '{}' must specify a base_url",
                entry.name
            )
        })?;

        let user_caps = entry.capabilities.as_ref();
        let protocol_defaults =
            crate::capability_bridge::CapabilityBridge::protocol_defaults(protocol);
        let resolved_caps = match user_caps {
            Some(uc) => uc.merge_with_defaults(&protocol_defaults),
            None => protocol_defaults,
        };

        let resolved = ResolvedProvider {
            name: entry.name.clone(),
            display_name: entry
                .display_name
                .clone()
                .unwrap_or_else(|| entry.name.clone()),
            protocol,
            base_url,
            api_key: entry.api_key.clone(),
            default_model: entry.default_model.clone(),
            http_headers: entry.http_headers.clone().unwrap_or_default(),
            capabilities: resolved_caps,
            is_builtin: false,
            overridden_fields: HashSet::new(),
        };
        self.providers.insert(entry.name, resolved);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedProvider> {
        self.providers.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ResolvedProvider> {
        self.providers.get_mut(name)
    }

    pub fn unregister(&mut self, name: &str) -> Result<UnregisterOutcome> {
        let is_builtin = self
            .providers
            .get(name)
            .map(|p| p.is_builtin)
            .context_else(|| format!("provider '{}' not found", name))?;

        if is_builtin {
            let overridden = self
                .providers
                .get(name)
                .unwrap()
                .overridden_fields
                .is_empty();
            if overridden {
                return Ok(UnregisterOutcome::CannotRemoveBuiltin);
            }
            let bd = self.builtin_defaults.get(name).unwrap();
            self.providers.insert(
                name.to_string(),
                ResolvedProvider {
                    name: name.to_string(),
                    display_name: bd.display_name.clone(),
                    protocol: bd.protocol,
                    base_url: bd.base_url.clone(),
                    api_key: None,
                    default_model: bd.default_model.clone(),
                    http_headers: HashMap::new(),
                    capabilities: bd.capabilities.clone(),
                    is_builtin: true,
                    overridden_fields: HashSet::new(),
                },
            );
            return Ok(UnregisterOutcome::ClearedOverride);
        }

        self.providers.remove(name);
        Ok(UnregisterOutcome::Removed)
    }

    pub fn all_providers(&self) -> Vec<&ResolvedProvider> {
        let mut list: Vec<_> = self.providers.values().collect();
        list.sort_by_key(|p| &p.name);
        list
    }

    pub fn active_provider(&self) -> &str {
        &self.active_provider
    }

    pub fn set_active(&mut self, name: &str) -> Result<()> {
        if !self.providers.contains_key(name) {
            bail!("provider '{}' not found", name);
        }
        self.active_provider = name.to_string();
        Ok(())
    }

    pub fn is_builtin(&self, name: &str) -> bool {
        self.providers
            .get(name)
            .map(|p| p.is_builtin)
            .unwrap_or(false)
    }

    pub fn builtin_names(&self) -> Vec<&str> {
        self.builtin_defaults.all_names()
    }

    pub fn is_name_valid(name: &str) -> bool {
        if name.is_empty() || name.len() > 63 {
            return false;
        }
        let first = name.as_bytes()[0];
        if !first.is_ascii_alphabetic() {
            return false;
        }
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !name.contains("..")
            && !name.contains('/')
            && !name.contains('\\')
    }

    pub fn apply_override_for_builtin(
        &mut self,
        name: &str,
        api_key: Option<String>,
        base_url: Option<String>,
        model: Option<String>,
        http_headers: Option<HashMap<String, String>>,
        capabilities: Option<ModelCapabilities>,
    ) -> Result<()> {
        let provider = self
            .providers
            .get_mut(name)
            .context_else(|| format!("provider '{}' not found", name))?;

        if !provider.is_builtin {
            bail!(
                "'{}' is not a builtin provider; use register() for user-defined providers",
                name
            );
        }

        let mut overridden = HashSet::new();
        if api_key.is_some() {
            provider.api_key = api_key;
            overridden.insert("api_key".to_string());
        }
        if base_url.is_some() {
            provider.base_url = base_url.unwrap();
            overridden.insert("base_url".to_string());
        }
        if model.is_some() {
            provider.default_model = model;
            overridden.insert("default_model".to_string());
        }
        if http_headers.is_some() {
            provider.http_headers = http_headers.unwrap();
            overridden.insert("http_headers".to_string());
        }
        if capabilities.is_some() {
            let user_caps = capabilities.unwrap();
            let builtin_caps = provider.capabilities.clone();
            provider.capabilities = user_caps.merge_with_defaults(&builtin_caps);
            overridden.insert("capabilities".to_string());
        }
        provider.overridden_fields = overridden;
        Ok(())
    }

    pub fn load_from_providers_config(
        &mut self,
        config: &crate::config::ProvidersConfig,
    ) -> Result<()> {
        let overrides: &[(&str, &crate::config::ProviderConfig)] = &[
            ("deepseek", &config.deepseek),
            ("deepseek-cn", &config.deepseek_cn),
            ("nvidia-nim", &config.nvidia_nim),
            ("openai", &config.openai),
            ("openrouter", &config.openrouter),
            ("novita", &config.novita),
            ("fireworks", &config.fireworks),
            ("sglang", &config.sglang),
            ("vllm", &config.vllm),
            ("ollama", &config.ollama),
        ];

        for (name, pc) in overrides {
            if pc.api_key.is_none() && pc.base_url.is_none() && pc.model.is_none() {
                continue;
            }
            self.apply_override_for_builtin(
                name,
                pc.api_key.clone(),
                pc.base_url.clone(),
                pc.model.clone(),
                pc.http_headers.clone(),
                None,
            )?;
        }
        Ok(())
    }

    pub fn load_from_custom_providers(&mut self, custom: &ProvidersToml) -> Result<()> {
        for (name, cfg) in &custom.entries {
            if self.providers.contains_key(name) && self.providers[name].is_builtin {
                if cfg.protocol.is_some() {
                    bail!(
                        "provider '{}' conflicts with builtin: cannot specify 'protocol' field when overriding a builtin provider (remove the 'protocol' field to override config only)",
                        name
                    );
                }
                self.apply_override_for_builtin(
                    name,
                    cfg.api_key.clone(),
                    cfg.base_url.clone(),
                    cfg.model.clone(),
                    cfg.http_headers.clone(),
                    cfg.capabilities.clone(),
                )?;
                continue;
            }

            let protocol_str = cfg.protocol.as_deref().context_else(|| {
                format!("custom provider '{}': missing required 'protocol' field (must be one of: openai_compatible, anthropic_messages, google_gemini)", name)
            })?;
            let protocol = match protocol_str {
                "openai_compatible" => ProtocolType::OpenaiCompatible,
                "anthropic_messages" => ProtocolType::AnthropicMessages,
                "google_gemini" => ProtocolType::GoogleGemini,
                other => bail!("custom provider '{}': unknown protocol '{}'", name, other),
            };

            let base_url = cfg.base_url.clone().context_else(|| {
                format!("custom provider '{}': missing required 'base_url'", name)
            })?;

            let entry = ProviderEntry {
                name: name.clone(),
                display_name: cfg.display_name.clone(),
                protocol: Some(protocol),
                base_url: Some(base_url),
                api_key: cfg.api_key.clone(),
                default_model: cfg.model.clone(),
                http_headers: cfg.http_headers.clone(),
                capabilities: cfg.capabilities.clone(),
                is_builtin: false,
            };
            self.register(entry)?;
        }
        Ok(())
    }
}

trait ContextElse<T>: Sized {
    fn context_else<C: std::fmt::Display + Send + Sync + 'static>(
        self,
        create: impl FnOnce() -> C,
    ) -> Result<T>;
}

impl<T> ContextElse<T> for Option<T> {
    fn context_else<C: std::fmt::Display + Send + Sync + 'static>(
        self,
        create: impl FnOnce() -> C,
    ) -> Result<T> {
        self.context(create())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registry_has_all_builtins() {
        let registry = ProviderRegistry::new();
        let all = registry.all_providers();
        assert!(all.len() >= 10);
        let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"deepseek"));
        assert!(names.contains(&"ollama"));
    }

    #[test]
    fn load_custom_provider_openai_compatible() {
        let mut registry = ProviderRegistry::new();
        let mut custom = ProvidersToml::default();
        custom.entries.insert(
            "my-llm".to_string(),
            ProviderTomlConfig {
                protocol: Some("openai_compatible".to_string()),
                base_url: Some("https://api.my-llm.com/v1".to_string()),
                api_key: Some("sk-test".to_string()),
                model: Some("my-model".to_string()),
                ..ProviderTomlConfig::default()
            },
        );
        registry.load_from_custom_providers(&custom).unwrap();

        let resolved = registry.get("my-llm").unwrap();
        assert_eq!(resolved.name, "my-llm");
        assert_eq!(resolved.protocol, ProtocolType::OpenaiCompatible);
        assert_eq!(resolved.base_url, "https://api.my-llm.com/v1");
        assert!(!resolved.is_builtin);
    }

    #[test]
    fn load_custom_provider_anthropiic() {
        let mut registry = ProviderRegistry::new();
        let mut custom = ProvidersToml::default();
        custom.entries.insert(
            "claude-proxy".to_string(),
            ProviderTomlConfig {
                protocol: Some("anthropic_messages".to_string()),
                base_url: Some("https://api.anthropic.com".to_string()),
                api_key: Some("sk-ant-test".to_string()),
                ..ProviderTomlConfig::default()
            },
        );
        registry.load_from_custom_providers(&custom).unwrap();

        let resolved = registry.get("claude-proxy").unwrap();
        assert_eq!(resolved.protocol, ProtocolType::AnthropicMessages);
    }

    #[test]
    fn load_custom_provider_gemini() {
        let mut registry = ProviderRegistry::new();
        let mut custom = ProvidersToml::default();
        custom.entries.insert(
            "gemini-proxy".to_string(),
            ProviderTomlConfig {
                protocol: Some("google_gemini".to_string()),
                base_url: Some("https://generativelanguage.googleapis.com".to_string()),
                ..ProviderTomlConfig::default()
            },
        );
        registry.load_from_custom_providers(&custom).unwrap();

        let resolved = registry.get("gemini-proxy").unwrap();
        assert_eq!(resolved.protocol, ProtocolType::GoogleGemini);
    }

    #[test]
    fn load_custom_provider_missing_base_url_fails() {
        let mut registry = ProviderRegistry::new();
        let mut custom = ProvidersToml::default();
        custom.entries.insert(
            "bad-provider".to_string(),
            ProviderTomlConfig {
                protocol: Some("openai_compatible".to_string()),
                base_url: None,
                ..ProviderTomlConfig::default()
            },
        );
        assert!(registry.load_from_custom_providers(&custom).is_err());
    }

    #[test]
    fn load_custom_provider_overrides_builtin() {
        let mut registry = ProviderRegistry::new();
        let mut custom = ProvidersToml::default();
        custom.entries.insert(
            "ollama".to_string(),
            ProviderTomlConfig {
                base_url: Some("http://custom-ollama:11434/v1".to_string()),
                ..ProviderTomlConfig::default()
            },
        );
        registry.load_from_custom_providers(&custom).unwrap();

        let resolved = registry.get("ollama").unwrap();
        assert_eq!(resolved.base_url, "http://custom-ollama:11434/v1");
        assert!(resolved.is_builtin);
    }

    #[test]
    fn load_from_providers_config_applies_overrides() {
        let mut registry = ProviderRegistry::new();
        let mut providers = crate::config::ProvidersConfig::default();
        providers.openai.api_key = Some("sk-openai-test".to_string());
        providers.openai.base_url = Some("https://custom-openai/v1".to_string());
        registry.load_from_providers_config(&providers).unwrap();

        let resolved = registry.get("openai").unwrap();
        assert_eq!(resolved.api_key, Some("sk-openai-test".to_string()));
        assert_eq!(resolved.base_url, "https://custom-openai/v1");
    }
}

pub fn save_custom_provider_to_config(
    provider_name: &str,
    base_url: &str,
    protocol: &str,
    api_key: Option<&str>,
) -> Result<std::path::PathBuf> {
    use std::fs;

    let config_path = crate::config::default_config_path()
        .context("Failed to resolve config path: home directory not found.")?;
    crate::config::ensure_parent_dir(&config_path)?;

    let mut doc: toml::Value = if config_path.exists() {
        let raw = fs::read_to_string(&config_path)?;
        toml::from_str(&raw)
            .with_context(|| format!("Failed to parse config at {}", config_path.display()))?
    } else {
        toml::Value::Table(toml::value::Table::new())
    };

    let table = doc
        .as_table_mut()
        .context("Config root must be a TOML table.")?;
    let providers = table
        .entry("providers".to_string())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()))
        .as_table_mut()
        .context("`providers` must be a table.")?;
    let entry = providers
        .entry(provider_name.to_string())
        .or_insert_with(|| toml::Value::Table(toml::value::Table::new()))
        .as_table_mut()
        .with_context(|| format!("`providers.{}` must be a table.", provider_name))?;

    entry.insert(
        "base_url".to_string(),
        toml::Value::String(base_url.to_string()),
    );
    entry.insert(
        "protocol".to_string(),
        toml::Value::String(protocol.to_string()),
    );
    if let Some(key) = api_key {
        entry.insert("api_key".to_string(), toml::Value::String(key.to_string()));
    }

    let serialized = toml::to_string_pretty(&doc).context("failed to serialize updated config")?;
    crate::config::write_config_file_secure(&config_path, &serialized)
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

    Ok(config_path)
}
