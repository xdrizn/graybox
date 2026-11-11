//! Runs one chat turn: `claude -p … --output-format stream-json` as a child process, its stdout
//! read on a thread and sent back as [`Event`]s. Stop kills only that child. Desktop only.
//!
//! The API is the one the icon feature will use too: describe a turn (a prompt, extra image files,
//! the conversation to continue), start it, poll its events.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use super::events::{Event, parse_line};
use super::guide::GUIDE;

/// How many characters of the CLI's own output are kept to explain a failure.
const TAIL: usize = 600;

/// Everything one turn needs.
#[derive(Clone, Debug)]
pub struct Turn {
    /// What the person typed.
    pub prompt: String,
    /// Image files Claude should look at (it is told to open each one).
    pub images: Vec<PathBuf>,
    /// The conversation to continue (`--resume`).
    pub session: Option<String>,
    /// The Claude Code CLI.
    pub claude: PathBuf,
    /// `figcraft-cli`, the MCP server that drives this app.
    pub cli: PathBuf,
    /// The app's control channel port on 127.0.0.1.
    pub port: u16,
}

/// A turn in progress.
pub struct Run {
    rx: Receiver<Event>,
    child: Arc<Mutex<Option<Child>>>,
    stopped: Arc<AtomicBool>,
}

/// The folder the CLI works in: not the repo, not the person's projects.
pub fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("graybox-claude")
}

/// The prompt Claude gets: the person's text, then the images to open.
pub fn full_prompt(turn: &Turn) -> String {
    if turn.images.is_empty() {
        return turn.prompt.clone();
    }
    let files: Vec<String> = turn.images.iter().map(|p| format!("- {}", p.display())).collect();
    format!("{}\n\nImages to look at (open each one with the Read tool):\n{}", turn.prompt, files.join("\n"))
}

/// The MCP config that attaches `figcraft-cli` to the app.
pub fn mcp_config(cli: &Path, port: u16) -> String {
    serde_json::json!({
        "mcpServers": {"graybox": {"command": cli.to_string_lossy(), "args": ["mcp", "--attach", port.to_string()]}}
    })
    .to_string()
}

/// The command line (after the program) for a turn whose MCP config is in `mcp`. The prompt goes
/// through stdin, not here.
pub fn args(turn: &Turn, mcp: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-p", "--output-format", "stream-json", "--verbose"].iter().map(OsString::from).collect();
    a.push("--mcp-config".into());
    a.push(mcp.as_os_str().to_owned());
    // Only the Graybox tools (and reading the images): no shell, no file edits, no user settings,
    // plugins or skills, and nothing asks for a permission nobody is there to answer.
    for flag in ["--strict-mcp-config", "--disable-slash-commands"] {
        a.push(flag.into());
    }
    for (flag, value) in [
        ("--setting-sources", "project"),
        ("--tools", "Read"),
        ("--allowedTools", "mcp__graybox Read"),
        ("--permission-mode", "dontAsk"),
        ("--append-system-prompt", GUIDE),
    ] {
        a.push(flag.into());
        a.push(value.into());
    }
    if let Some(session) = turn.session.as_deref().filter(|s| !s.is_empty()) {
        a.push("--resume".into());
        a.push(session.into());
    }
    a
}

impl Run {
    /// Start `turn`. `wake` is called whenever new events are waiting (it repaints the window).
    pub fn start(turn: &Turn, wake: impl Fn() + Send + 'static) -> Result<Run, String> {
        let dir = scratch_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't prepare a working folder for Claude: {e}"))?;
        let mcp = dir.join(format!("mcp-{}.json", turn.port));
        std::fs::write(&mcp, mcp_config(&turn.cli, turn.port)).map_err(|e| format!("Couldn't write Claude's settings: {e}"))?;
        let mut cmd = Command::new(&turn.claude);
        cmd.args(args(turn, &mcp)).current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // His plan, never a metered key that happens to be in the environment.
        cmd.env_remove("ANTHROPIC_API_KEY").env_remove("ANTHROPIC_AUTH_TOKEN");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashing up.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|e| format!("Couldn't start Claude Code: {e}"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        if let Some(mut stdin) = child.stdin.take() {
            let prompt = full_prompt(turn);
            // A thread: a prompt bigger than the pipe must not block the window.
            std::thread::spawn(move || {
                let _ = stdin.write_all(prompt.as_bytes());
            });
        }
        let (tx, rx) = channel();
        let child = Arc::new(Mutex::new(Some(child)));
        let stopped = Arc::new(AtomicBool::new(false));
        let tail = Arc::new(Mutex::new(String::new()));
        if let Some(stderr) = stderr {
            let tail = tail.clone();
            std::thread::spawn(move || keep_tail(stderr, &tail));
        }
        let (c, s) = (child.clone(), stopped.clone());
        std::thread::Builder::new()
            .name("graybox-claude".into())
            .spawn(move || pump(stdout, &tx, &c, &s, &tail, &wake))
            .map_err(|e| format!("Couldn't start Claude Code: {e}"))?;
        Ok(Run { rx, child, stopped })
    }

    /// The events that arrived since the last call.
    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }

