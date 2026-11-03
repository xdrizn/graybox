use std::sync::{Arc, Mutex};

use figcraft_collab::message::ClientMsg;
use serde_json::{Value, json};

use crate::Session;
use crate::live::{Event, HostOptions, LiveLink, Presence, PublishOptions, Published, SessionInfo, Shared, SharedLink};

const VIEW: &str = "ViewSecretViewSecret000";
const EDIT: &str = "EditSecretEditSecret000";
const OWNER: &str = "OwnerSecretOwnerSecret00";
const KEY: &str = "ServerKeyServerKey000";

#[derive(Default)]
struct Script {
    published: Vec<(u64, PublishOptions)>,
    events: Vec<(u64, Event)>,
}
struct Link(Arc<Mutex<Script>>);
impl LiveLink for Link {
    fn host(&mut self, _: u64, _: HostOptions) -> Result<SessionInfo, String> {
        Err("this test adapter only publishes".into())
    }
    fn join(&mut self, _: u64, _: &str, _: &str) -> Result<(), String> {
        Err("this test adapter only publishes".into())
    }
    fn publish(&mut self, id: u64, options: PublishOptions) -> Result<(), String> {
        self.0.lock().unwrap().published.push((id, options));
        Ok(())
    }
    fn leave(&mut self, _: u64) {}
    fn presence(&mut self, _: u64, _: &Presence) {}
    fn send(&mut self, _: u64, _: ClientMsg) {}
    fn poll(&mut self) -> Vec<(u64, Event)> {
        std::mem::take(&mut self.0.lock().unwrap().events)
    }
}
fn setup() -> (Session, Arc<Mutex<Script>>) {
    let mut session = Session::new();
    let script = Arc::new(Mutex::new(Script::default()));
    session.live.install(Box::new(Link(script.clone())));
    (session, script)
}
fn execute(s: &mut Session, command: &str, args: Value) -> Value {
    s.execute(command, &args).unwrap()
}
fn invite(token: &str) -> String {
    format!("figcraft://join?server=wss://server.example/live/&file=f1&token={token}")
}
fn published(s: &mut Session, script: &Arc<Mutex<Script>>, view_token: &str) {
    let mut script = script.lock().unwrap();
    let id = script.published.last().unwrap().0;
    let event = Published {
        server: "wss://server.example/live/".into(),
        file: "f1".into(),
        name: "Prototype".into(),
        edit: invite(EDIT),
        view: invite(VIEW),
        owner: Some(OWNER.into()),
        expires: None,
    };
    script.events.push((id, Event::Published(event)));
    let link = |token: &str| SharedLink { id: token.into(), link: invite(token), code: None };
    script.events.push((id, Event::Shared(Ok(Shared { edit: link(EDIT), view: Some(link(view_token)), comment: None }))));
    drop(script);
    s.live_poll();
}
fn unescape(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(&value[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn real_share_command_waits_for_upstream_and_returns_only_view_invitation() {
    let (mut s, script) = setup();
    execute(&mut s, "game.configure", json!({"webUrl": "https://web.example/editor/?webgl&lang=pt%2DBR#old-invitation"}));
    let response = execute(&mut s, "game.share", json!({"server": format!("wss://server.example/live/?key={KEY}")}));
    assert_eq!(response["status"], "connecting");
    assert_eq!(response["ready"], false);
    assert!(response["link"].is_null(), "a click alone never produces a ready link");
    {
        let script = script.lock().unwrap();
        let options = &script.published[0].1;
        assert!(options.share, "upstream must create the invitation, not a local backend");
        assert_eq!(options.server, format!("wss://server.example/live/?key={KEY}"));
    }
    published(&mut s, &script, VIEW);
    let response = execute(&mut s, "game.share.status", json!({}));
    assert_eq!(response["ready"], true);
    let link = url::Url::parse(response["link"].as_str().unwrap()).unwrap();
    assert_eq!(link.scheme(), "https");
    assert_eq!(link.host_str(), Some("web.example"));
    assert_eq!(link.path(), "/editor/");
    assert_eq!(link.query(), Some("webgl&lang=pt%2DBR&play"));
    let fragment = link.fragment().unwrap();
    assert!(fragment.starts_with("join=figcraft%3A%2F%2Fjoin%3F"));
    assert_eq!(unescape(fragment.strip_prefix("join=").unwrap()), invite(VIEW));
    for secret in [EDIT, OWNER, KEY] {
        assert!(!response.to_string().contains(secret), "the API returns no privileged credentials");
        assert!(!link.as_str().contains(secret));
    }
    assert!(!link.query().unwrap().contains(VIEW), "the secret is only in the fragment");
    assert_eq!(script.lock().unwrap().published.len(), 1, "status reads don't republish");
    let again = execute(&mut s, "game.share", json!({}));
    assert_eq!(again["link"], response["link"]);
    assert_eq!(script.lock().unwrap().published.len(), 1, "repeat sharing reuses upstream session");
}

#[test]
fn invitation_is_encoded_once_without_query_or_fragment_boundary_confusion() {
    let (mut s, script) = setup();
    execute(&mut s, "game.share", json!({"webUrl": "http://localhost:8080/?play&x=a%26b#join=old", "server": "ws://127.0.0.1:47900/"}));
    // Escapes in an upstream endpoint remain escaped after one browser decode.
    let token = "ReadOnly%2BToken%25Token";
    published(&mut s, &script, token);
    let state = execute(&mut s, "game.share.status", json!({"webUrl": "http://localhost:8080/?play&x=a%26b#ignored"}));
    let link = url::Url::parse(state["link"].as_str().unwrap()).unwrap();
    assert_eq!(link.query(), Some("play&x=a%26b"));
    assert_eq!(unescape(link.fragment().unwrap().strip_prefix("join=").unwrap()), invite(token));
    assert!(!link.fragment().unwrap().contains('&'));
}

#[test]
fn privileged_tokens_never_become_play_links_even_if_server_mislabels_them() {
    for token in [EDIT, OWNER] {
        let (mut s, script) = setup();
        execute(&mut s, "game.share", json!({"webUrl": "https://web.example/", "server": "ws://localhost:47900/"}));
        published(&mut s, &script, token);
        let state = execute(&mut s, "game.share.status", json!({"webUrl": "https://web.example/"}));
        assert_eq!(state["ready"], false);
        assert!(state["link"].is_null());
    }
}

#[test]
fn unsafe_browser_bases_fail_before_any_publication_and_configuration_is_undoable() {
    let (mut s, script) = setup();
    for base in [
        "",
        "javascript:alert(1)",
        "file:///tmp/index.html",
        "https://owner:password@web.example/",
        "https://web.example/?token=secret",
        "https://web.example/?%6Bey=secret",
        "https://web.ex\nample/",
    ] {
        assert!(s.execute("game.share", &json!({"webUrl": base})).is_err(), "{base:?}");
    }
    assert!(script.lock().unwrap().published.is_empty());
    execute(&mut s, "game.configure", json!({"webUrl": "http://localhost:8080/"}));
    assert_eq!(s.doc().unwrap().game.web_url, "http://localhost:8080/");
    execute(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().game.web_url.is_empty());
}

#[test]
fn removed_view_links_do_not_fall_back_to_the_original_published_invitation() {
    let (mut s, script) = setup();
    execute(&mut s, "game.share", json!({"webUrl": "https://web.example/", "server": "ws://localhost:47900/"}));
    published(&mut s, &script, VIEW);
    s.live_mut().unwrap().share.as_mut().unwrap().links.as_mut().unwrap().view = None;
    let response = execute(&mut s, "game.share.status", json!({"webUrl": "https://web.example/"}));
    assert_eq!(response["ready"], false);
    assert!(response["link"].is_null());
    s.live_mut().unwrap().share.as_mut().unwrap().links = None;
    let response = execute(&mut s, "game.share.status", json!({"webUrl": "https://web.example/"}));
    assert_eq!(response["ready"], false);
    assert!(response["link"].is_null());
}
