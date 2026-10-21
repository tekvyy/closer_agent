//! Translates Codex CLI `exec --json` events into Claude stream-json lines
//!
//! The rest of the pipeline (log persistence, the frontend stream parser)
//! understands Claude's stream-json format. Rather than teaching every
//! consumer a second format, Codex output is converted line by line into the
//! equivalent Claude events before it enters the StreamProcessor.

use serde_json::{json, Value};
use std::time::Instant;

pub struct CodexStreamTranslator {
    model: String,
    started_at: Instant,
    turns: u32,
}

impl CodexStreamTranslator {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            started_at: Instant::now(),
            turns: 0,
        }
    }

    /// Convert one Codex output line into zero or more Claude stream-json lines.
    /// Lines that are not JSON are passed through unchanged.
    pub fn translate(&mut self, line: &str) -> Vec<String> {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return vec![line.to_string()];
        };
        let str_field = |value: &Value, key: &str| -> String {
            value
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let events: Vec<Value> = match event.get("type").and_then(|t| t.as_str()) {
            Some("thread.started") => vec![json!({
                "type": "system",
                "subtype": "init",
                "session_id": str_field(&event, "thread_id"),
                "model": self.model,
            })],
            Some("turn.started") => {
                self.turns += 1;
                vec![]
            }
            Some("item.started") => match event.get("item") {
                Some(item)
                    if item.get("type").and_then(|t| t.as_str()) == Some("command_execution") =>
                {
                    vec![tool_use(
                        item,
                        "Bash",
                        json!({ "command": str_field(item, "command") }),
                    )]
                }
                _ => vec![],
            },
            Some("item.completed") => match event.get("item") {
                Some(item) => self.translate_completed_item(item),
                None => vec![],
            },
            Some("turn.completed") => vec![json!({
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "duration_ms": self.elapsed_ms(),
                "num_turns": self.turns,
                "usage": event.get("usage").cloned().unwrap_or(Value::Null),
            })],
            Some("turn.failed") => {
                let message = event
                    .get("error")
                    .map(|e| str_field(e, "message"))
                    .unwrap_or_default();
                vec![json!({
                    "type": "result",
                    "subtype": "error_during_execution",
                    "is_error": true,
                    "duration_ms": self.elapsed_ms(),
                    "num_turns": self.turns,
                    "errors": [message],
                })]
            }
            Some("error") => {
                vec![json!({ "type": "error", "error": str_field(&event, "message") })]
            }
            _ => vec![],
        };

        events.iter().map(Value::to_string).collect()
    }

    fn translate_completed_item(&self, item: &Value) -> Vec<Value> {
        let text = |key: &str| {
            item.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        match item.get("type").and_then(|t| t.as_str()) {
            Some("agent_message") => {
                vec![assistant(json!({ "type": "text", "text": text("text") }))]
            }
            Some("reasoning") => {
                vec![assistant(
                    json!({ "type": "thinking", "thinking": text("text") }),
                )]
            }
            Some("web_search") => {
                let count = item
                    .get("results")
                    .and_then(|r| r.as_array())
                    .map(|r| r.len())
                    .unwrap_or(0);
                vec![
                    tool_use(item, "WebSearch", json!({ "query": text("query") })),
                    tool_result(item, &format!("{count} search results"), false),
                ]
            }
            Some("command_execution") => {
                let exit_code = item.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(0);
                let output = text("aggregated_output");
                let content = if output.is_empty() {
                    format!("Exit code {exit_code}")
                } else {
                    output
                };
                vec![tool_result(item, &content, exit_code != 0)]
            }
            Some("file_change") => {
                let paths: Vec<&str> = item
                    .get("changes")
                    .and_then(|c| c.as_array())
                    .map(|changes| {
                        changes
                            .iter()
                            .filter_map(|change| change.get("path").and_then(|p| p.as_str()))
                            .collect()
                    })
                    .unwrap_or_default();
                vec![tool_use(
                    item,
                    "Write",
                    json!({ "file_path": paths.join(", ") }),
                )]
            }
            Some("mcp_tool_call") => {
                let name = format!("mcp__{}__{}", text("server"), text("tool"));
                let input = item.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let mut events = vec![tool_use(item, &name, input)];
                if item.get("status").and_then(|s| s.as_str()) == Some("failed") {
                    let message = item
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("MCP tool call failed");
                    events.push(tool_result(item, message, true));
                }
                events
            }
            Some("error") => vec![json!({ "type": "error", "error": text("message") })],
            _ => vec![],
        }
    }

    fn elapsed_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
    }
}

