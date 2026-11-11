//! The control channel the chat needs. The app opens it only when the first message is sent (unless
//! it was started with `--control`): `apps/figcraft` watches [`take_wish`] and opens a localhost
//! port, then tells us with [`set_port`]. Desktop only.

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

static PORT: AtomicU16 = AtomicU16::new(0);
static WISH: AtomicBool = AtomicBool::new(false);

/// The port of the control channel, when there is one.
pub fn port() -> Option<u16> {
    match PORT.load(Ordering::Relaxed) {
        0 => None,
        p => Some(p),
    }
}

/// The app has a control channel on `port`.
pub fn set_port(port: u16) {
    PORT.store(port, Ordering::Relaxed);
}

/// The chat wants a control channel (no-op when there is one).
pub fn ask() {
    if port().is_none() {
        WISH.store(true, Ordering::Relaxed);
    }
}

/// Whether the chat asked for a control channel since the last call (the app opens it then).
pub fn take_wish() -> bool {
    WISH.swap(false, Ordering::Relaxed) && port().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asking_is_taken_once_and_not_when_a_port_exists() {
        set_port(0);
        let _ = take_wish();
        ask();
        assert!(take_wish());
        assert!(!take_wish());
        set_port(4242);
        assert_eq!(port(), Some(4242));
        ask();
        assert!(!take_wish());
        set_port(0);
    }
}
