use crate::config::ProtocolType;
use crate::llm_client::StreamEventBox;
use crate::models::{MessageRequest, MessageResponse, ModelCapabilities};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use std::collections::HashMap;

const PROTECTED_HEADERS: &[&str] = &["authorization", "content-type"];

fn is_protected_header(name: &str) -> bool {
    PROTECTED_HEADERS.contains(&name.to_ascii_lowercase().as_str())
}

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    #[allow(dead_code)]
    fn protocol_type(&self) -> ProtocolType;

    async fn send_request(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        capabilities: &ModelCapabilities,
    ) -> Result<MessageResponse>;

    async fn send_request_stream(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        capabilities: &ModelCapabilities,
    ) -> Result<StreamEventBox>;

    fn build_auth_headers(&self, api_key: &str) -> HeaderMap;
}

pub struct ProtocolAdapterFactory;

impl ProtocolAdapterFactory {
    pub fn create(protocol: ProtocolType) -> Box<dyn ProtocolAdapter> {
        match protocol {
            ProtocolType::OpenaiCompatible => Box::new(OpenAiCompatibleAdapter),
            ProtocolType::AnthropicMessages => Box::new(AnthropicMessagesAdapter),
            ProtocolType::GoogleGemini => Box::new(GoogleGeminiAdapter),
        }
    }
}

pub struct OpenAiCompatibleAdapter;

#[async_trait]
impl ProtocolAdapter for OpenAiCompatibleAdapter {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::OpenaiCompatible
    }

    async fn send_request(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        _capabilities: &ModelCapabilities,
    ) -> Result<MessageResponse> {
        use crate::client::chat::{
            build_chat_messages_for_request, parse_chat_message, tool_to_chat_for_base_url,
        };

        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let auth_headers = match api_key {
            Some(key) => self.build_auth_headers(key),
            None => HeaderMap::new(),
        };

        let chat_messages = build_chat_messages_for_request(&request);
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": chat_messages,
            "max_tokens": request.max_tokens,
        });

        if let Some(ref tools) = request.tools {
            let chat_tools: Vec<_> = tools
                .iter()
                .map(|t| tool_to_chat_for_base_url(t, base_url))
                .collect();
            body["tools"] = serde_json::json!(chat_tools);
        }
        if let Some(ref tc) = request.tool_choice {
            body["tool_choice"] = tc.clone();
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(ref thinking) = request.thinking {
            body["thinking"] = thinking.clone();
        }
        if let Some(ref effort) = request.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(effort);
        }

        let mut req_builder = http_client.post(&url).headers(auth_headers).json(&body);
        for (key, value) in http_headers {
            if is_protected_header(key) {
                continue;
            }
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                && let Ok(header_value) = reqwest::header::HeaderValue::from_str(value)
            {
                req_builder = req_builder.header(header_name, header_value);
            }
        }

        let resp = req_builder.send().await?;
        let resp_json: serde_json::Value = resp.json().await?;
        parse_chat_message(&resp_json)
    }

    async fn send_request_stream(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        _capabilities: &ModelCapabilities,
    ) -> Result<StreamEventBox> {
        use crate::client::chat::{
            build_chat_messages_for_request, openai_sse_stream_from_response,
            tool_to_chat_for_base_url,
        };

        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let auth_headers = match api_key {
            Some(key) => self.build_auth_headers(key),
            None => HeaderMap::new(),
        };

        let chat_messages = build_chat_messages_for_request(&request);
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": chat_messages,
            "max_tokens": request.max_tokens,
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        if let Some(ref tools) = request.tools {
            let chat_tools: Vec<_> = tools
                .iter()
                .map(|t| tool_to_chat_for_base_url(t, base_url))
                .collect();
            body["tools"] = serde_json::json!(chat_tools);
        }
        if let Some(ref tc) = request.tool_choice {
            body["tool_choice"] = tc.clone();
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(ref thinking) = request.thinking {
            body["thinking"] = thinking.clone();
        }
        if let Some(ref effort) = request.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(effort);
        }

        let mut req_builder = http_client.post(&url).headers(auth_headers).json(&body);
        for (key, value) in http_headers {
            if is_protected_header(key) {
                continue;
            }
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                && let Ok(header_value) = reqwest::header::HeaderValue::from_str(value)
            {
                req_builder = req_builder.header(header_name, header_value);
            }
        }

        let response = req_builder.send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("SSE stream request failed: HTTP {}", status);
        }

        Ok(openai_sse_stream_from_response(
            response,
            request.model.clone(),
            0,
        ))
    }

    fn build_auth_headers(&self, api_key: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", api_key).parse().unwrap(),
        );
        headers
    }
}

pub struct AnthropicMessagesAdapter;