    /// End this turn's process and what it started (and nothing else).
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(child) = self.child.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut() {
            kill_tree(child);
        }
    }
}

/// Kill `child` and the processes it started (Claude Code runs its tool servers as children).
fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let id = child.id().to_string();
        let _ =
            Command::new("taskkill").args(["/PID", &id, "/T", "/F"]).creation_flags(0x0800_0000).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
    let _ = child.kill();
}

impl Drop for Run {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The last [`TAIL`] characters of `r`, kept in `into` as it is read.
fn keep_tail(r: impl Read, into: &Mutex<String>) {
    for line in BufReader::new(r).lines().map_while(Result::ok) {
        let mut t = into.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        t.push_str(&line);
        t.push('\n');
        let extra = t.chars().count().saturating_sub(TAIL);
        if extra > 0 {
            *t = t.chars().skip(extra).collect();
        }
    }
}

/// Read the child's stdout to its end, then settle how the turn ended.
fn pump(stdout: Option<impl Read>, tx: &Sender<Event>, child: &Mutex<Option<Child>>, stopped: &AtomicBool, tail: &Mutex<String>, wake: &impl Fn()) {
    let mut done = false;
    if let Some(out) = stdout {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            for event in parse_line(&line) {
                done |= matches!(event, Event::Done { .. });
                let _ = tx.send(event);
            }
            wake();
        }
    }
    let status = child.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take().and_then(|mut c| c.wait().ok());
    if stopped.load(Ordering::Relaxed) {
        let _ = tx.send(Event::Stopped);
    } else if !done {
        let said = tail.lock().unwrap_or_else(std::sync::PoisonError::into_inner).trim().to_string();
        let error = if said.is_empty() {
            match status.and_then(|s| s.code()) {
                Some(code) => format!("Claude Code stopped with code {code} and said nothing."),
                None => "Claude Code stopped unexpectedly.".to_string(),
            }
        } else {
            said
        };
        let _ = tx.send(Event::Done { session: None, text: String::new(), error: Some(error) });
    }
    wake();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn() -> Turn {
        Turn {
            prompt: "make a frame".into(),
            images: Vec::new(),
            session: None,
            claude: PathBuf::from("claude"),
            cli: PathBuf::from("figcraft-cli"),
            port: 4321,
        }
    }

    fn strings(a: &[OsString]) -> Vec<String> {
        a.iter().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn a_first_turn_has_no_resume_and_only_graybox_tools() {
        let a = strings(&args(&turn(), Path::new("mcp.json")));
        assert!(a.windows(2).any(|w| w[0] == "--output-format" && w[1] == "stream-json"));
        assert!(a.contains(&"--verbose".to_string()));
        assert!(a.windows(2).any(|w| w[0] == "--mcp-config" && w[1] == "mcp.json"));
        assert!(a.windows(2).any(|w| w[0] == "--allowedTools" && w[1].contains("mcp__graybox")));
        assert!(a.contains(&"--strict-mcp-config".to_string()));
        assert!(!a.contains(&"--resume".to_string()));
        assert!(!a.contains(&"make a frame".to_string()), "the prompt goes through stdin");
    }

    #[test]
    fn a_follow_up_resumes_its_conversation() {
        let mut t = turn();
        t.session = Some("abc-123".into());
        let a = strings(&args(&t, Path::new("mcp.json")));
        assert!(a.windows(2).any(|w| w[0] == "--resume" && w[1] == "abc-123"));
        t.session = Some(String::new());
        assert!(!strings(&args(&t, Path::new("mcp.json"))).contains(&"--resume".to_string()));
    }

    #[test]
    fn images_are_named_in_the_prompt() {
        let mut t = turn();
        assert_eq!(full_prompt(&t), "make a frame");
        t.images = vec![PathBuf::from("a.png"), PathBuf::from("b.png")];
        let p = full_prompt(&t);
        assert!(p.starts_with("make a frame") && p.contains("- a.png") && p.contains("- b.png") && p.contains("Read tool"));
    }

    #[test]
    fn the_mcp_config_attaches_the_cli_to_the_port() {
        let v: serde_json::Value = serde_json::from_str(&mcp_config(Path::new("C:\\x\\figcraft-cli.exe"), 5555)).unwrap();
        assert_eq!(v["mcpServers"]["graybox"]["command"], "C:\\x\\figcraft-cli.exe");
        assert_eq!(v["mcpServers"]["graybox"]["args"], serde_json::json!(["mcp", "--attach", "5555"]));
    }

    #[test]
    fn the_tail_keeps_the_last_characters() {
        let into = Mutex::new(String::new());
        let text = (0..200).map(|i| format!("line number {i}\n")).collect::<String>();
        keep_tail(text.as_bytes(), &into);
        let t = into.lock().unwrap();
        assert!(t.chars().count() <= TAIL + 1);
        assert!(t.contains("line number 199"));
    }

    #[test]
    fn a_missing_program_is_a_plain_error() {
        let mut t = turn();
        t.claude = std::env::temp_dir().join("graybox-no-such-claude-program");
        let err = Run::start(&t, || {}).err().unwrap_or_default();
        assert!(err.starts_with("Couldn't start Claude Code"), "{err}");
    }
}
