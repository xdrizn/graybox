//! Editing blocks the way the keyboard does (G13.2): Enter splits a block at the cursor, Backspace
//! at the start of a block joins it to the one above, Duplicate copies a block with what is inside it.
//! Each is one change, so one undo takes it back.

use super::{
    Block, BlockId, BlockKind, DocError, DocId, Documents, MAX_BLOCK_DEPTH, MAX_BLOCKS, Run, check_text, count, depth_of, find, find_mut, list_mut,
};

/// What a split made: the new block, and which block the keyboard goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split {
    pub new: BlockId,
    pub focus: BlockId,
}

/// Where a merge put the words: into which block, and where the cursor goes (the joint, in characters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Merge {
    pub into: BlockId,
    pub at: usize,
}

/// `runs` cut at character `at`: what comes before and what comes after, each keeping its look.
pub fn split_runs(runs: &[Run], at: usize) -> (Vec<Run>, Vec<Run>) {
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    let mut pos = 0;
    for r in runs {
        let len = r.text.chars().count();
        if pos + len <= at {
            head.push(r.clone());
        } else if pos >= at {
            tail.push(r.clone());
        } else {
            let cut = at - pos;
            let a: String = r.text.chars().take(cut).collect();
            let b: String = r.text.chars().skip(cut).collect();
            // A mention cut in two is ordinary text on both sides.
            head.push(Run { text: a, mention: None, ..r.clone() });
            tail.push(Run { text: b, mention: None, ..r.clone() });
        }
        pos += len;
    }
    (head, tail)
}

/// Runs one after the other, with neighbours that look the same joined into one.
pub(super) fn join_runs(mut a: Vec<Run>, b: Vec<Run>) -> Vec<Run> {
    for r in b {
        match a.last_mut() {
            Some(last)
                if last.mention.is_none()
                    && r.mention.is_none()
                    && Run { text: String::new(), ..last.clone() } == Run { text: String::new(), ..r.clone() } =>
            {
                last.text.push_str(&r.text);
            }
            _ => a.push(r),
        }
    }
    a
}

/// The kind of the block Enter makes after a block of `kind`: lists go on, a task starts unchecked,
/// everything else is followed by text.
fn next_kind(kind: &BlockKind) -> BlockKind {
    match kind {
        BlockKind::Bulleted => BlockKind::Bulleted,
        BlockKind::Numbered => BlockKind::Numbered,
        BlockKind::Checklist { .. } => BlockKind::Checklist { checked: false },
        _ => BlockKind::Paragraph,
    }
}

/// The block shown just above `id` when every block is laid out in reading order: the last block
/// inside the previous sibling (at any depth), or the parent when `id` is the first inside it.
fn above(blocks: &[Block], id: BlockId) -> Option<BlockId> {
    fn last_inside(b: &Block) -> BlockId {
        b.children.last().map_or(b.id, last_inside)
    }
    fn walk(blocks: &[Block], parent: Option<BlockId>, id: BlockId) -> Option<Option<BlockId>> {
        for (i, b) in blocks.iter().enumerate() {
            if b.id == id {
                return Some(match i.checked_sub(1).and_then(|p| blocks.get(p)) {
                    Some(prev) => Some(last_inside(prev)),
                    None => parent,
                });
            }
            if let Some(found) = walk(&b.children, Some(b.id), id) {
                return Some(found);
            }
        }
        None
    }
    walk(blocks, None, id).flatten()
}

/// A copy of `b` and everything inside it, each with a fresh id from `next`.
fn renumber(b: &Block, next: &mut u64) -> Block {
    *next = (*next).max(1);
    let id = BlockId(*next);
    *next = next.saturating_add(1);
    Block { id, children: b.children.iter().map(|c| renumber(c, next)).collect(), ..b.clone() }
}

