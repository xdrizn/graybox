//! Graybox: pseudo-localization preview. Every text that has letters is shown as a stand-in
//! translation: its letters accented (Settings → Šéţţîñĝš), about 40% longer (short labels more,
//! as translations of them grow most) and in brackets, so a cut or overflowing label shows at once.
//! It works on a copy of the document: the file is never changed.

use figcraft_doc::{Document, NodeId, NodeKind};

/// The stand-in for `s`: accented letters, longer, bracketed. A text without letters (a number, a
/// symbol) stays as it is.
pub fn pseudo(s: &str) -> String {
    if !s.chars().any(char::is_alphabetic) {
        return s.to_string();
    }
    let accented: String = s.chars().map(accent).collect();
    let letters = s.chars().filter(|c| !c.is_whitespace()).count();
    // Short labels grow most when translated (German, Finnish, Portuguese...).
    let grow = if letters <= 10 { 0.6 } else { 0.4 };
    let pad = ((letters as f64) * grow).ceil() as usize;
    format!("[{accented}{}]", "~".repeat(pad.max(2)))
}

fn accent(c: char) -> char {
    match c {
        'a' => 'á',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'đ',
        'e' => 'é',
        'f' => 'ƒ',
        'g' => 'ĝ',
        'h' => 'ĥ',
        'i' => 'î',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ľ',
        'n' => 'ñ',
        'o' => 'ö',
        'r' => 'ŕ',
        's' => 'š',
        't' => 'ţ',
        'u' => 'û',
        'w' => 'ŵ',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Å',
        'C' => 'Ç',
        'D' => 'Đ',
        'E' => 'É',
        'G' => 'Ĝ',
        'I' => 'Î',
        'L' => 'Ľ',
        'N' => 'Ñ',
        'O' => 'Ö',
        'R' => 'Ŕ',
        'S' => 'Š',
        'T' => 'Ţ',
        'U' => 'Û',
        'Y' => 'Ý',
        'Z' => 'Ž',
        other => other,
    }
}

/// `doc` with every text shown pseudo-localized and laid out again (auto-sizing text grows, auto
/// layout makes room), for drawing only.
pub fn preview(doc: &Document) -> Document {
    let before = doc.clone();
    let mut out = doc.clone();
    // An instance's text properties set its text layers when it is laid out: they change too.
    let instances: Vec<NodeId> = doc.nodes().filter(|n| matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).collect();
    for id in instances {
        // Only the properties its main declares as text (not instance swaps or variants).
        let text_props: Vec<String> = doc
            .node(id)
            .and_then(|n| match &n.kind {
                NodeKind::Instance { component, .. } => doc.node(*component),
                _ => None,
            })
            .map(|main| match &main.kind {
                NodeKind::Component { props, .. } => {
                    props.iter().filter(|(_, d)| matches!(d, figcraft_doc::PropDef::Text { .. })).map(|(k, _)| k.clone()).collect()
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();
        if text_props.is_empty() {
            continue;
        }
        if let Some(n) = out.node_mut(id)
            && let NodeKind::Instance { props, .. } = &mut n.kind
        {
            for (k, v) in props.iter_mut() {
                if text_props.contains(k)
                    && let Some(s) = v.as_str()
                {
                    *v = serde_json::Value::String(pseudo(s));
                }
            }
        }
        out.mark_dirty([id]);
    }
    let texts: Vec<NodeId> = doc.nodes().filter(|n| matches!(n.kind, NodeKind::Text(_))).map(|n| n.id).collect();
    for id in texts {
        let Some(n) = out.node_mut(id) else { continue };
        let NodeKind::Text(t) = &mut n.kind else { continue };
        let old = t.characters.clone();
        let new = pseudo(&old);
        if new == old {
            continue;
        }
        // Letter for letter first (each keeps its style), then the brackets and the extra length.
        let accented: String = old.chars().map(accent).collect();
        let len = t.char_count();
        t.replace_range(0, len, &accented);
        let len = t.char_count();
        if let (Some(open), Some(rest)) = (new.chars().next(), new.strip_prefix('[').and_then(|r| r.strip_prefix(accented.as_str()))) {
            t.replace_range(len, len, rest);
            t.replace_range(0, 0, &open.to_string());
        }
        out.mark_dirty([id]);
    }
    crate::normalize(&mut out, &before);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::json;

    #[test]
    fn labels_turn_accented_longer_and_bracketed() {
        assert_eq!(pseudo("Settings"), "[Šéţţîñĝš~~~~~]");
        assert_eq!(pseudo("100"), "100", "numbers stay");
        assert!(pseudo("Continue the adventure").chars().count() > "Continue the adventure".chars().count() * 13 / 10);
    }

    /// An auto-sizing label in a hugging auto layout button grows, and so does the button; the
    /// document itself is unchanged.
    #[test]
    fn the_preview_lays_out_longer_text_without_touching_the_file() {
        let mut s = Session::new();
        let button = s
            .execute(
                "frame.create",
                &json!({"width": 10, "height": 10, "props": {"layout": "horizontal", "padding": 8, "sizingH": "hug", "sizingV": "hug"}}),
            )
            .unwrap();
        let button = NodeId(button["id"].as_u64().unwrap());
        s.execute("text.create", &json!({"text": "Play", "parent": button})).unwrap();
        let doc = s.doc().unwrap().clone();
        let w0 = doc.node(button).unwrap().width;
        let shown = preview(&doc);
        assert!(shown.node(button).unwrap().width > w0 * 1.3, "{} vs {w0}", shown.node(button).unwrap().width);
        assert_eq!(s.doc().unwrap().node(button).unwrap().width, w0, "the file keeps its text");
    }
}
