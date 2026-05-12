#[cfg(test)]
use crate::models::{ContentBlock, Message, SystemBlock};
use crate::models::{MessageRequest, ModelCapabilities, SystemPrompt};

pub struct CapabilityFilter;

impl CapabilityFilter {
    pub fn filter_request(
        request: MessageRequest,
        capabilities: &ModelCapabilities,
    ) -> MessageRequest {
        let mut request = request;

        if capabilities.thinking_supported == Some(false) {
            request.thinking = None;
            request.reasoning_effort = None;
        }

        if capabilities.supports_tools == Some(false) {
            request.tools = None;
            request.tool_choice = None;
        }

        if capabilities.supports_vision == Some(false) {
            // 上游 ContentBlock 枚举当前无 Image 变体，无法执行
            // retain |block| !matches!(block, ContentBlock::Image { .. })。
            // 待上游添加 ContentBlock::Image 后实现剥离逻辑。
            // 当前为安全空操作：无 Image block 需要移除。
        }

        if capabilities.cache_telemetry_supported == Some(false) {
            if let Some(ref mut system) = request.system {
                if let SystemPrompt::Blocks(blocks) = system {
                    for block in blocks.iter_mut() {
                        block.cache_control = None;
                    }
                }
            }
        }

        if capabilities.supports_streaming == Some(false) {
            request.stream = Some(false);
        }

        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> MessageRequest {
        MessageRequest {
            model: "test-model".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "hello".to_string(),
                    cache_control: None,
                }],
            }],
            max_tokens: 4096,
            system: None,
            tools: None,
            tool_choice: None,
            metadata: None,
            thinking: None,
            reasoning_effort: None,
            stream: None,
            temperature: None,
            top_p: None,
        }
    }

    #[test]
    fn filter_removes_thinking_when_not_supported() {
        let mut req = base_request();
        req.thinking = Some(serde_json::json!({"type": "enabled", "budget_tokens": 10000}));
        req.reasoning_effort = Some("high".to_string());

        let caps = ModelCapabilities {
            thinking_supported: Some(false),
            ..ModelCapabilities::default()
        };

        let filtered = CapabilityFilter::filter_request(req, &caps);
        assert!(filtered.thinking.is_none());
        assert!(filtered.reasoning_effort.is_none());
    }

    #[test]
    fn filter_removes_tools_when_not_supported() {
        let mut req = base_request();
        req.tools = Some(vec![]);
        req.tool_choice = Some(serde_json::json!("auto"));

        let caps = ModelCapabilities {
            supports_tools: Some(false),
            ..ModelCapabilities::default()
        };

        let filtered = CapabilityFilter::filter_request(req, &caps);
        assert!(filtered.tools.is_none());
        assert!(filtered.tool_choice.is_none());
    }

    #[test]
    fn filter_disables_streaming_when_not_supported() {
        let mut req = base_request();
        req.stream = Some(true);

        let caps = ModelCapabilities {
            supports_streaming: Some(false),
            ..ModelCapabilities::default()
        };

        let filtered = CapabilityFilter::filter_request(req, &caps);
        assert_eq!(filtered.stream, Some(false));
    }

    #[test]
    fn filter_clears_cache_control_when_not_supported() {
        let mut req = base_request();
        req.system = Some(SystemPrompt::Blocks(vec![SystemBlock {
            block_type: "text".to_string(),
            text: "system prompt".to_string(),
            cache_control: Some(crate::models::CacheControl {
                cache_type: "ephemeral".to_string(),
            }),
        }]));

        let caps = ModelCapabilities {
            cache_telemetry_supported: Some(false),
            ..ModelCapabilities::default()
        };

        let filtered = CapabilityFilter::filter_request(req, &caps);
        if let Some(SystemPrompt::Blocks(blocks)) = &filtered.system {
            assert!(blocks[0].cache_control.is_none());
        } else {
            panic!("expected SystemPrompt::Blocks");
        }
    }

    #[test]
    fn filter_noop_when_all_capabilities_default() {
        let req = base_request();
        let caps = ModelCapabilities::default();
        let filtered = CapabilityFilter::filter_request(req.clone(), &caps);
        assert_eq!(filtered.model, req.model);
        assert_eq!(filtered.messages.len(), req.messages.len());
    }

    #[test]
    fn filter_combined_disables() {
        let mut req = base_request();
        req.thinking = Some(serde_json::json!({"type": "enabled"}));
        req.reasoning_effort = Some("max".to_string());
        req.tools = Some(vec![]);
        req.stream = Some(true);

        let caps = ModelCapabilities {
            thinking_supported: Some(false),
            supports_tools: Some(false),
            supports_streaming: Some(false),
            ..ModelCapabilities::default()
        };

        let filtered = CapabilityFilter::filter_request(req, &caps);
        assert!(filtered.thinking.is_none());
        assert!(filtered.reasoning_effort.is_none());
        assert!(filtered.tools.is_none());
        assert_eq!(filtered.stream, Some(false));
    }
}
