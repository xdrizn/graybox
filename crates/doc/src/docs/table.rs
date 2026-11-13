//! Editing a table block (G13.2): write in a cell, add or remove a row or a column, make the first
//! row a header. Each is one change, so one undo takes it back.

use super::{Block, BlockId, BlockKind, DocError, DocId, Documents, MAX_TABLE, Run, Table};

/// A change to the shape of a table. Places count from 0 and are clamped when inserting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableOp {
    InsertRow(usize),
    InsertColumn(usize),
    DeleteRow(usize),
    DeleteColumn(usize),
    /// Whether the first row is a header.
    Header(bool),
}

impl TableOp {
    /// The change called `name` (`insertRow`, `insertColumn`, `deleteRow`, `deleteColumn`, `header`).
    pub fn from_name(name: &str, at: usize, on: bool) -> Option<Self> {
        Some(match name {
            "insertRow" => Self::InsertRow(at),
            "insertColumn" => Self::InsertColumn(at),
            "deleteRow" => Self::DeleteRow(at),
            "deleteColumn" => Self::DeleteColumn(at),
            "header" => Self::Header(on),
            _ => return None,
        })
    }
}

impl Table {
    /// An empty table of `rows` by `cols` cells.
    pub fn blank(rows: usize, cols: usize) -> Self {
        Self { rows: vec![vec![Vec::new(); cols.min(MAX_TABLE)]; rows.min(MAX_TABLE)], header: false }
    }

    /// The number of columns: the longest row.
    pub fn width(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// The words of cell (`row`, `col`), without looks.
    pub fn cell_text(&self, row: usize, col: usize) -> String {
        self.rows.get(row).and_then(|r| r.get(col)).map(|c| c.iter().map(|r| r.text.as_str()).collect()).unwrap_or_default()
    }

    /// Every row as long as the longest.
    fn square(&mut self) {
        let w = self.width();
        for r in &mut self.rows {
            r.resize(w, Vec::new());
        }
    }

    fn apply(&mut self, op: TableOp) -> Result<(), DocError> {
        self.square();
        let (h, w) = (self.rows.len(), self.width());
        match op {
            TableOp::InsertRow(at) => {
                if h >= MAX_TABLE {
                    return Err(DocError::Limit("table rows"));
                }
                self.rows.insert(at.min(h), vec![Vec::new(); w.max(1)]);
            }
            TableOp::InsertColumn(at) => {
                if w >= MAX_TABLE {
                    return Err(DocError::Limit("table columns"));
                }
                if self.rows.is_empty() {
                    self.rows.push(Vec::new());
                }
                for r in &mut self.rows {
                    r.insert(at.min(w), Vec::new());
                }
            }
            TableOp::DeleteRow(at) => {
                if at >= h || h <= 1 {
                    return Err(DocError::Invalid("a table keeps at least one row"));
                }
                self.rows.remove(at);
            }
            TableOp::DeleteColumn(at) => {
                if at >= w || w <= 1 {
                    return Err(DocError::Invalid("a table keeps at least one column"));
                }
                for r in &mut self.rows {
                    if at < r.len() {
                        r.remove(at);
                    }
                }
            }
            TableOp::Header(on) => self.header = on,
        }
        Ok(())
    }
}

/// The table of block `b`, or a refusal when it is not one.
fn table_of(b: &mut Block) -> Result<&mut Table, DocError> {
    match &mut b.kind {
        BlockKind::Table { table } => Ok(table),
        _ => Err(DocError::Invalid("that block is not a table")),
    }
}

impl Documents {
    /// Write `text` in cell (`row`, `col`) of table `id`. Words that stay keep their looks.
    #[allow(clippy::too_many_arguments)]
    pub fn set_cell(&mut self, doc: DocId, version: Option<u32>, id: BlockId, row: usize, col: usize, text: &str) -> Result<(), DocError> {
        self.update_block(doc, version, id, |b| {
            let table = table_of(b)?;
            table.square();
            let cell = table.rows.get_mut(row).and_then(|r| r.get_mut(col)).ok_or(DocError::Invalid("there is no such cell"))?;
            // A block of one cell's words, to edit them the way a block's words are edited.
            let mut words = Block::new(BlockId(0), BlockKind::Paragraph);
            words.text = std::mem::take(cell);
            words.set_plain_text(text);
            *cell = words.text.into_iter().filter(|r: &Run| !r.text.is_empty()).collect();
            Ok(())
        })
    }

