//! What `claude -p --output-format stream-json` prints, one JSON object per line, turned into the
//! few events the chat panel shows. Pure parsing: junk, unknown kinds and half lines are skipped,
//! never an error.

use serde_json::Value;

/// One thing the chat shows or remembers.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The CLI started and named its conversation (`--resume` takes it for the next message).
    Started { session: String },
    /// A piece of Claude's reply, in order.
    Text(String),
    /// Claude is using one of the Graybox tools (or another): a plain-words line for the trace.
    Tool { name: String, line: String },
    /// The turn ended. `error` is the CLI's own message when it failed.
    Done { session: Option<String>, text: String, error: Option<String> },
    /// The person stopped the turn.
    Stopped,
}

/// Parse one stdout line into the events it holds (a message can hold several).
pub fn parse_line(line: &str) -> Vec<Event> {
    let line = line.trim();
    if !line.starts_with('{') {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    let session = v.get("session_id").and_then(Value::as_str).map(str::to_string);
    match v.get("type").and_then(Value::as_str) {
        Some("system") if v.get("subtype").and_then(Value::as_str) == Some("init") => {
            session.map(|session| Event::Started { session }).into_iter().collect()
        }
        Some("assistant") => assistant(&v),
        Some("result") => {
            let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let text = v.get("result").and_then(Value::as_str).unwrap_or("").to_string();
            let error = is_error.then(|| if text.is_empty() { "Claude Code stopped with an error.".to_string() } else { text.clone() });
            vec![Event::Done { session, text, error }]
        }
        _ => Vec::new(),
    }
}

fn assistant(v: &Value) -> Vec<Event> {
    let Some(parts) = v.pointer("/message/content").and_then(Value::as_array) else { return Vec::new() };
    let mut out = Vec::new();
    for part in parts {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = part.get("text").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                    out.push(Event::Text(t.to_string()));
                }
            }
            Some("tool_use") => {
                let name = part.get("name").and_then(Value::as_str).unwrap_or("tool").to_string();
                let line = tool_line(&name, part.get("input").unwrap_or(&Value::Null));
                out.push(Event::Tool { name, line });
            }
            _ => {}
        }
    }
    out
}

/// A tool call in plain words: "Creating a frame…". Graybox tools get their own line; others the
/// bare tool name.
pub fn tool_line(name: &str, input: &Value) -> String {
    let short = name.strip_prefix("mcp__graybox__").unwrap_or(name);
    let command = input.get("command").and_then(Value::as_str).unwrap_or("");
    match short {
        "create_node" => "Creating a layer…".to_string(),
        "set_properties" => "Changing a layer…".to_string(),
        "inspect_document" | "get_node" | "list_commands" | "inspect_ui" => "Looking at the file…".to_string(),
        "select" => "Selecting…".to_string(),
        "screenshot" | "export" => "Looking at the result…".to_string(),
        "undo" => "Undoing…".to_string(),
        "redo" => "Redoing…".to_string(),
        "save_file" => "Saving…".to_string(),
        "batch" => "Making several changes…".to_string(),
        "run_command" if !command.is_empty() => command_line(command),
        "run_command" => "Running a command…".to_string(),
        "Read" => "Reading a file…".to_string(),
        _ => format!("Using {short}…"),
    }
}

fn command_line(command: &str) -> String {
    let what = match command {
        "frame.create" => "a frame",
        "shape.rectangle" => "a rectangle",
        "shape.ellipse" => "an ellipse",
        "shape.line" => "a line",
        "text.create" | "shape.text" => "a text",
        _ => "",
    };
    if what.is_empty() { format!("Running {command}…") } else { format!("Drawing {what}…") }
}

/// Whether a CLI message means the account is not signed in (or the sign-in ran out).
pub fn is_sign_in_problem(message: &str) -> bool {
    let m = message.to_lowercase();
    ["/login", "not logged in", "authenticat", "oauth", "invalid api key", "sign in", "401"].iter().any(|k| m.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_names_the_session() {
        let e = parse_line(r#"{"type":"system","subtype":"init","session_id":"abc","tools":[]}"#);
        assert_eq!(e, vec![Event::Started { session: "abc".into() }]);
    }

    #[test]
    fn assistant_text_and_tools_come_in_order() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","name":"mcp__graybox__create_node","input":{}},{"type":"text","text":"  "}]}}"#;
        let e = parse_line(line);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0], Event::Text("Hello".into()));
        assert_eq!(e[1], Event::Tool { name: "mcp__graybox__create_node".into(), line: "Creating a layer…".into() });
    }

    #[test]
    fn run_command_names_what_it_draws() {
        let input = serde_json::json!({"command": "frame.create"});
        assert_eq!(tool_line("mcp__graybox__run_command", &input), "Drawing a frame…");
        let input = serde_json::json!({"command": "edit.undo"});
        assert_eq!(tool_line("mcp__graybox__run_command", &input), "Running edit.undo…");
        assert_eq!(tool_line("mcp__graybox__run_command", &serde_json::json!({})), "Running a command…");
    }

    #[test]
    fn result_ends_the_turn_with_or_without_error() {
        let ok = parse_line(r#"{"type":"result","subtype":"success","is_error":false,"result":"Done.","session_id":"s1"}"#);
        assert_eq!(ok, vec![Event::Done { session: Some("s1".into()), text: "Done.".into(), error: None }]);
        let bad = parse_line(r#"{"type":"result","is_error":true,"result":"Not logged in · Please run /login"}"#);
        assert_eq!(
            bad,
            vec![Event::Done {
                session: None,
                text: "Not logged in · Please run /login".into(),
                error: Some("Not logged in · Please run /login".into())
            }]
        );
        let empty = parse_line(r#"{"type":"result","is_error":true}"#);
        assert!(matches!(&empty[0], Event::Done { error: Some(e), .. } if !e.is_empty()));
    }

    #[test]
    fn junk_lines_make_no_events() {
        for line in [
            "",
            "   ",
            "not json",
            "{",
            "{\"type\":",
            "[1,2]",
            "null",
            "{}",
            r#"{"type":42}"#,
            r#"{"type":"assistant"}"#,
            r#"{"type":"assistant","message":{"content":"text"}}"#,
            r#"{"type":"assistant","message":{"content":[1,null,{"type":"tool_use"},{"type":"text"}]}}"#,
            r#"{"type":"system","subtype":"api_retry","session_id":"x"}"#,
            r#"{"type":"rate_limit_event"}"#,
            "\u{0}\u{1}{\u{7f}",
        ] {
            let e = parse_line(line);
            // `{"type":"assistant"... tool_use}` with no name still names a tool; nothing panics.
            assert!(e.len() <= 1, "{line:?} gave {e:?}");
        }
    }

    #[test]
    fn sign_in_problems_are_recognised() {
        assert!(is_sign_in_problem("Failed to authenticate. API Error: 401 OAuth access token has expired."));
        assert!(is_sign_in_problem("Please run /login"));
        assert!(!is_sign_in_problem("Could not reach the file"));
    }
}
