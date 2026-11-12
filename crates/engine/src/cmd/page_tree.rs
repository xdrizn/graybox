//! Graybox: nested pages (G9). Pages can sit inside other pages in the Pages list; each keeps its own
//! canvas. The list order (`doc.pages`) is the tree walked top to bottom, so the next/previous page
//! shortcuts, the page menus and the exports follow it. The data lives in `figcraft_doc::page_tree`.

use figcraft_doc::{Document, NodeId};
use serde_json::{Value, json};

use super::{CommandSpec, ok};
use crate::params::{boolean, opt_id, opt_index};
use crate::{Result, Session, bad, spec};

/// The page a command targets: the explicit `id` (which must be a page) or the open page.
fn target(s: &Session, cmd: &str, p: &Value) -> Result<NodeId> {
    let page = match opt_id(cmd, p, "id")? {
        Some(i) => i,
        None => s.page()?,
    };
    if !s.doc()?.pages.contains(&page) {
        return Err(bad(cmd, format!("{page} is not a page")));
    }
    Ok(page)
}

/// `page.add` with a `parent`: the new page `added` goes inside it (and that page opens).
pub(super) fn nest_added(doc: &mut Document, added: NodeId, p: &Value) -> Result<()> {
    let Some(parent) = opt_id("page.add", p, "parent")? else { return Ok(()) };
    doc.nest_page(added, Some(parent), None).map_err(|e| bad("page.add", e.to_string()))?;
    doc.set_page_collapsed(parent, false);
    Ok(())
}

/// `page.select`, `view.nextPage` and `page.duplicate` open `page`: the pages above it expand, so it
/// shows in the list.
pub(super) fn reveal(s: &mut Session, page: NodeId) -> Result<()> {
    if s.doc()?.page_hidden(page) {
        s.doc_mut()?.reveal_page(page);
    }
    Ok(())
}

/// `page.delete` of `page`: the pages inside it go with it (`subpages`) or move up a level.
pub(super) fn before_delete(doc: &mut Document, page: NodeId, with_subpages: bool) -> Result<()> {
    if !with_subpages {
        doc.lift_subpages(page);
        return Ok(());
    }
    let inside: Vec<NodeId> = doc.page_subtree(page).into_iter().filter(|p| *p != page).collect();
    for p in inside.into_iter().rev() {
        doc.remove(p)?;
    }
    Ok(())
}

/// `page.duplicate`: the copy sits in the page its source is in, and shows.
pub(super) fn place_copy(doc: &mut Document, src: NodeId, copy: NodeId) {
    doc.nest_like(copy, src);
    doc.reveal_page(copy);
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "page.nest",
            "Put page inside another page",
            "{id? (default: the open page), parent? (a page; omit or null for the top level), index? (place among that page's subpages)}",
            true,
            |s, p| {
                let page = target(s, "page.nest", p)?;
                let parent = opt_id("page.nest", p, "parent")?;
                let index = opt_index("page.nest", p, "index")?;
                let doc = s.doc_mut()?;
                doc.nest_page(page, parent, index).map_err(|e| bad("page.nest", e.to_string()))?;
                // The page it went into opens, so the move can be seen.
                if let Some(parent) = parent {
                    doc.set_page_collapsed(parent, false);
                }
                ok()
            }
        ),
        spec!(
            "page.unnest",
            "Move page out of its parent",
            "{id? (default: the open page)} (it lands right after the page it was in)",
            true,
            |s, p| {
                let page = target(s, "page.unnest", p)?;
                let doc = s.doc_mut()?;
                let Some(parent) = doc.page_parent(page) else { return Err(bad("page.unnest", "that page is not inside another page")) };
                let up = doc.page_parent(parent);
                let at = doc.page_children(up).iter().position(|c| *c == parent).map(|i| i + 1);
                doc.nest_page(page, up, at).map_err(|e| bad("page.unnest", e.to_string()))?;
                ok()
            }
        ),
        spec!(
            "page.collapse",
            "Collapse or expand page",
            "{id? (default: the open page), collapsed? (default true; false expands)} (hides or shows the pages inside it in the list)",
            false,
            |s, p| {
                let page = target(s, "page.collapse", p)?;
                let collapsed = boolean(p, "collapsed").unwrap_or(true);
                if s.doc()?.page_tree.collapsed.contains(&page) != collapsed {
                    s.doc_mut()?.set_page_collapsed(page, collapsed);
                }
                ok()
            }
        ),
        spec!("page.reveal", "Show page in the list", "{id? (default: the open page)} (expands the pages above it)", false, |s, p| {
            let page = target(s, "page.reveal", p)?;
            reveal(s, page)?;
            ok()
        }),
        spec!("page.tree", "List pages as a tree", "{} → [{id, name, parent, depth, collapsed, divider}] in list order", false, |s, _| {
            let doc = s.doc()?;
            Ok(Value::Array(
                doc.pages
                    .iter()
                    .filter_map(|pid| doc.node(*pid))
                    .map(|n| {
                        json!({"id": n.id, "name": n.name, "parent": doc.page_parent(n.id), "depth": doc.page_depth(n.id),
                               "collapsed": doc.page_tree.collapsed.contains(&n.id), "divider": doc.is_divider(n.id)})
                    })
                    .collect(),
            ))
        }),
    ]
}
