use crate::models::{ContentBlock, MessageRequest, MessageResponse, StreamEvent, Usage};

pub fn convert_request(request: &MessageRequest) -> serde_json::Value {
    let mut body = serde_json::json!({});

    if let Some(ref system) = request.system {
        let system_text = match system {
            crate::models::SystemPrompt::Text(t) => t.clone(),
            crate::models::SystemPrompt::Blocks(blocks) => blocks
                .iter()
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        };
        body["systemInstruction"] = serde_json::json!({
            "parts": [{"text": system_text}]
        });
    }

    let mut contents = Vec::new();
    for msg in &request.messages {
        let role = match msg.role.as_str() {
            "assistant" => "model",
            other => other,
        };

        let mut parts = Vec::new();
        for block in &msg.content {
            match block {
                ContentBlock::Text { text, .. } => {
                    parts.push(serde_json::json!({"text": text}));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    parts.push(serde_json::json!({
                        "functionCall": {
                            "name": name,
                            "args": input,
                        }
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content: result_content,
                    ..
                } => {
                    parts.push(serde_json::json!({
                        "functionResponse": {
                            "name": tool_use_id,
                            "response": {"result": result_content},
                        }
                    }));
                }
                _ => {}
            }
        }

        if !parts.is_empty() {
            contents.push(serde_json::json!({
                "role": role,
                "parts": parts,
            }));
        }
    }
    body["contents"] = serde_json::Value::Array(contents);

    if let Some(ref tools) = request.tools {
        let declarations: Vec<_> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                })
            })
            .collect();
        body["tools"] = serde_json::json!([{
            "functionDeclarations": declarations
        }]);
    }

    if let Some(temp) = request.temperature {
        body["generationConfig"]["temperature"] = serde_json::json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["generationConfig"]["topP"] = serde_json::json!(top_p);
    }

    if request.max_tokens > 0 {
        body["generationConfig"]["maxOutputTokens"] = serde_json::json!(request.max_tokens);
    }

    body
}