    /// Change the shape of table `id`.
    pub fn change_table(&mut self, doc: DocId, version: Option<u32>, id: BlockId, op: TableOp) -> Result<(), DocError> {
        self.update_block(doc, version, id, |b| table_of(b)?.apply(op))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::Place;

    fn with_table(rows: usize, cols: usize) -> (Documents, DocId, BlockId) {
        let mut d = Documents::default();
        let doc = d.create("Spec", None, None).unwrap();
        let b = d.insert_block(doc, None, Place::default(), BlockKind::Table { table: Table::blank(rows, cols) }, vec![]).unwrap();
        (d, doc, b)
    }

    fn table(d: &Documents, doc: DocId, b: BlockId) -> Table {
        match &d.get(doc).unwrap().current().unwrap().block(b).unwrap().kind {
            BlockKind::Table { table } => table.clone(),
            k => panic!("not a table: {k:?}"),
        }
    }

    fn texts(t: &Table) -> Vec<Vec<String>> {
        (0..t.rows.len()).map(|r| (0..t.width()).map(|c| t.cell_text(r, c)).collect()).collect()
    }

    #[test]
    fn writing_a_cell_changes_only_that_cell_and_keeps_bold_words() {
        let (mut d, doc, b) = with_table(2, 2);
        d.set_cell(doc, None, b, 1, 0, "HP").unwrap();
        assert_eq!(texts(&table(&d, doc, b)), [["", ""], ["HP", ""]]);
        let mut t = table(&d, doc, b);
        t.rows[1][0] = vec![Run { bold: true, ..Run::plain("HP") }];
        d.update_block(doc, None, b, |blk| {
            blk.kind = BlockKind::Table { table: t };
            Ok(())
        })
        .unwrap();
        d.set_cell(doc, None, b, 1, 0, "HP bar").unwrap();
        let cell = table(&d, doc, b).rows[1][0].clone();
        assert!(cell.iter().all(|r| r.bold), "{cell:?}");
        d.set_cell(doc, None, b, 1, 0, "").unwrap();
        assert!(table(&d, doc, b).rows[1][0].is_empty());
        assert!(d.set_cell(doc, None, b, 5, 0, "x").is_err());
    }

    #[test]
    fn rows_and_columns_go_in_where_asked_and_out_but_never_the_last() {
        let (mut d, doc, b) = with_table(2, 2);
        d.set_cell(doc, None, b, 0, 0, "a").unwrap();
        d.set_cell(doc, None, b, 1, 1, "d").unwrap();
        d.change_table(doc, None, b, TableOp::InsertRow(1)).unwrap();
        d.change_table(doc, None, b, TableOp::InsertColumn(0)).unwrap();
        assert_eq!(texts(&table(&d, doc, b)), [["", "a", ""], ["", "", ""], ["", "", "d"]]);
        d.change_table(doc, None, b, TableOp::DeleteRow(1)).unwrap();
        d.change_table(doc, None, b, TableOp::DeleteColumn(0)).unwrap();
        assert_eq!(texts(&table(&d, doc, b)), [["a", ""], ["", "d"]]);
        // A huge place just means the end.
        d.change_table(doc, None, b, TableOp::InsertRow(usize::MAX)).unwrap();
        assert_eq!(table(&d, doc, b).rows.len(), 3);
        assert!(d.change_table(doc, None, b, TableOp::DeleteRow(9)).is_err());
        let (mut d, doc, b) = with_table(1, 1);
        assert!(d.change_table(doc, None, b, TableOp::DeleteRow(0)).is_err());
        assert!(d.change_table(doc, None, b, TableOp::DeleteColumn(0)).is_err());
    }

    #[test]
    fn an_empty_table_grows_from_its_first_row_or_column() {
        let (mut d, doc, b) = with_table(0, 0);
        d.change_table(doc, None, b, TableOp::InsertColumn(0)).unwrap();
        assert_eq!(texts(&table(&d, doc, b)), [[""]]);
        let (mut d, doc, b) = with_table(0, 0);
        d.change_table(doc, None, b, TableOp::InsertRow(0)).unwrap();
        assert_eq!(texts(&table(&d, doc, b)), [[""]]);
    }

    #[test]
    fn the_header_is_switched_and_tables_stop_at_their_limit() {
        let (mut d, doc, b) = with_table(1, MAX_TABLE);
        d.change_table(doc, None, b, TableOp::Header(true)).unwrap();
        assert!(table(&d, doc, b).header);
        assert!(d.change_table(doc, None, b, TableOp::InsertColumn(0)).is_err());
        let p = d.insert_block(doc, None, Place::default(), BlockKind::Paragraph, vec![]).unwrap();
        assert!(d.change_table(doc, None, p, TableOp::InsertRow(0)).is_err());
        assert!(d.set_cell(doc, None, p, 0, 0, "x").is_err());
        assert_eq!(TableOp::from_name("deleteColumn", 2, false), Some(TableOp::DeleteColumn(2)));
        assert_eq!(TableOp::from_name("nope", 0, false), None);
    }
}
