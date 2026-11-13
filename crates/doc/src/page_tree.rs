//! Graybox: nested pages (G9). Pages can sit inside other pages in the Pages list; every page keeps
//! its own canvas, nesting only organises the list.
//!
//! Upstream's page model is untouched: [`Document::pages`] stays a flat list. Here that list is
//! always the tree walked top to bottom (a page, then the pages inside it, then the next page), so
//! the next/previous page shortcuts, the page menus and the exports, which all follow `pages`, follow
//! the tree too. What is added is Graybox-owned data saved under `page_tree` in the file: each nested
//! page's parent, and which pages are collapsed in the list. The key is only written when something is
//! nested or collapsed, so older files open flat and upstream FigCraft ignores it.
//!
//! Files are untrusted: [`Document::settle_pages`] (run when a file is loaded and after every page
//! edit) drops a parent that is not a page, cuts cycles and puts `pages` back in tree order.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Document, NodeId};

/// Deepest a page can be nested (a top-level page is 0).
pub const MAX_PAGE_DEPTH: usize = 32;
/// Longest page icon, in characters (one emoji can be several, joined).
pub const MAX_PAGE_ICON: usize = 16;

/// The nesting of the pages list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageTree {
    /// Each nested page and the page it sits in. A page that is not here is at the top level.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parents: BTreeMap<NodeId, NodeId>,
    /// Pages whose subpages are hidden in the list.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed: BTreeSet<NodeId>,
    /// The emoji or symbol shown in place of a page's icon in the list (the name stays as it is).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub icons: BTreeMap<NodeId, String>,
}

/// An icon as it is kept: no control characters, trimmed, at most [`MAX_PAGE_ICON`] characters.
fn clean_icon(icon: &str) -> String {
    icon.chars().filter(|c| !c.is_control()).take(MAX_PAGE_ICON).collect::<String>().trim().to_string()
}

impl PageTree {
    /// Whether nothing is nested or collapsed (the file then has no `page_tree` key).
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty() && self.collapsed.is_empty() && self.icons.is_empty()
    }
}

/// Why a page could not be nested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NestError {
    NotAPage(NodeId),
    /// A page cannot go inside itself or inside one of its own subpages.
    Cycle,
    /// A divider is a line in the list; nothing goes inside it.
    Divider,
    TooDeep,
}

impl fmt::Display for NestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAPage(id) => write!(f, "{id} is not a page"),
            Self::Cycle => write!(f, "a page cannot go inside itself or inside one of its own subpages"),
            Self::Divider => write!(f, "nothing can go inside a divider"),
            Self::TooDeep => write!(f, "pages can be nested at most {MAX_PAGE_DEPTH} levels deep"),
        }
    }
}

impl std::error::Error for NestError {}

impl Document {
    /// The page `id` sits in (`None`: top level, or not a page).
    pub fn page_parent(&self, id: NodeId) -> Option<NodeId> {
        self.page_tree.parents.get(&id).copied().filter(|p| *p != id && self.pages.contains(p))
    }

    /// The pages directly inside `parent` (`None`: the top level), in list order.
    pub fn page_children(&self, parent: Option<NodeId>) -> Vec<NodeId> {
        self.pages.iter().copied().filter(|p| self.page_parent(*p) == parent).collect()
    }

    /// Whether page `id` has pages inside it.
    pub fn has_subpages(&self, id: NodeId) -> bool {
        self.pages.iter().any(|p| self.page_parent(*p) == Some(id))
    }

    /// How many pages `id` sits inside (0: top level).
    pub fn page_depth(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut cur = id;
        while let Some(p) = self.page_parent(cur) {
            depth += 1;
            cur = p;
            if depth > self.pages.len() {
                break;
            }
        }
        depth
    }

    /// Whether page `id` sits somewhere inside page `ancestor` (not when they are the same page).
    pub fn page_is_inside(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut cur = id;
        for _ in 0..=self.pages.len() {
            match self.page_parent(cur) {
                Some(p) if p == ancestor => return true,
                Some(p) => cur = p,
                None => return false,
            }
        }
        false
    }