#[async_trait]
impl ProtocolAdapter for AnthropicMessagesAdapter {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::AnthropicMessages
    }

    async fn send_request(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        _capabilities: &ModelCapabilities,
    ) -> Result<MessageResponse> {
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
        let auth_headers = match api_key {
            Some(key) => self.build_auth_headers(key),
            None => HeaderMap::new(),
        };

        let anthropic_request = crate::client::anthropic::convert_request(&request);
        let mut req_builder = http_client
            .post(&url)
            .headers(auth_headers)
            .json(&anthropic_request);

        for (key, value) in http_headers {
            if is_protected_header(key) {
                continue;
            }
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                    req_builder = req_builder.header(header_name, header_value);
                }
            }
        }

        let resp = req_builder.send().await?;
        let resp_json: serde_json::Value = resp.json().await?;
        Ok(crate::client::anthropic::convert_response(&resp_json))
    }

    async fn send_request_stream(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        _capabilities: &ModelCapabilities,
    ) -> Result<StreamEventBox> {
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
        let auth_headers = match api_key {
            Some(key) => self.build_auth_headers(key),
            None => HeaderMap::new(),
        };

        let mut anthropic_request = crate::client::anthropic::convert_request(&request);
        anthropic_request["stream"] = serde_json::json!(true);

        let mut req_builder = http_client
            .post(&url)
            .headers(auth_headers)
            .json(&anthropic_request);

        for (key, value) in http_headers {
            if is_protected_header(key) {
                continue;
            }
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                    req_builder = req_builder.header(header_name, header_value);
                }
            }
        }

        let response = req_builder.send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("Anthropic SSE stream request failed: HTTP {}", status);
        }

        Ok(
            crate::client::anthropic::anthropic_sse_stream_from_response(
                response,
                request.model.clone(),
            ),
        )
    }

    fn build_auth_headers(&self, api_key: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static("x-api-key"),
            api_key.parse().unwrap(),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("anthropic-version"),
            "2023-06-01".parse().unwrap(),
        );
        headers
    }
}

pub struct GoogleGeminiAdapter;

#[async_trait]
impl ProtocolAdapter for GoogleGeminiAdapter {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::GoogleGemini
    }

    async fn send_request(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        _capabilities: &ModelCapabilities,
    ) -> Result<MessageResponse> {
        let model = &request.model;
        let base = base_url.trim_end_matches('/');
        let url = match api_key {
            Some(key) => format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                base, model, key
            ),
            None => format!("{}/v1beta/models/{}:generateContent", base, model),
        };

        let gemini_request = crate::client::gemini::convert_request(&request);
        let mut req_builder = http_client.post(&url).json(&gemini_request);

        for (key, value) in http_headers {
            if is_protected_header(key) {
                continue;
            }
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                    req_builder = req_builder.header(header_name, header_value);
                }
            }
        }

        let resp = req_builder.send().await?;
        let resp_json: serde_json::Value = resp.json().await?;
        Ok(crate::client::gemini::convert_response(&resp_json, model))
    }

    async fn send_request_stream(
        &self,
        http_client: &reqwest::Client,
        base_url: &str,
        api_key: Option<&str>,
        http_headers: &HashMap<String, String>,
        request: MessageRequest,
        _capabilities: &ModelCapabilities,
    ) -> Result<StreamEventBox> {
        let model = &request.model;
        let base = base_url.trim_end_matches('/');
        let url = match api_key {
            Some(key) => format!(
                "{}/v1beta/models/{}:streamGenerateContent?key={}",
                base, model, key
            ),
            None => format!("{}/v1beta/models/{}:streamGenerateContent", base, model),
        };

        let gemini_request = crate::client::gemini::convert_request(&request);
        let mut req_builder = http_client.post(&url).json(&gemini_request);

        for (key, value) in http_headers {
            if is_protected_header(key) {
                continue;
            }
            if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                    req_builder = req_builder.header(header_name, header_value);
                }
            }
        }

        let response = req_builder.send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("Gemini NDJSON stream request failed: HTTP {}", status);
        }

        Ok(crate::client::gemini::gemini_ndjson_stream_from_response(
            response,
            request.model.clone(),
        ))
    }

    fn build_auth_headers(&self, _api_key: &str) -> HeaderMap {
        HeaderMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_creates_openai_compatible() {
        let adapter = ProtocolAdapterFactory::create(ProtocolType::OpenaiCompatible);
        assert_eq!(adapter.protocol_type(), ProtocolType::OpenaiCompatible);
    }

    #[test]
    fn factory_creates_anthropic_messages() {
        let adapter = ProtocolAdapterFactory::create(ProtocolType::AnthropicMessages);
        assert_eq!(adapter.protocol_type(), ProtocolType::AnthropicMessages);
    }

    #[test]
    fn factory_creates_google_gemini() {
        let adapter = ProtocolAdapterFactory::create(ProtocolType::GoogleGemini);
        assert_eq!(adapter.protocol_type(), ProtocolType::GoogleGemini);
    }

    #[test]
    fn protected_headers_blocks_authorization() {
        assert!(is_protected_header("authorization"));
        assert!(is_protected_header("Authorization"));
        assert!(is_protected_header("AUTHORIZATION"));
    }

    #[test]
    fn protected_headers_blocks_content_type() {
        assert!(is_protected_header("content-type"));
        assert!(is_protected_header("Content-Type"));
    }

    #[test]
    fn protected_headers_allows_custom() {
        assert!(!is_protected_header("x-custom-header"));
        assert!(!is_protected_header("x-api-key"));
    }

    #[test]
    fn openai_auth_headers_include_bearer() {
        let adapter = OpenAiCompatibleAdapter;
        let headers = adapter.build_auth_headers("sk-test123");
        assert!(headers.contains_key(reqwest::header::AUTHORIZATION));
    }

    #[test]
    fn anthropic_auth_headers_include_x_api_key() {
        let adapter = AnthropicMessagesAdapter;
        let headers = adapter.build_auth_headers("sk-ant-test");
        assert!(headers.contains_key("x-api-key"));
        assert!(headers.contains_key("anthropic-version"));
    }
}
