use super::{ByteStream, Provider};
use crate::models::{ChatMessage, ChatRequest, ChatResponse, Choice, Usage};
use crate::Result;
use async_trait::async_trait;
use bytes::Bytes;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct CursorProvider {
    cursor_agent_path: String,
    timeout_ms: u64,
}

impl Default for CursorProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorProvider {
    pub fn new() -> Self {
        Self {
            cursor_agent_path: std::env::var("CURSOR_AGENT_EXECUTABLE")
                .unwrap_or_else(|_| "cursor-agent".to_string()),
            timeout_ms: 300_000, // 5 minutes default
        }
    }

    pub fn with_agent_path(mut self, path: String) -> Self {
        self.cursor_agent_path = path;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Build a text prompt from ChatRequest messages.
    fn build_prompt(&self, request: &ChatRequest) -> String {
        let mut lines = Vec::new();

        for msg in &request.messages {
            let role = msg.role.to_uppercase();
            lines.push(format!("{}: {}", role, msg.content));
        }

        lines.join("\n\n")
    }

    /// Build cursor-agent CLI arguments.
    fn build_args(&self, model: &str) -> Vec<String> {
        let model = if model.is_empty() || model == "auto" {
            "auto"
        } else {
            model
        };

        vec![
            "--print".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--stream-partial-output".to_string(),
            "--mode".to_string(),
            "ask".to_string(),
            "--model".to_string(),
            model.to_string(),
        ]
    }

    /// Parse a stream-json line and extract assistant text.
    ///
    /// cursor-agent outputs:
    /// `{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"..."}]}}`
    fn parse_stream_line(&self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let value: Value = serde_json::from_str(line).ok()?;

        if value.get("type")?.as_str()? != "assistant" {
            return None;
        }

        // Navigate: message.content[0].text
        value
            .get("message")?
            .get("content")?
            .as_array()?
            .iter()
            .filter_map(|block| {
                if block.get("type")?.as_str()? == "text" {
                    block.get("text")?.as_str()
                } else {
                    None
                }
            })
            .next()
            .map(|s| s.to_string())
    }
}

#[async_trait]
impl Provider for CursorProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let prompt = self.build_prompt(request);
        let args = self.build_args(&request.model);