    /// `id` and every page inside it, in list order.
    pub fn page_subtree(&self, id: NodeId) -> Vec<NodeId> {
        self.pages.iter().copied().filter(|p| *p == id || self.page_is_inside(*p, id)).collect()
    }

    /// Every page above `id`, nearest first.
    pub fn page_ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = id;
        while let Some(p) = self.page_parent(cur) {
            out.push(p);
            cur = p;
            if out.len() > self.pages.len() {
                break;
            }
        }
        out
    }

    /// Whether a collapsed page hides `id` in the list.
    pub fn page_hidden(&self, id: NodeId) -> bool {
        self.page_ancestors(id).iter().any(|a| self.page_tree.collapsed.contains(a))
    }

    /// Collapse or expand `id`'s subpages. True when that changed anything.
    pub fn set_page_collapsed(&mut self, id: NodeId, collapsed: bool) -> bool {
        if !self.pages.contains(&id) {
            return false;
        }
        if collapsed { self.page_tree.collapsed.insert(id) } else { self.page_tree.collapsed.remove(&id) }
    }

    /// The emoji or symbol page `id` wears in the list, if any.
    pub fn page_icon(&self, id: NodeId) -> Option<&str> {
        self.page_tree.icons.get(&id).map(String::as_str).filter(|s| !s.is_empty())
    }

    /// Give page `id` an icon (blank text removes it). True when that changed anything.
    pub fn set_page_icon(&mut self, id: NodeId, icon: &str) -> bool {
        if !self.pages.contains(&id) {
            return false;
        }
        let icon = clean_icon(icon);
        if icon.is_empty() {
            return self.page_tree.icons.remove(&id).is_some();
        }
        self.page_tree.icons.insert(id, icon.clone()) != Some(icon)
    }

    /// Expand every page above `id`, so that `id` shows in the list. True when that changed anything.
    pub fn reveal_page(&mut self, id: NodeId) -> bool {
        if self.page_tree.collapsed.is_empty() {
            return false;
        }
        let mut changed = false;
        for a in self.page_ancestors(id) {
            changed |= self.page_tree.collapsed.remove(&a);
        }
        changed
    }

    /// Put page `id` (with the pages inside it) inside `parent` (`None`: the top level) at `index`
    /// among that page's subpages as they are once `id` is lifted out (`None`: last).
    pub fn nest_page(&mut self, id: NodeId, parent: Option<NodeId>, index: Option<usize>) -> Result<(), NestError> {
        if !self.pages.contains(&id) {
            return Err(NestError::NotAPage(id));
        }
        if let Some(p) = parent {
            if !self.pages.contains(&p) {
                return Err(NestError::NotAPage(p));
            }
            if p == id || self.page_is_inside(p, id) {
                return Err(NestError::Cycle);
            }
            if self.is_divider(p) {
                return Err(NestError::Divider);
            }
        }
        // The deepest page of the block keeps the whole thing within the limit.
        let block = self.page_subtree(id);
        let below = block.iter().map(|b| self.page_depth(*b).saturating_sub(self.page_depth(id))).max().unwrap_or(0);
        let base = parent.map_or(0, |p| self.page_depth(p) + 1);
        if base + below > MAX_PAGE_DEPTH {
            return Err(NestError::TooDeep);
        }
        let kept: Vec<NodeId> = self.pages.iter().copied().filter(|p| !block.contains(p)).collect();
        let siblings: Vec<NodeId> = kept.iter().copied().filter(|p| self.page_parent(*p) == parent).collect();
        // Where the block goes in `kept`: before the sibling now at `index`, else after the last page
        // of the parent's block (the end of the list for the top level).
        let at = match index.and_then(|i| siblings.get(i)) {
            Some(before) => kept.iter().position(|p| p == before).unwrap_or(kept.len()),
            None => match parent {
                Some(p) => {
                    let last = kept.iter().rposition(|k| *k == p || self.page_is_inside(*k, p)).unwrap_or(kept.len().saturating_sub(1));
                    last + 1
                }
                None => kept.len(),
            },
        };
        let mut pages = kept;
        let at = at.min(pages.len());
        pages.splice(at..at, block);
        self.pages = pages;
        match parent {
            Some(p) => {
                self.page_tree.parents.insert(id, p);
            }
            None => {
                self.page_tree.parents.remove(&id);
            }
        }
        self.settle_pages();
        Ok(())
    }

    /// Before page `id` is deleted: the pages inside it move up to the page it sits in, keeping their
    /// place in the list.
    pub fn lift_subpages(&mut self, id: NodeId) {
        let up = self.page_parent(id);
        let kids = self.page_children(Some(id));
        for k in kids {
            match up {
                Some(p) => self.page_tree.parents.insert(k, p),
                None => self.page_tree.parents.remove(&k),
            };
        }
        self.page_tree.parents.remove(&id);
        self.page_tree.collapsed.remove(&id);
    }

    /// A copy `copy` of page `src` sits next to it, in the same page.
    pub fn nest_like(&mut self, copy: NodeId, src: NodeId) {
        match self.page_parent(src) {
            Some(p) => self.page_tree.parents.insert(copy, p),
            None => self.page_tree.parents.remove(&copy),
        };
        self.settle_pages();
    }

    /// Whether [`settle_pages`](Self::settle_pages) would change nothing.
    pub fn page_tree_is_settled(&self) -> bool {
        if self.page_tree.is_empty() {
            return true;
        }
        let (tree, order) = self.settled_pages();
        tree == self.page_tree && order == self.pages
    }

    /// Make the nesting valid and `pages` the tree walked top to bottom: forget parents that are not
    /// pages, cut cycles, and keep each page after its parent and the pages of one parent together,
    /// in the order `pages` has them. Nothing is touched when it already is.
    pub fn settle_pages(&mut self) {
        if self.page_tree.is_empty() {
            return;
        }
        let (tree, order) = self.settled_pages();
        self.page_tree = tree;
        self.pages = order;
    }

    /// The nesting and page order `settle_pages` would leave, computed without touching the document
    /// (only the small tree is copied, never the nodes).
    fn settled_pages(&self) -> (PageTree, Vec<NodeId>) {
        let mut tree = self.page_tree.clone();
        let listed: BTreeSet<NodeId> = self.pages.iter().copied().collect();
        tree.parents.retain(|c, p| c != p && listed.contains(c) && listed.contains(p));
        tree.collapsed.retain(|p| listed.contains(p));
        tree.icons.retain(|p, _| listed.contains(p));
        for icon in tree.icons.values_mut() {
            *icon = clean_icon(icon);
        }
        tree.icons.retain(|_, icon| !icon.is_empty());
        // A parent chain that comes back to its start is a cycle: the page that starts it goes to the top.
        for id in self.pages.iter().copied() {
            let mut cur = id;
            for _ in 0..=self.pages.len() {
                match tree.parents.get(&cur).copied() {
                    Some(p) if p == id => {
                        tree.parents.remove(&id);
                        break;
                    }
                    Some(p) => cur = p,
                    None => break,
                }
            }
        }
        let mut kids: BTreeMap<Option<NodeId>, Vec<NodeId>> = BTreeMap::new();
        for p in &self.pages {
            kids.entry(tree.parents.get(p).copied()).or_default().push(*p);
        }
        let mut order = Vec::with_capacity(self.pages.len());
        let mut stack: Vec<NodeId> = kids.get(&None).map(|v| v.iter().rev().copied().collect()).unwrap_or_default();
        while let Some(p) = stack.pop() {
            order.push(p);
            if let Some(v) = kids.get(&Some(p)) {
                stack.extend(v.iter().rev().copied());
            }
        }
        // A list the walk cannot cover (a page listed twice) keeps its order.
        if order.len() != self.pages.len() {
            order = self.pages.clone();
        }
        (tree, order)
    }
}