fn item_id(item: &Value) -> &str {
    item.get("id").and_then(|i| i.as_str()).unwrap_or("")
}

fn assistant(block: Value) -> Value {
    json!({ "type": "assistant", "message": { "content": [block] } })
}

fn tool_use(item: &Value, name: &str, input: Value) -> Value {
    assistant(json!({ "type": "tool_use", "id": item_id(item), "name": name, "input": input }))
}

fn tool_result(item: &Value, content: &str, is_error: bool) -> Value {
    json!({
        "type": "user",
        "message": { "content": [{
            "type": "tool_result",
            "tool_use_id": item_id(item),
            "content": content,
            "is_error": is_error,
        }] }
    })
}

#[cfg(test)]
mod tests {
    use super::CodexStreamTranslator;
    use serde_json::Value;

    fn translate(translator: &mut CodexStreamTranslator, line: &str) -> Vec<Value> {
        translator
            .translate(line)
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn translates_session_start_to_init() {
        let mut t = CodexStreamTranslator::new("gpt-6.1-sol");
        let out = translate(&mut t, r#"{"type":"thread.started","thread_id":"abc"}"#);
        assert_eq!(out[0]["type"], "system");
        assert_eq!(out[0]["subtype"], "init");
        assert_eq!(out[0]["session_id"], "abc");
        assert_eq!(out[0]["model"], "gpt-6.1-sol");
    }

    #[test]
    fn translates_agent_message_to_assistant_text() {
        let mut t = CodexStreamTranslator::new("m");
        let out = translate(
            &mut t,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"ok"}}"#,
        );
        assert_eq!(out[0]["message"]["content"][0]["type"], "text");
        assert_eq!(out[0]["message"]["content"][0]["text"], "ok");
    }

    #[test]
    fn translates_web_search_to_tool_use_and_result() {
        let mut t = CodexStreamTranslator::new("m");
        let out = translate(
            &mut t,
            r#"{"type":"item.completed","item":{"id":"item_1","type":"web_search","query":"Stripe","results":[{},{}]}}"#,
        );
        assert_eq!(out[0]["message"]["content"][0]["name"], "WebSearch");
        assert_eq!(out[0]["message"]["content"][0]["input"]["query"], "Stripe");
        assert_eq!(
            out[1]["message"]["content"][0]["content"],
            "2 search results"
        );
    }

    #[test]
    fn translates_failed_command_to_error_result() {
        let mut t = CodexStreamTranslator::new("m");
        let started = translate(
            &mut t,
            r#"{"type":"item.started","item":{"id":"item_2","type":"command_execution","command":"ls"}}"#,
        );
        assert_eq!(started[0]["message"]["content"][0]["name"], "Bash");
        let done = translate(
            &mut t,
            r#"{"type":"item.completed","item":{"id":"item_2","type":"command_execution","command":"ls","aggregated_output":"","exit_code":2}}"#,
        );
        assert_eq!(done[0]["message"]["content"][0]["is_error"], true);
        assert_eq!(done[0]["message"]["content"][0]["content"], "Exit code 2");
    }

    #[test]
    fn translates_turn_outcomes_to_results() {
        let mut t = CodexStreamTranslator::new("m");
        assert!(translate(&mut t, r#"{"type":"turn.started"}"#).is_empty());
        let ok = translate(&mut t, r#"{"type":"turn.completed","usage":{}}"#);
        assert_eq!(ok[0]["subtype"], "success");
        assert_eq!(ok[0]["num_turns"], 1);
        let failed = translate(
            &mut t,
            r#"{"type":"turn.failed","error":{"message":"boom"}}"#,
        );
        assert_eq!(failed[0]["is_error"], true);
        assert_eq!(failed[0]["errors"][0], "boom");
    }

    #[test]
    fn passes_through_non_json_and_drops_unknown_events() {
        let mut t = CodexStreamTranslator::new("m");
        assert_eq!(t.translate("plain text"), vec!["plain text".to_string()]);
        assert!(t.translate(r#"{"type":"something.new"}"#).is_empty());
    }
}