impl Documents {
    /// Enter at character `at` of block `id`: the words before stay, the words after go to a new
    /// block right after it (inside it, first, when it is an open toggle). At the very start of a
    /// block with words, the new block is an empty one above and the keyboard stays.
    pub fn split_block(&mut self, doc: DocId, version: Option<u32>, id: BlockId, at: usize) -> Result<Split, DocError> {
        let new = BlockId(self.next_block.max(1));
        let v = self.draft_mut(doc, version)?;
        if v.block_count() >= MAX_BLOCKS {
            return Err(DocError::Limit("blocks"));
        }
        let (parent, index) = v.locate(id).ok_or(DocError::NoBlock(id))?;
        // A block inside it would be one level deeper than blocks may go.
        let room_inside = depth_of(&v.blocks, id).is_some_and(|d| d < MAX_BLOCK_DEPTH);
        let block = find_mut(&mut v.blocks, id).ok_or(DocError::NoBlock(id))?;
        if !block.kind.has_text() {
            return Err(DocError::Invalid("only a block with words can be split"));
        }
        let len = block.text_len();
        let at = at.min(len);
        let kind = next_kind(&block.kind);
        let split = if at == 0 && len > 0 {
            // An empty line above, plain; the block keeps its words and its kind.
            let above = Block::new(new, if matches!(block.kind, BlockKind::Heading { .. }) { BlockKind::Paragraph } else { kind });
            list_mut(&mut v.blocks, parent).ok_or(DocError::NoBlock(id))?.insert(index, above);
            Split { new, focus: id }
        } else {
            let (head, tail) = split_runs(&block.text, at);
            block.text = head;
            let mut after = Block::new(new, kind);
            // Words that move keep their colours and font; a new empty line starts plain.
            if tail.iter().any(|r| !r.text.is_empty()) {
                after.style = block.style.clone();
            }
            after.text = tail;
            if matches!(block.kind, BlockKind::Toggle { open: true }) && at == len && room_inside {
                block.children.insert(0, after);
            } else {
                list_mut(&mut v.blocks, parent).ok_or(DocError::NoBlock(id))?.insert(index + 1, after);
            }
            Split { new, focus: new }
        };
        self.next_block = new.0.saturating_add(1);
        Ok(split)
    }

    /// Backspace at the start of block `id`: its words join the end of the block shown above it, the
    /// blocks inside it take its place, and it goes. Refused at the top of the document and when the
    /// block above has no words (a divider, an image).
    pub fn merge_block(&mut self, doc: DocId, version: Option<u32>, id: BlockId) -> Result<Merge, DocError> {
        let v = self.draft_mut(doc, version)?;
        let into = above(&v.blocks, id).ok_or(DocError::Invalid("there is no block above to join"))?;
        let block = find(&v.blocks, id).ok_or(DocError::NoBlock(id))?;
        let target = find(&v.blocks, into).ok_or(DocError::NoBlock(into))?;
        if !block.kind.has_text() || !target.kind.has_text() {
            return Err(DocError::Invalid("only blocks with words can be joined"));
        }
        let at = target.text_len();
        let mut joined = target.clone();
        joined.text = join_runs(joined.text, block.text.clone());
        check_text(&joined)?;
        let (parent, index) = v.locate(id).ok_or(DocError::NoBlock(id))?;
        let list = list_mut(&mut v.blocks, parent).ok_or(DocError::NoBlock(id))?;
        if index >= list.len() {
            return Err(DocError::NoBlock(id));
        }
        let gone = list.remove(index);
        for (k, child) in gone.children.into_iter().enumerate() {
            list.insert(index + k, child);
        }
        if let Some(t) = find_mut(&mut v.blocks, into) {
            t.text = joined.text;
        }
        Ok(Merge { into, at })
    }

    /// A copy of block `id`, with what is inside it, right after it; returns the copy's id.
    pub fn duplicate_block(&mut self, doc: DocId, version: Option<u32>, id: BlockId) -> Result<BlockId, DocError> {
        let mut next = self.next_block;
        let v = self.draft_mut(doc, version)?;
        let block = find(&v.blocks, id).ok_or(DocError::NoBlock(id))?;
        if v.block_count() + count(std::slice::from_ref(block)) > MAX_BLOCKS {
            return Err(DocError::Limit("blocks"));
        }
        let copy = renumber(block, &mut next);
        let copy_id = copy.id;
        let (parent, index) = v.locate(id).ok_or(DocError::NoBlock(id))?;
        list_mut(&mut v.blocks, parent).ok_or(DocError::NoBlock(id))?.insert(index + 1, copy);
        self.next_block = next;
        Ok(copy_id)
    }
}

#[cfg(test)]
mod tests;
