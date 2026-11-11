//! Graybox: opens the control channel at run time for the Claude chat. The chat drives the open
//! file through `figcraft-cli mcp --attach <port>`, which needs the channel; when the app was not
//! started with `--control`, it is opened here on a free localhost port at the first chat message.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use figcraft_engine::events::EventLog;
use figcraft_ui_egui::ControlRequest;
use figcraft_ui_egui::game::claude::control;

/// A free port on 127.0.0.1 (found by binding port 0; the control server binds it right after).
fn free_port() -> Option<u16> {
    TcpListener::bind(("127.0.0.1", 0)).ok()?.local_addr().ok().map(|a| a.port())
}

/// Open the channel if the chat asked for it and there is none yet.
pub fn open_if_asked(queue: &Arc<Mutex<Vec<ControlRequest>>>, ctx_slot: &Arc<Mutex<Option<egui::Context>>>, events: Arc<EventLog>) {
    if !control::take_wish() {
        return;
    }
    let Some(port) = free_port() else { return };
    crate::control_server::start(port, Duration::from_secs(2), queue.clone(), ctx_slot.clone(), events);
    control::set_port(port);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chat_asking_opens_one_localhost_channel() {
        let queue = Arc::new(Mutex::new(Vec::new()));
        let slot = Arc::new(Mutex::new(None));
        let events = Arc::new(EventLog::default());
        // Nothing asked: nothing opens.
        open_if_asked(&queue, &slot, events.clone());
        assert_eq!(control::port(), None);
        control::ask();
        open_if_asked(&queue, &slot, events.clone());
        let port = control::port().expect("the channel is open");
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
        // Asking again with a channel open does nothing.
        control::ask();
        assert!(!control::take_wish());
    }
}
