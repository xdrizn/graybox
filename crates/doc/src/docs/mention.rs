//! Mentions in documents (G13.2): `@` and a pick put a layer or another document into the words, as
//! one run that names it. Changing its words turns it into ordinary text.

use super::edit::{join_runs, split_runs};
use super::{BlockId, DocError, DocId, Documents, Mention, Run};

/// The longest name a mention shows, in characters.
const MAX_NAME: usize = 120;

/// `runs` with characters `from..to` replaced by a mention of `what` reading `@name`, then a space
/// unless one follows already (so typing after it is ordinary text).
pub fn mention_runs(runs: &[Run], from: usize, to: usize, name: &str, what: Mention) -> Vec<Run> {
    let (head, rest) = split_runs(runs, from);
    let (_, tail) = split_runs(&rest, to.saturating_sub(from));
    let name: String = name.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    let pill = Run { text: format!("@{}", name.trim()), mention: Some(what), ..Run::default() };
    let spaced = tail.first().and_then(|r| r.text.chars().next()).is_some_and(char::is_whitespace);
    let mut mid = vec![pill];
    if !spaced {
        mid.push(Run::plain(" "));
    }
    join_runs(join_runs(head, mid), tail).into_iter().filter(|r| !r.text.is_empty()).collect()
}

impl Documents {
    /// Replace characters `from..to` of block `id` with a mention (see [`mention_runs`]). Refused for a
    /// block without words.
    #[allow(clippy::too_many_arguments)]
    pub fn mention_text(
        &mut self,
        doc: DocId,
        version: Option<u32>,
        id: BlockId,
        from: usize,
        to: usize,
        name: &str,
        what: Mention,
    ) -> Result<(), DocError> {
        self.update_block(doc, version, id, |b| {
            if !b.kind.has_text() {
                return Err(DocError::Invalid("only a block with words can mention something"));
            }
            let len = b.text_len();
            let (from, to) = (from.min(len), to.min(len));
            if from > to {
                return Err(DocError::Invalid("`from` comes after `to`"));
            }
            b.text = mention_runs(&b.text, from, to, name, what);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeId;

    fn texts(runs: &[Run]) -> Vec<(&str, bool)> {
        runs.iter().map(|r| (r.text.as_str(), r.mention.is_some())).collect()
    }

    #[test]
    fn the_typed_at_and_letters_become_one_mention_and_a_space() {
        let runs = vec![Run::plain("see @hu")];
        let out = mention_runs(&runs, 4, 7, "HUD", Mention::Layer { node: NodeId(7) });
        assert_eq!(texts(&out), [("see ", false), ("@HUD", true), (" ", false)]);
        assert_eq!(out[1].mention, Some(Mention::Layer { node: NodeId(7) }));
    }

    #[test]
    fn a_space_already_after_it_is_kept_and_not_doubled() {
        let runs = vec![Run::plain("@x and more")];
        let out = mention_runs(&runs, 0, 2, "Brief", Mention::Document { doc: DocId(3) });
        assert_eq!(texts(&out), [("@Brief", true), (" and more", false)]);
    }

    #[test]
    fn a_long_name_is_cut_and_its_line_breaks_go() {
        let out = mention_runs(&[], 0, 0, &format!("a\nb{}", "z".repeat(500)), Mention::Document { doc: DocId(1) });
        assert!(out[0].text.chars().count() <= MAX_NAME + 1);
        assert!(!out[0].text.contains('\n'));
    }
}