pub fn convert_response(resp_json: &serde_json::Value, model: &str) -> MessageResponse {
    let content = resp_json
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        Some(ContentBlock::Text {
                            text: text.to_string(),
                            cache_control: None,
                        })
                    } else if let Some(fc) = part.get("functionCall") {
                        Some(ContentBlock::ToolUse {
                            id: format!("call_{}", uuid::Uuid::new_v4().as_simple()),
                            name: fc.get("name")?.as_str()?.to_string(),
                            input: fc.get("args").cloned().unwrap_or(serde_json::Value::Null),
                            caller: None,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let usage = resp_json
        .get("usageMetadata")
        .map(|u| Usage {
            input_tokens: u
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            output_tokens: u
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
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
        role: "assistant".to_string(),
        content,
        model: model.to_string(),
        stop_reason: resp_json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("finishReason"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        stop_sequence: None,
        container: None,
        usage,
    }
}

pub fn gemini_ndjson_stream_from_response(
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

        let mut byte_buf: Vec<u8> = Vec::with_capacity(4096);
        let mut content_index: u32 = 0;
        let mut byte_stream = std::pin::pin!(byte_stream);

        loop {
            let chunk = match byte_stream.next().await {
                Some(Ok(bytes)) => bytes,
                Some(Err(e)) => {
                    yield Err(anyhow::anyhow!("Gemini stream read error: {e}"));
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
                    continue;
                }

                if let Ok(chunk_json) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(parts) = chunk_json
                        .get("candidates")
                        .and_then(|c| c.as_array())
                        .and_then(|a| a.first())
                        .and_then(|c| c.get("content"))
                        .and_then(|c| c.get("parts"))
                        .and_then(|p| p.as_array())
                    {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                yield Ok(StreamEvent::ContentBlockStart {
                                    index: content_index,
                                    content_block: crate::models::ContentBlockStart::Text {
                                        text: String::new(),
                                    },
                                });
                                yield Ok(StreamEvent::ContentBlockDelta {
                                    index: content_index,
                                    delta: crate::models::Delta::TextDelta {
                                        text: text.to_string(),
                                    },
                                });
                                yield Ok(StreamEvent::ContentBlockStop { index: content_index });
                                content_index += 1;
                            } else if let Some(fc) = part.get("functionCall") {
                                yield Ok(StreamEvent::ContentBlockStart {
                                    index: content_index,
                                    content_block: crate::models::ContentBlockStart::ToolUse {
                                        id: format!("call_{}", uuid::Uuid::new_v4().as_simple()),
                                        name: fc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        input: fc.get("args").cloned().unwrap_or(serde_json::Value::Null),
                                        caller: None,
                                    },
                                });
                                yield Ok(StreamEvent::ContentBlockStop { index: content_index });
                                content_index += 1;
                            }
                        }
                    }

                    if let Some(u) = chunk_json.get("usageMetadata") {
                        let usage = Usage {
                            input_tokens: u.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            output_tokens: u.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            ..Usage::default()
                        };
                        let finish_reason = chunk_json
                            .get("candidates")
                            .and_then(|c| c.as_array())
                            .and_then(|a| a.first())
                            .and_then(|c| c.get("finishReason"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        yield Ok(StreamEvent::MessageDelta {
                            delta: crate::models::MessageDelta {
                                stop_reason: finish_reason,
                                stop_sequence: None,
                            },
                            usage: Some(usage),
                        });
                    }
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
            model: "gemini-2.5-flash".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Hello".to_string(),
                    cache_control: None,
                }],
            }],
            max_tokens: 8192,
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
    fn convert_request_maps_system_instruction() {
        let mut req = base_request();
        req.system = Some(crate::models::SystemPrompt::Text("Be concise.".to_string()));
        let body = convert_request(&req);
        let sys = &body["systemInstruction"];
        assert!(sys.is_object());
        assert_eq!(sys["parts"][0]["text"], "Be concise.");
    }

    #[test]
    fn convert_request_maps_assistant_role_to_model() {
        let req = MessageRequest {
            model: "gemini-2.5-flash".to_string(),
            messages: vec![Message {
                role: "assistant".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Sure".to_string(),
                    cache_control: None,
                }],
            }],
            max_tokens: 8192,
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
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents[0]["role"], "model");
    }

    #[test]
    fn convert_request_maps_tool_use_to_function_call() {
        let req = MessageRequest {
            model: "gemini-2.5-flash".to_string(),
            messages: vec![Message {
                role: "assistant".to_string(),
                content: vec![ContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "search".to_string(),
                    input: serde_json::json!({"q": "test"}),
                    caller: None,
                }],
            }],
            max_tokens: 8192,
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
        let parts = body["contents"][0]["parts"].as_array().unwrap();
        assert!(parts[0].get("functionCall").is_some());
        assert_eq!(parts[0]["functionCall"]["name"], "search");
    }

    #[test]
    fn convert_request_includes_generation_config() {
        let mut req = base_request();
        req.temperature = Some(0.5);
        req.top_p = Some(0.8);
        let body = convert_request(&req);
        let temp = body["generationConfig"]["temperature"].as_f64().unwrap();
        let top_p = body["generationConfig"]["topP"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < 0.01);
        assert!((top_p - 0.8).abs() < 0.01);
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 8192);
    }

    #[test]
    fn convert_request_wraps_tools_in_function_declarations() {
        let mut req = base_request();
        req.tools = Some(vec![Tool {
            tool_type: None,
            name: "calc".to_string(),
            description: "Calculate".to_string(),
            input_schema: serde_json::json!({"type": "object", "properties": {"expr": {"type": "string"}}}),
            allowed_callers: None,
            defer_loading: None,
            input_examples: None,
            strict: None,
            cache_control: None,
        }]);
        let body = convert_request(&req);
        let tools_arr = body["tools"].as_array().unwrap();
        assert_eq!(tools_arr.len(), 1);
        let decls = tools_arr[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls[0]["name"], "calc");
    }

    #[test]
    fn convert_response_parses_candidates_text() {
        let json = serde_json::json!({
            "id": "resp_1",
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello!"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {"promptTokenCount": 5, "candidatesTokenCount": 3}
        });
        let resp = convert_response(&json, "gemini-2.5-flash");
        assert_eq!(resp.model, "gemini-2.5-flash");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.stop_reason, Some("STOP".to_string()));
        if let ContentBlock::Text { text, .. } = &resp.content[0] {
            assert_eq!(text, "Hello!");
        } else {
            panic!("expected Text block");
        }
    }

    #[test]
    fn convert_response_parses_function_call() {
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{"functionCall": {"name": "calc", "args": {"expr": "2+2"}}}],
                    "role": "model"
                }
            }],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 8}
        });
        let resp = convert_response(&json, "gemini-2.5-flash");
        assert_eq!(resp.content.len(), 1);
        if let ContentBlock::ToolUse { name, input, .. } = &resp.content[0] {
            assert_eq!(name, "calc");
            assert_eq!(input["expr"], "2+2");
        } else {
            panic!("expected ToolUse block");
        }
    }

    #[test]
    fn convert_response_handles_empty_candidates() {
        let json = serde_json::json!({
            "candidates": [],
            "usageMetadata": {"promptTokenCount": 0, "candidatesTokenCount": 0}
        });
        let resp = convert_response(&json, "gemini-2.5-flash");
        assert!(resp.content.is_empty());
        assert_eq!(resp.usage.input_tokens, 0);
    }
}
