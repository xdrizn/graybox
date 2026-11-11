//! The chat's memory: the messages of each open file's conversation and how events change them.
//! UI-only and in memory: nothing is saved in the `.figcraft` file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::events::{Event, is_sign_in_problem};

/// One line of the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    User(String),
    Claude(String),
    /// A small remark in the flow ("Stopped.").
    Note(String),
    /// Something went wrong, in plain words.
    Error(String),
}

/// A message waiting for the control channel to open before it can start.
#[derive(Clone, Debug, PartialEq)]
pub struct Queued {
    pub text: String,
    pub images: Vec<PathBuf>,
    /// The time (egui seconds) it was queued.
    pub since: f64,
}

/// One file's conversation with Claude.
#[derive(Default)]
pub struct Conversation {
    pub messages: Vec<Message>,
    /// The text box.
    pub input: String,
    /// The Claude Code conversation to continue (`--resume`).
    pub session: Option<String>,
    /// What Claude is doing right now, as one line.
    pub trace: Option<String>,
    /// A turn is running or about to run.
    pub busy: bool,
    /// Claude has written text in the running turn.
    pub got_text: bool,
    pub queued: Option<Queued>,
    #[cfg(not(target_arch = "wasm32"))]
    pub run: Option<super::runner::Run>,
}

/// The chat window and every file's conversation.
pub struct Chat {
    pub open: bool,
    /// The panel's size, changed by dragging its top-left corner.
    pub size: (f32, f32),
    /// The box takes the keyboard on the next frame.
    pub focus: bool,
    pub convs: BTreeMap<u64, Conversation>,
}

impl Default for Chat {
    fn default() -> Self {
        Self { open: false, size: (380.0, 520.0), focus: false, convs: BTreeMap::new() }
    }
}

/// An error from the CLI, in plain words for a designer.
pub fn plain_error(raw: &str) -> String {
    if is_sign_in_problem(raw) {
        return "Claude Code isn't signed in, or its sign-in ran out. Open a terminal, type claude auth login, sign in, then ask again here."
            .to_string();
    }
    let raw = raw.trim();
    let last: Vec<&str> = raw.lines().rev().take(4).collect();
    let tail: Vec<&str> = last.into_iter().rev().collect();
    format!("Claude Code stopped before it could answer.\n{}", tail.join("\n"))
}

impl Conversation {
    /// Forget the conversation (New chat). A running turn is stopped.
    pub fn reset(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(run) = self.run.take() {
            run.stop();
        }
        self.messages.clear();
        self.session = None;
        self.trace = None;
        self.busy = false;
        self.got_text = false;
        self.queued = None;
    }

    /// Show a message the person sent and mark the turn as started.
    pub fn begin_turn(&mut self, text: &str) {
        self.messages.push(Message::User(text.to_string()));
        self.busy = true;
        self.got_text = false;
        self.trace = Some("Starting…".to_string());
    }

    /// The turn ended badly before it could start (no CLI, no port…).
    pub fn fail(&mut self, message: impl Into<String>) {
        self.messages.push(Message::Error(message.into()));
        self.busy = false;
        self.trace = None;
        self.queued = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.run = None;
        }
    }

    /// Take one event from the running turn.
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Started { session } => self.session = Some(session),
            Event::Text(text) => {
                self.got_text = true;
                self.trace = None;
                match self.messages.last_mut() {
                    Some(Message::Claude(last)) => {
                        last.push_str("\n\n");
                        last.push_str(&text);
                    }
                    _ => self.messages.push(Message::Claude(text)),
                }
            }
            Event::Tool { line, .. } => self.trace = Some(line),
            Event::Done { session, text, error } => {
                if let Some(s) = session.filter(|s| !s.is_empty()) {
                    self.session = Some(s);
                }
                match error {
                    Some(raw) => {
                        // The CLI also writes its error as a Claude message: show it once.
                        if matches!(self.messages.last(), Some(Message::Claude(last)) if last.trim() == raw.trim()) {
                            self.messages.pop();
                        }
                        self.messages.push(Message::Error(plain_error(&raw)));
                    }
                    None if !self.got_text && !text.trim().is_empty() => self.messages.push(Message::Claude(text)),
                    None => {}
                }
                self.finish();
            }
            Event::Stopped => {
                self.messages.push(Message::Note("Stopped.".to_string()));
                self.finish();
            }
        }
    }

    fn finish(&mut self) {
        self.busy = false;
        self.trace = None;
        self.queued = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.run = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(error: Option<&str>, text: &str) -> Event {
        Event::Done { session: Some("s".into()), text: text.into(), error: error.map(str::to_string) }
    }

    #[test]
    fn a_turn_collects_text_into_one_reply() {
        let mut c = Conversation::default();
        c.begin_turn("make a frame");
        assert!(c.busy);
        c.apply(Event::Started { session: "s1".into() });
        c.apply(Event::Tool { name: "t".into(), line: "Drawing a frame…".into() });
        assert_eq!(c.trace.as_deref(), Some("Drawing a frame…"));
        c.apply(Event::Text("Made it.".into()));
        c.apply(Event::Text("Anything else?".into()));
        c.apply(done(None, "Anything else?"));
        assert_eq!(c.messages, vec![Message::User("make a frame".into()), Message::Claude("Made it.\n\nAnything else?".into())]);
        assert!(!c.busy && c.trace.is_none());
        assert_eq!(c.session.as_deref(), Some("s"));
    }

    #[test]
    fn a_result_text_is_used_when_no_text_came_before() {
        let mut c = Conversation::default();
        c.begin_turn("hi");
        c.apply(done(None, "Hello!"));
        assert_eq!(c.messages.last(), Some(&Message::Claude("Hello!".into())));
    }

    #[test]
    fn a_failed_turn_says_what_happened_in_plain_words() {
        let mut c = Conversation::default();
        c.begin_turn("hi");
        c.messages.push(Message::Claude("Failed to authenticate. API Error: 401 OAuth access token has expired.".into()));
        c.apply(done(Some("Failed to authenticate. API Error: 401 OAuth access token has expired."), ""));
        assert_eq!(c.messages.iter().filter(|m| matches!(m, Message::Claude(_))).count(), 0, "the CLI's own error line is not shown twice");
        assert!(matches!(c.messages.last(), Some(Message::Error(m)) if m.contains("signed in") && m.contains("claude")));
        assert!(!c.busy);
        let mut c = Conversation::default();
        c.begin_turn("hi");
        c.apply(done(Some("boom\nsecond\nthird"), ""));
        assert!(matches!(c.messages.last(), Some(Message::Error(m)) if m.contains("third")));
    }

    #[test]
    fn stop_and_new_chat() {
        let mut c = Conversation::default();
        c.begin_turn("hi");
        c.apply(Event::Stopped);
        assert_eq!(c.messages.last(), Some(&Message::Note("Stopped.".into())));
        assert!(!c.busy);
        c.session = Some("x".into());
        c.reset();
        assert!(c.messages.is_empty() && c.session.is_none());
    }

    #[test]
    fn the_second_message_keeps_the_session() {
        let mut c = Conversation::default();
        c.begin_turn("one");
        c.apply(done(None, "ok"));
        c.begin_turn("two");
        assert_eq!(c.session.as_deref(), Some("s"));
        assert_eq!(c.messages.iter().filter(|m| matches!(m, Message::User(_))).count(), 2);
    }
}