        let mut child = Command::new(&self.cursor_agent_path)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                crate::error::YoloRouterError::RequestError(format!("cursor-agent failed: {}", e))
            })?;

        // Write prompt to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| {
            crate::error::YoloRouterError::RequestError("cursor-agent timeout".to_string())
        })?
        .map_err(|e| {
            crate::error::YoloRouterError::RequestError(format!("cursor-agent wait failed: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "cursor-agent exited with code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut content = String::new();
        let mut result_content: Option<String> = None;

        for line in stdout.lines() {
            if let Some(text) = self.parse_stream_line(line) {
                content.push_str(&text);
            }
            // Capture the final result event if present (avoids duplicate assistant chunks)
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if value.get("type").and_then(|t| t.as_str()) == Some("result") {
                    if let Some(text) = value.get("result").and_then(|r| r.as_str()) {
                        result_content = Some(text.to_string());
                    }
                }
            }
        }

        // Prefer the complete result over accumulated assistant chunks
        if let Some(result) = result_content {
            content = result;
        }

        if content.is_empty() {
            let stdout_preview = if stdout.trim().is_empty() {
                "(empty)".to_string()
            } else {
                format!("(first 500 chars): {}", &stdout[..stdout.len().min(500)])
            };
            return Err(crate::error::YoloRouterError::RequestError(format!(
                "cursor-agent produced no assistant content. stdout {}, stderr: {}",
                stdout_preview,
                if stderr.trim().is_empty() { "(empty)".to_string() } else { stderr.trim().to_string() }
            )));
        }

        Ok(ChatResponse {
            id: format!("cursor-{}", uuid::Uuid::new_v4()),
            model: request.model.clone(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    ..Default::default()
                },
                finish_reason: "stop".to_string(),
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            anthropic_content: None,
            anthropic_stop_sequence: None,
        })
    }

    async fn start_streaming_request(&self, request: &ChatRequest) -> Result<ByteStream> {
        let prompt = self.build_prompt(request);
        let model = request.model.clone();
        let cursor_agent_path = self.cursor_agent_path.clone();
        let args = self.build_args(&model);
        let timeout_ms = self.timeout_ms;

        let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<Bytes>>(128);

        tokio::spawn(async move {
            let mut child = match Command::new(&cursor_agent_path)
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx
                        .send(Err(std::io::Error::other(format!(
                            "Failed to spawn cursor-agent: {}",
                            e
                        ))))
                        .await;
                    return;
                }
            };

            // Write prompt to stdin
            if let Some(mut stdin) = child.stdin.take() {
                if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
                    let _ = tx
                        .send(Err(std::io::Error::other(format!(
                            "Failed to write to cursor-agent stdin: {}",
                            e
                        ))))
                        .await;
                    return;
                }
                let _ = stdin.shutdown().await;
            }

            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    let _ = tx
                        .send(Err(std::io::Error::other(
                            "cursor-agent stdout not available",
                        )))
                        .await;
                    return;
                }
            };

            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            let id = format!("cursor-{}", uuid::Uuid::new_v4());
            let created = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Yield SSE chunks
            loop {
                let line_result = tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_ms),
                    lines.next_line(),
                )
                .await;

                let line = match line_result {
                    Ok(Ok(Some(l))) => l,
                    Ok(Ok(None)) => break, // EOF
                    Ok(Err(e)) => {
                        let _ = tx
                            .send(Err(std::io::Error::other(format!(
                                "cursor-agent stdout read error: {}",
                                e
                            ))))
                            .await;
                        break;
                    }
                    Err(_) => {
                        let _ = tx
                            .send(Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "cursor-agent streaming timeout",
                            )))
                            .await;
                        break;
                    }
                };

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let value: Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let event_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match event_type {
                    "assistant" => {
                        let content = value
                            .get("message")
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_array())
                            .and_then(|arr| {
                                arr.iter()
                                    .find_map(|block| {
                                        if block.get("type")?.as_str()? == "text" {
                                            block.get("text")?.as_str()
                                        } else {
                                            None
                                        }
                                    })
                            });
                        if let Some(text) = content {
                            let chunk = serde_json::json!({
                                "id": &id,
                                "object": "chat.completion.chunk",
                                "created": created,
                                "model": format!("cursor/{}", model),
                                "choices": [{
                                    "index": 0,
                                    "delta": { "content": text },
                                    "finish_reason": null
                                }]
                            });
                            let sse = format!("data: {}\n\n", chunk);
                            if tx.send(Ok(Bytes::from(sse))).await.is_err() {
                                break;
                            }
                        }
                    }
                    "result" | "done" => {
                        let chunk = serde_json::json!({
                            "id": &id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": format!("cursor/{}", model),
                            "choices": [{
                                "index": 0,
                                "delta": {},
                                "finish_reason": "stop"
                            }]
                        });
                        let sse = format!("data: {}\n\n", chunk);
                        let _ = tx.send(Ok(Bytes::from(sse))).await;
                        break;
                    }
                    _ => {}
                }
            }

            // Yield [DONE]
            let _ = tx.send(Ok(Bytes::from("data: [DONE]\n\n"))).await;

            // Clean up child process
            let _ = child.wait().await;
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "cursor"
    }

    fn model_list(&self) -> Vec<String> {
        vec![
            "auto".to_string(),
            "composer-2-fast".to_string(),
            "composer-2".to_string(),
            "composer-1.5".to_string(),
            "gpt-5.4-high".to_string(),
            "gpt-5.4-high-fast".to_string(),
            "gpt-5.4-xhigh-fast".to_string(),
            "gpt-5.4-xhigh".to_string(),
            "gpt-5.4-medium".to_string(),
            "gpt-5.4-medium-fast".to_string(),
            "gpt-5.4-low".to_string(),
            "gpt-5.4-mini-none".to_string(),
            "gpt-5.4-mini-low".to_string(),
            "gpt-5.4-mini-medium".to_string(),
            "gpt-5.4-mini-high".to_string(),
            "gpt-5.4-mini-xhigh".to_string(),
            "gpt-5.4-nano-none".to_string(),
            "gpt-5.4-nano-low".to_string(),
            "gpt-5.4-nano-medium".to_string(),
            "gpt-5.4-nano-high".to_string(),
            "gpt-5.4-nano-xhigh".to_string(),
            "gpt-5.3-codex".to_string(),
            "gpt-5.3-codex-fast".to_string(),
            "gpt-5.3-codex-low".to_string(),
            "gpt-5.3-codex-low-fast".to_string(),
            "gpt-5.3-codex-high".to_string(),
            "gpt-5.3-codex-high-fast".to_string(),
            "gpt-5.3-codex-xhigh".to_string(),
            "gpt-5.3-codex-xhigh-fast".to_string(),
            "gpt-5.3-codex-spark-preview".to_string(),
            "gpt-5.3-codex-spark-preview-low".to_string(),
            "gpt-5.3-codex-spark-preview-high".to_string(),
            "gpt-5.3-codex-spark-preview-xhigh".to_string(),
            "gpt-5.2".to_string(),
            "gpt-5.2-fast".to_string(),
            "gpt-5.2-low".to_string(),
            "gpt-5.2-low-fast".to_string(),
            "gpt-5.2-high".to_string(),
            "gpt-5.2-high-fast".to_string(),
            "gpt-5.2-xhigh".to_string(),
            "gpt-5.2-xhigh-fast".to_string(),
            "gpt-5.2-codex".to_string(),
            "gpt-5.2-codex-fast".to_string(),
            "gpt-5.2-codex-low".to_string(),
            "gpt-5.2-codex-low-fast".to_string(),
            "gpt-5.2-codex-high".to_string(),
            "gpt-5.2-codex-high-fast".to_string(),
            "gpt-5.2-codex-xhigh".to_string(),
            "gpt-5.2-codex-xhigh-fast".to_string(),
            "gpt-5.1".to_string(),
            "gpt-5.1-low".to_string(),
            "gpt-5.1-high".to_string(),
            "gpt-5.1-codex-max-low".to_string(),
            "gpt-5.1-codex-max-low-fast".to_string(),
            "gpt-5.1-codex-max-medium".to_string(),
            "gpt-5.1-codex-max-medium-fast".to_string(),
            "gpt-5.1-codex-max-high".to_string(),
            "gpt-5.1-codex-max-high-fast".to_string(),
            "gpt-5.1-codex-max-xhigh".to_string(),
            "gpt-5.1-codex-max-xhigh-fast".to_string(),
            "gpt-5.1-codex-mini-low".to_string(),
            "gpt-5.1-codex-mini".to_string(),
            "gpt-5.1-codex-mini-high".to_string(),
            "gpt-5-mini".to_string(),
            "claude-opus-4-7-low".to_string(),
            "claude-opus-4-7-medium".to_string(),
            "claude-opus-4-7-high".to_string(),
            "claude-opus-4-7-xhigh".to_string(),
            "claude-opus-4-7-max".to_string(),
            "claude-opus-4-7-thinking-low".to_string(),
            "claude-opus-4-7-thinking-medium".to_string(),
            "claude-opus-4-7-thinking-high".to_string(),
            "claude-opus-4-7-thinking-xhigh".to_string(),
            "claude-opus-4-7-thinking-max".to_string(),
            "claude-4.6-sonnet-medium".to_string(),
            "claude-4.6-sonnet-medium-thinking".to_string(),
            "claude-4.6-opus-high".to_string(),
            "claude-4.6-opus-max".to_string(),
            "claude-4.6-opus-high-thinking".to_string(),
            "claude-4.6-opus-max-thinking".to_string(),
            "claude-4.5-opus-high".to_string(),
            "claude-4.5-opus-high-thinking".to_string(),
            "claude-4.5-sonnet".to_string(),
            "claude-4.5-sonnet-thinking".to_string(),
            "claude-4-sonnet".to_string(),
            "claude-4-sonnet-1m".to_string(),
            "claude-4-sonnet-thinking".to_string(),
            "claude-4-sonnet-1m-thinking".to_string(),
            "gemini-3.1-pro".to_string(),
            "gemini-3-flash".to_string(),
            "grok-4-20".to_string(),
            "grok-4-20-thinking".to_string(),
            "kimi-k2.5".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt() {
        let provider = CursorProvider::new();
        let request = ChatRequest {
            model: "auto".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                    ..Default::default()
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    ..Default::default()
                },
            ],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stream: None,
            system: None,
            anthropic: None,
            tools: None,
            tool_choice: None,
            stop_sequences: None,
        };

        let prompt = provider.build_prompt(&request);
        assert!(prompt.contains("SYSTEM: You are helpful."));
        assert!(prompt.contains("USER: Hello"));
    }

    #[test]
    fn test_build_args() {
        let provider = CursorProvider::new();
        let args = provider.build_args("sonnet-4.5");
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"sonnet-4.5".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--mode".to_string()));
        assert!(args.contains(&"ask".to_string()));
    }

    #[test]
    fn test_parse_stream_line() {
        let provider = CursorProvider::new();

        // cursor-agent actual format: message.content[0].text
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello world"}]}}"#;
        assert_eq!(
            provider.parse_stream_line(line),
            Some("Hello world".to_string())
        );

        // Non-assistant type should return None
        let line = r#"{"type":"thinking","message":{"role":"assistant","content":[{"type":"text","text":"..."}]}}"#;
        assert_eq!(provider.parse_stream_line(line), None);

        // Old format (flat content string) should return None
        let line = r#"{"type": "assistant", "content": "Hello world"}"#;
        assert_eq!(provider.parse_stream_line(line), None);
    }
}
