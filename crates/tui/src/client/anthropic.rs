use crate::models::{
    ContentBlock, MessageRequest, MessageResponse, StreamEvent, SystemPrompt, Usage,
};

pub fn convert_request(request: &MessageRequest) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
    });

    if let Some(ref system) = request.system {
        let system_text = match system {
            SystemPrompt::Text(t) => t.clone(),
            SystemPrompt::Blocks(blocks) => blocks
                .iter()
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        };
        body["system"] = serde_json::Value::String(system_text);
    }

    let mut messages = Vec::new();
    for msg in &request.messages {
        let mut content = Vec::new();
        for block in &msg.content {
            match block {
                ContentBlock::Text { text, .. } => {
                    content.push(serde_json::json!({"type": "text", "text": text}));
                }
                ContentBlock::Thinking { thinking } => {
                    content.push(serde_json::json!({"type": "thinking", "thinking": thinking}));
                }
                ContentBlock::ToolUse {
                    id, name, input, ..
                } => {
                    content.push(serde_json::json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input,
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content: result_content,
                    is_error,
                    ..
                } => {
                    content.push(serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": result_content,
                        "is_error": is_error.unwrap_or(false),
                    }));
                }
                _ => {}
            }
        }
        messages.push(serde_json::json!({
            "role": msg.role,
            "content": content,
        }));
    }
    body["messages"] = serde_json::Value::Array(messages);

    if let Some(ref tools) = request.tools {
        let anthropic_tools: Vec<_> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();
        body["tools"] = serde_json::Value::Array(anthropic_tools);
    }

    if let Some(ref tc) = request.tool_choice {
        body["tool_choice"] = tc.clone();
    }

    if let Some(ref thinking) = request.thinking {
        body["thinking"] = thinking.clone();
    }

    if let Some(temp) = request.temperature {
        body["temperature"] = serde_json::json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["top_p"] = serde_json::json!(top_p);
    }

    body
}

