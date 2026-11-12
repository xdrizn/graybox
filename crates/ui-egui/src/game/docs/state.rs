//! What the Documents tab and the document editor remember between frames, per file (by uid).
//! It lives here, not in `FigApp`, so upstream state is untouched. It is never saved.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use figcraft_doc::docs::{BlockId, DocId};

use crate::FigApp;

/// What a pending edit is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Title,
    Block(BlockId),
}

/// Text typed but not yet sent as a command: it is sent when the field loses focus, after a short
/// pause, and before any other command runs, so one burst of typing is one undo step.
#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub doc: DocId,
    pub target: Target,
    pub text: String,
    /// When the text last changed (egui time, seconds).
    pub at: f64,
}

#[derive(Default)]
pub(super) struct State {
    /// The document shown in place of the canvas (while the Documents tab is the left tab).
    pub open: Option<DocId>,
    /// The tree row being renamed, and whether its field takes the keyboard this frame.
    pub renaming: Option<DocId>,
    pub rename_start: bool,
    /// Documents shown open in the tree (children listed).
    pub expanded: BTreeSet<DocId>,
    /// The document a delete waits for the person to confirm.
    pub confirm_delete: Option<DocId>,
    /// The tree row being dragged.
    pub drag: Option<DocId>,
    pub pending: Option<Pending>,
    /// A field that takes the keyboard next frame.
    pub focus: Option<Target>,
}

thread_local! {
    static STATES: RefCell<BTreeMap<u64, State>> = const { RefCell::new(BTreeMap::new()) };
}

/// The id of the file in front.
pub(super) fn uid(app: &FigApp) -> Option<u64> {
    app.session.ds().ok().map(|ds| ds.uid)
}

/// Take the file's state out (a default one when none): put it back with [`put`]. Taking, not
/// borrowing, so a command run in between can come back here without a conflict.
pub(super) fn take(uid: u64) -> State {
    STATES.with(|s| s.try_borrow_mut().ok().and_then(|mut m| m.remove(&uid))).unwrap_or_default()
}

/// Forget the files that are no longer open (`open` holds the uids of the open ones).
pub(super) fn prune(open: &[u64]) {
    STATES.with(|s| {
        if let Ok(mut m) = s.try_borrow_mut() {
            m.retain(|uid, _| open.contains(uid));
        }
    });
}

pub(super) fn put(uid: u64, state: State) {
    STATES.with(|s| {
        if let Ok(mut m) = s.try_borrow_mut() {
            m.insert(uid, state);
        }
    });
}
