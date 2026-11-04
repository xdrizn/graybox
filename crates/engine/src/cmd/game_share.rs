//! Browser Play links use upstream online sharing, never an edit/owner credential. The invite
//! stays percent-encoded in the fragment so neither the web host nor Referer sees its secret.
//! `game.share` starts publication; poll `game.share.status` after `Session::live_poll` for readiness.

use serde_json::{Value, json};
use url::Url;

use super::CommandSpec;
use crate::live::{Access, Status, Via};
use crate::params::req_string;
use crate::{Result, Session, bad, spec};

const CMD: &str = "game.share";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "game.configure",
            "Configure browser Play URL",
            "{webUrl (absolute http(s) browser build URL; empty clears)} → {webUrl}: saved in this design file; contains no invitation or server credentials",
            true,
            configure
        ),
        spec!(
            "game.share",
            "Share browser Play link",
            "{webUrl? (default: this file's configured browser build URL), server? (default: upstream sharing server preference; may contain its publishing key)} → {session, status, ready, link (read-only invitation in #join, only when ready), access: view, reason}: asynchronously publishes through collab.share; poll game.share.status after live events",
            false,
            share
        ),
        spec!(
            "game.share.status",
            "Browser Play sharing status",
            "{webUrl? (default: this file's configured browser build URL)} → {session, status, ready, link, access: view, reason}: reads current sharing state without publishing or exposing edit/owner credentials",
            false,
            status
        ),
    ]
}

/// A public browser base, without credentials or a previous invitation. Preserve unrelated
/// query entries (for example `webgl`); the old fragment is deliberately replaced, never copied.
pub fn browser_base(value: &str) -> Result<Url> {
    let value = value.trim();
    if value.len() > 4096 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(bad(CMD, "webUrl must be an absolute http(s) browser build URL without whitespace"));
    }
    let mut url = Url::parse(value).map_err(|_| bad(CMD, "webUrl must be an absolute http(s) browser build URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err(bad(CMD, "webUrl must be an http(s) browser build URL without user credentials"));
    }
    if url.query_pairs().any(|(k, _)| matches!(k.to_ascii_lowercase().as_str(), "join" | "token" | "key" | "owner" | "edit")) {
        return Err(bad(CMD, "webUrl must not contain invitation or server credentials in its query"));
    }
    url.set_fragment(None);
    Ok(url)
}

fn configure(s: &mut Session, p: &Value) -> Result<Value> {
    let value = req_string("game.configure", p, "webUrl")?;
    let base = if value.trim().is_empty() { String::new() } else { browser_base(&value)?.to_string() };
    s.doc_mut()?.game.web_url = base.clone();
    Ok(json!({"webUrl": base}))
}

fn base(s: &Session, p: &Value) -> Result<Url> {
    let value = match p.get("webUrl") {
        Some(_) => req_string(CMD, p, "webUrl")?,
        None => s.doc()?.game.web_url.clone(),
    };
    if value.trim().is_empty() {
        return Err(bad(CMD, "set webUrl to the address hosting your browser build first (game.configure)"));
    }
    browser_base(&value)
}

fn share(s: &mut Session, p: &Value) -> Result<Value> {
    s.doc()?;
    let base = base(s, p)?;
    let live = s.live();
    // Never replace a joined document's session with a fresh publication. Its existing view
    // invitation can be reused, but an edit-only guest/local host needs an owner's online share.
    if live.status.active() && live.share.is_none() {
        if readonly_invite(s).is_none() {
            return Err(bad(
                CMD,
                "this session has no read-only server invitation; ask its owner to share online, or leave the session before publishing your own copy",
            ));
        }
    } else {
        // Graybox: the font store fonts the text uses go with the file, so viewers see them.
        crate::cmd::game::embed_shared_fonts(s)?;
        let mut args = json!({"via": "online", "access": "view"});
        if p.get("server").is_some() {
            args["server"] = json!(req_string(CMD, p, "server")?);
        }
        // Upstream owns asynchronous upload, publication, tokens, permissions and persistence.
        // Its full summary is intentionally discarded: it includes owner and edit credentials.
        s.execute("collab.share", &args)?;
    }
    state_with_base(s, base)
}