pub fn convert_response(resp_json: &serde_json::Value) -> MessageResponse {
    let content = resp_json
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|block| {
                    let block_type = block.get("type")?.as_str()?;
                    match block_type {
                        "text" => Some(ContentBlock::Text {
                            text: block.get("text")?.as_str()?.to_string(),
                            cache_control: None,
                        }),
                        "thinking" => Some(ContentBlock::Thinking {
                            thinking: block.get("thinking")?.as_str()?.to_string(),
                        }),
                        "tool_use" => Some(ContentBlock::ToolUse {
                            id: block.get("id")?.as_str()?.to_string(),
                            name: block.get("name")?.as_str()?.to_string(),
                            input: block
                                .get("input")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null),
                            caller: None,
                        }),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let usage = resp_json
        .get("usage")
        .map(|u| Usage {
            input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            ..Usage::default()
        })
        .unwrap_or_default();

    MessageResponse {
        id: resp_json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        r#type: "message".to_string(),
        role: resp_json
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant")
            .to_string(),
        content,
        model: resp_json
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        stop_reason: resp_json
            .get("stop_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        stop_sequence: resp_json
            .get("stop_sequence")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        container: None,
        usage,
    }
}

pub fn anthropic_sse_stream_from_response(
    response: reqwest::Response,
    model: String,
) -> crate::llm_client::StreamEventBox {
    use crate::models::{MessageResponse, Usage};
    use std::pin::Pin;

    let byte_stream = response.bytes_stream();
    let stream = async_stream::stream! {
        use futures_util::StreamExt;

        yield Ok(StreamEvent::MessageStart {
            message: MessageResponse {
                id: String::new(),
                r#type: "message".to_string(),
                role: "assistant".to_string(),
                content: Vec::new(),
                model: model.clone(),
                stop_reason: None,
                stop_sequence: None,
                container: None,
                usage: Usage::default(),
            },
        });

        let mut line_buf = String::new();
        let mut byte_buf: Vec<u8> = Vec::with_capacity(4096);
        let mut content_index: u32 = 0;

        let mut byte_stream = std::pin::pin!(byte_stream);

        loop {
            let chunk = match byte_stream.next().await {
                Some(Ok(bytes)) => bytes,
                Some(Err(e)) => {
                    yield Err(anyhow::anyhow!("Anthropic stream read error: {e}"));
                    break;
                }
                None => break,
            };
            byte_buf.extend_from_slice(&chunk);

            while let Some(newline_pos) = byte_buf.iter().position(|&b| b == b'\n') {
                let mut end = newline_pos;
                if end > 0 && byte_buf[end - 1] == b'\r' {
                    end -= 1;
                }
                let line = String::from_utf8_lossy(&byte_buf[..end]).into_owned();
                byte_buf.drain(..newline_pos + 1);

                if line.is_empty() {
                    if !line_buf.is_empty() {
                        let data = std::mem::take(&mut line_buf);
                        if let Ok(event_json) = serde_json::from_str::<serde_json::Value>(&data) {
                            let event_type = event_json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match event_type {
                                "content_block_start" => {
                                    if let Some(block) = event_json.get("content_block") {
                                        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                        match block_type {
                                            "text" => {
                                                let text = block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                                yield Ok(StreamEvent::ContentBlockStart {
                                                    index: content_index,
                                                    content_block: crate::models::ContentBlockStart::Text {
                                                        text: text.to_string(),
                                                    },
                                                });
                                                content_index += 1;
                                            }
                                            "thinking" => {
                                                let thinking = block.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
                                                yield Ok(StreamEvent::ContentBlockStart {
                                                    index: content_index,
                                                    content_block: crate::models::ContentBlockStart::Thinking {
                                                        thinking: thinking.to_string(),
                                                    },
                                                });
                                                content_index += 1;
                                            }
                                            "tool_use" => {
                                                yield Ok(StreamEvent::ContentBlockStart {
                                                    index: content_index,
                                                    content_block: crate::models::ContentBlockStart::ToolUse {
                                                        id: block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                        name: block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                        input: block.get("input").cloned().unwrap_or(serde_json::Value::Null),
                                                        caller: None,
                                                    },
                                                });
                                                content_index += 1;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                "content_block_delta" => {
                                    if let Some(delta) = event_json.get("delta") {
                                        let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                        let idx = event_json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                        match delta_type {
                                            "text_delta" => {
                                                yield Ok(StreamEvent::ContentBlockDelta {
                                                    index: idx,
                                                    delta: crate::models::Delta::TextDelta {
                                                        text: delta.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                    },
                                                });
                                            }
                                            "thinking_delta" => {
                                                yield Ok(StreamEvent::ContentBlockDelta {
                                                    index: idx,
                                                    delta: crate::models::Delta::ThinkingDelta {
                                                        thinking: delta.get("thinking").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                    },
                                                });
                                            }
                                            "input_json_delta" => {
                                                yield Ok(StreamEvent::ContentBlockDelta {
                                                    index: idx,
                                                    delta: crate::models::Delta::InputJsonDelta {
                                                        partial_json: delta.get("partial_json").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                    },
                                                });
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                "content_block_stop" => {
                                    let idx = event_json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    yield Ok(StreamEvent::ContentBlockStop { index: idx });
                                }
                                "message_delta" => {
                                    if let Some(delta) = event_json.get("delta") {
                                        let stop_reason = delta.get("stop_reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                                        let usage = event_json.get("usage").map(|u| Usage {
                                            input_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                                            output_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                                            ..Usage::default()
                                        });
                                        yield Ok(StreamEvent::MessageDelta {
                                            delta: crate::models::MessageDelta {
                                                stop_reason,
                                                stop_sequence: None,
                                            },
                                            usage,
                                        });
                                    }
                                }
                                "message_stop" => {
                                    yield Ok(StreamEvent::MessageStop);
                                }
                                _ => {}
                            }
                        }
                    }
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    line_buf.push_str(data);
                }
            }
        }

        yield Ok(StreamEvent::MessageStop);
    };

    Pin::from(Box::new(stream)
        as Box<
            dyn futures_util::Stream<Item = anyhow::Result<StreamEvent>> + Send,
        >)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Message, Tool};

    fn base_request() -> MessageRequest {
        MessageRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Hello".to_string(),
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
    fn convert_request_includes_model_and_max_tokens() {
        let req = base_request();
        let body = convert_request(&req);
        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["max_tokens"], 4096);
    }

    #[test]
    fn convert_request_maps_text_content_blocks() {
        let req = base_request();
        let body = convert_request(&req);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        let content = messages[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hello");
    }

    #[test]
    fn convert_request_maps_system_prompt_text() {
        let mut req = base_request();
        req.system = Some(SystemPrompt::Text("You are helpful.".to_string()));
        let body = convert_request(&req);
        assert_eq!(body["system"], "You are helpful.");
    }

    #[test]
    fn convert_request_maps_thinking_block() {
        let req = MessageRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![Message {
                role: "assistant".to_string(),
                content: vec![ContentBlock::Thinking {
                    thinking: "Let me think...".to_string(),
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
        };
        let body = convert_request(&req);
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[0]["thinking"], "Let me think...");
    }

    #[test]
    fn convert_request_includes_tools() {
        let mut req = base_request();
        req.tools = Some(vec![Tool {
            tool_type: None,
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
            allowed_callers: None,
            defer_loading: None,
            input_examples: None,
            strict: None,
            cache_control: None,
        }]);
        let body = convert_request(&req);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "get_weather");
        assert!(tools[0].get("input_schema").is_some());
    }

    #[test]
    fn convert_request_includes_temperature_and_top_p() {
        let mut req = base_request();
        req.temperature = Some(0.7);
        req.top_p = Some(0.9);
        let body = convert_request(&req);
        let temp = body["temperature"].as_f64().unwrap();
        let top_p = body["top_p"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01);
        assert!((top_p - 0.9).abs() < 0.01);
    }

    #[test]
    fn convert_response_parses_text_content() {
        let json = serde_json::json!({
            "id": "msg_123",
            "role": "assistant",
            "model": "claude-sonnet-4-20250514",
            "content": [{"type": "text", "text": "Hi there!"}],
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "stop_reason": "end_turn"
        });
        let resp = convert_response(&json);
        assert_eq!(resp.id, "msg_123");
        assert_eq!(resp.model, "claude-sonnet-4-20250514");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.stop_reason, Some("end_turn".to_string()));
        if let ContentBlock::Text { text, .. } = &resp.content[0] {
            assert_eq!(text, "Hi there!");
        } else {
            panic!("expected Text block");
        }
    }

    #[test]
    fn convert_response_parses_thinking_and_tool_use() {
        let json = serde_json::json!({
            "id": "msg_456",
            "role": "assistant",
            "model": "claude-sonnet-4-20250514",
            "content": [
                {"type": "thinking", "thinking": "reasoning..."},
                {"type": "tool_use", "id": "tu_1", "name": "search", "input": {"q": "test"}}
            ],
            "usage": {"input_tokens": 20, "output_tokens": 10},
            "stop_reason": "tool_use"
        });
        let resp = convert_response(&json);
        assert_eq!(resp.content.len(), 2);
        if let ContentBlock::Thinking { thinking } = &resp.content[0] {
            assert_eq!(thinking, "reasoning...");
        } else {
            panic!("expected Thinking block");
        }
        if let ContentBlock::ToolUse { name, .. } = &resp.content[1] {
            assert_eq!(name, "search");
        } else {
            panic!("expected ToolUse block");
        }
    }

    #[test]
    fn convert_response_handles_empty_content() {
        let json = serde_json::json!({
            "id": "msg_789",
            "role": "assistant",
            "model": "claude-sonnet-4-20250514",
            "content": [],
            "usage": {"input_tokens": 0, "output_tokens": 0}
        });
        let resp = convert_response(&json);
        assert!(resp.content.is_empty());
        assert_eq!(resp.usage.input_tokens, 0);
    }
}
