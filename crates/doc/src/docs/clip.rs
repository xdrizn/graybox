//! Several blocks at once (G13.2): blocks selected together are deleted in one step, and copied
//! blocks are pasted as fresh copies, with what is inside them, in one step.

use super::edit::renumber;
use super::{
    Block, BlockId, DocError, DocId, Documents, MAX_BLOCK_DEPTH, MAX_BLOCKS, Place, Run, check_text, count, depth_of, find, height_of, list_mut, take,
};

/// The most blocks one paste takes.
pub const MAX_PASTE: usize = 2_000;

/// `b` made safe to put in a document: its data checked, its words cleaned. Refused when anything
/// in it is out of range.
fn checked(mut b: Block) -> Result<Block, DocError> {
    if !b.kind.valid() || !b.style.valid() {
        return Err(DocError::Invalid("that block's data is out of range"));
    }
    b.kind.sanitize();
    check_text(&b)?;
    b.text.iter_mut().for_each(Run::sanitize);
    b.children = b.children.into_iter().map(checked).collect::<Result<_, _>>()?;
    Ok(b)
}

impl Documents {
    /// Copies of `blocks` (fresh ids, with what is inside them) at `place`, in order; returns their
    /// ids. Refused as a whole when one block is out of range or the document would get too many
    /// blocks or levels.
    pub fn paste_blocks(&mut self, doc: DocId, version: Option<u32>, place: Place, blocks: Vec<Block>) -> Result<Vec<BlockId>, DocError> {
        if blocks.is_empty() {
            return Err(DocError::Invalid("nothing to paste"));
        }
        if count(&blocks) > MAX_PASTE {
            return Err(DocError::Limit("blocks"));
        }
        let blocks = blocks.into_iter().map(checked).collect::<Result<Vec<_>, _>>()?;
        let mut next = self.next_block;
        let v = self.draft_mut(doc, version)?;
        if v.block_count() + count(&blocks) > MAX_BLOCKS {
            return Err(DocError::Limit("blocks"));
        }
        let depth = match place.parent {
            Some(p) => depth_of(&v.blocks, p).ok_or(DocError::NoBlock(p))? + 1,
            None => 1,
        };
        if blocks.iter().any(|b| depth + height_of(b) - 1 > MAX_BLOCK_DEPTH) {
            return Err(DocError::Limit("levels of blocks"));
        }
        let copies: Vec<Block> = blocks.iter().map(|b| renumber(b, &mut next)).collect();
        let ids = copies.iter().map(|b| b.id).collect();
        let list = list_mut(&mut v.blocks, place.parent).ok_or(DocError::Invalid("no such parent block"))?;
        let at = place.index.map_or(list.len(), |i| i.min(list.len()));
        for (k, b) in copies.into_iter().enumerate() {
            list.insert(at + k, b);
        }
        self.next_block = next;
        Ok(ids)
    }

    /// Delete blocks `ids` (with what is inside them) in one step. Refused as a whole when one is
    /// missing.
    pub fn delete_blocks(&mut self, doc: DocId, version: Option<u32>, ids: &[BlockId]) -> Result<(), DocError> {
        let v = self.draft_mut(doc, version)?;
        let mut blocks = v.blocks.clone();
        for id in ids {
            // A block inside one deleted before is gone with it.
            if find(&v.blocks, *id).is_none() {
                return Err(DocError::NoBlock(*id));
            }
            let _ = take(&mut blocks, *id);
        }
        v.blocks = blocks;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::BlockKind;

    fn doc_with(texts: &[&str]) -> (Documents, DocId, Vec<BlockId>) {
        let mut d = Documents::default();
        let doc = d.create("Notes", None, None).unwrap();
        let ids = texts.iter().map(|t| d.insert_block(doc, None, Place::default(), BlockKind::Paragraph, vec![Run::plain(*t)]).unwrap()).collect();
        (d, doc, ids)
    }

    fn texts(d: &Documents, doc: DocId) -> Vec<String> {
        d.get(doc).unwrap().current().unwrap().blocks.iter().map(Block::plain_text).collect()
    }

    #[test]
    fn pasted_blocks_are_fresh_copies_in_order_where_asked() {
        let (mut d, doc, ids) = doc_with(&["a", "b"]);
        let copied: Vec<Block> = d.get(doc).unwrap().current().unwrap().blocks.clone();
        let new = d.paste_blocks(doc, None, Place { parent: None, index: Some(1) }, copied).unwrap();
        assert_eq!(texts(&d, doc), ["a", "a", "b", "b"]);
        assert_eq!(new.len(), 2);
        assert!(new.iter().all(|n| !ids.contains(n)), "fresh ids");
    }

    #[test]
    fn blocks_selected_together_go_in_one_step() {
        let (mut d, doc, ids) = doc_with(&["a", "b", "c"]);
        d.delete_blocks(doc, None, &[ids[0], ids[2]]).unwrap();
        assert_eq!(texts(&d, doc), ["b"]);
        assert!(d.delete_blocks(doc, None, &[ids[1], BlockId(999)]).is_err());
        assert_eq!(texts(&d, doc), ["b"], "refused as a whole");
    }

    #[test]
    fn a_paste_out_of_range_is_refused_as_a_whole() {
        let (mut d, doc, _) = doc_with(&["a"]);
        let bad = Block::new(BlockId(5), BlockKind::Swatch { color: "red".into() });
        let good = Block::new(BlockId(6), BlockKind::Paragraph);
        assert!(d.paste_blocks(doc, None, Place::default(), vec![good, bad]).is_err());
        assert_eq!(texts(&d, doc), ["a"]);
    }
}