fn status(s: &mut Session, p: &Value) -> Result<Value> {
    s.doc()?;
    state_with_base(s, base(s, p)?)
}

/// Read-only observation for the sharing panel; it never starts network work. A link is ready
/// once the server has actually supplied a view invitation, not merely on a publish click.
pub fn state(s: &Session, web_url: Option<&str>) -> Result<Value> {
    s.doc()?;
    let p = web_url.map_or_else(|| json!({}), |url| json!({"webUrl": url}));
    state_with_base(s, base(s, &p)?)
}

fn readonly_invite(s: &Session) -> Option<(&str, Option<&str>)> {
    let live = s.live();
    if !live.status.active() {
        return None;
    }
    if let Some(share) = &live.share {
        if share.via != Via::Online {
            return None;
        }
        let links = share.links.as_ref()?;
        return Some((&links.view.as_ref()?.link, Some(&links.edit.link)));
    }
    if let Some(published) = &live.published {
        return Some((&published.view, Some(&published.edit)));
    }
    let info = live.info.as_ref().filter(|info| info.access == Access::View)?;
    Some((&info.link, None))
}

/// Accept only a token-bearing upstream invitation, never a URL containing owner/server keys.
fn invitation_token(link: &str) -> Option<String> {
    if link.len() > 2048 {
        return None;
    }
    let url = Url::parse(link).ok()?;
    if url.scheme() != "figcraft"
        || url.host_str() != Some("join")
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let mut token = None;
    for (key, value) in url.query_pairs() {
        if matches!(key.to_ascii_lowercase().as_str(), "owner" | "key" | "edit") {
            return None;
        }
        if key == "token" {
            if token.is_some() || value.is_empty() {
                return None;
            }
            token = Some(value.into_owned());
        }
    }
    token
}

fn state_with_base(s: &Session, mut base: Url) -> Result<Value> {
    let live = s.live();
    let invite = readonly_invite(s).and_then(|(view, edit)| {
        let token = invitation_token(view)?;
        if edit.and_then(invitation_token).is_some_and(|edit_token| edit_token == token)
            || live.published.as_ref().and_then(|p| invitation_token(&p.edit)).is_some_and(|edit_token| edit_token == token)
            || live.published.as_ref().and_then(|p| p.owner.as_ref()).is_some_and(|owner| *owner == token)
        {
            return None;
        }
        Some(view)
    });
    let link = invite.map(|invite| {
        // Keep a bare flag, not `play=`, matching the browser's other opt-in query switches.
        if !base.query_pairs().any(|(key, _)| key == "play") {
            let query = match base.query().filter(|q| !q.is_empty()) {
                Some(q) => format!("{q}&play"),
                None => "play".into(),
            };
            base.set_query(Some(&query));
        }
        let mut fragment = String::with_capacity(5 + invite.len() * 3);
        fragment.push_str("join=");
        // RFC 3986 component encoding: importantly &, ?, #, % and + cannot cross boundaries.
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for byte in invite.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                fragment.push(char::from(byte));
            } else {
                fragment.push('%');
                fragment.push(char::from(HEX[usize::from(byte >> 4)]));
                fragment.push(char::from(HEX[usize::from(byte & 15)]));
            }
        }
        base.set_fragment(Some(&fragment));
        base.to_string()
    });
    let reason = match &live.status {
        Status::Failed { reason } => Some(reason.as_str()),
        Status::Live if link.is_none() => Some("The server has not supplied a read-only invitation. Share online to create one."),
        _ => None,
    };
    Ok(
        json!({"session": (live.id != 0).then_some(live.id), "status": live.status.id(), "ready": link.is_some(), "link": link, "access": "view", "reason": reason}),
    )
}

#[cfg(test)]
mod tests;
