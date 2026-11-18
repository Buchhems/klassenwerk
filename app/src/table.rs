//! Tables in the task text: merged cells, a frame per cell, and the editing
//! commands of the toolbar (insert/delete rows and columns, merge, split).
//!
//! Markup (one line per row, cells separated by `|`):
//! - `+| a | b |` a row with frames around every cell, `| a | b |` without
//! - a cell that is just `<` belongs to the cell on its left (merged across)
//! - a cell that is just `^` belongs to the cell above (merged down)
//! - a cell starting with `!` switches its frame: framed in a table without
//!   frames, unframed in a table with frames

// the merging works on a grid of rows and columns
#![allow(clippy::needless_range_loop)]

/// How a cell is written in the markup.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mark {
    #[default]
    None,
    /// `<`: part of the cell on the left
    Left,
    /// `^`: part of the cell above
    Up,
}

/// Layout of one cell after merging.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellInfo {
    /// columns and rows the cell covers; 0 = covered by another cell
    pub colspan: usize,
    pub rowspan: usize,
    /// frame around the cell
    pub border: bool,
    pub mark: Mark,
    /// `!` written in front of the cell
    pub toggle: bool,
}

impl Default for CellInfo {
    fn default() -> Self {
        CellInfo { colspan: 1, rowspan: 1, border: false, mark: Mark::None, toggle: false }
    }
}

impl CellInfo {
    pub fn covered(&self) -> bool {
        self.colspan == 0
    }
}

/// Reads the marks of a cell's raw text: (text without the marks, mark, frame switched).
pub fn cell_marks(raw: &str) -> (&str, Mark, bool) {
    let t = raw.trim();
    match t {
        "<" => return ("", Mark::Left, false),
        "^" => return ("", Mark::Up, false),
        _ => {}
    }
    match t.strip_prefix('!') {
        Some(r) => (r.trim_start(), Mark::None, true),
        None => (t, Mark::None, false),
    }
}

/// Fills in spans and frames: every row gets `cols` cells, merged cells are
/// combined into rectangles (the smallest one around all their parts).
pub fn layout(info: &mut [Vec<CellInfo>], bordered: bool) {
    let n = info.len();
    let cols = info.iter().map(|r| r.len()).max().unwrap_or(0);
    for r in info.iter_mut() {
        r.resize(cols, CellInfo::default());
    }
    for r in info.iter_mut() {
        for c in r.iter_mut() {
            c.colspan = 1;
            c.rowspan = 1;
        }
    }
    // owner of every cell
    let mut owner: Vec<Vec<(usize, usize)>> = (0..n).map(|r| (0..cols).map(|c| (r, c)).collect()).collect();
    for r in 0..n {
        for c in 0..cols {
            let o = match info[r][c].mark {
                Mark::Left if c > 0 => owner[r][c - 1],
                Mark::Up if r > 0 => owner[r - 1][c],
                _ => continue,
            };
            owner[r][c] = o;
        }
    }
    // spans: the rectangle around all parts of a cell
    let mut ext: Vec<Vec<(usize, usize)>> = vec![vec![(1, 1); cols]; n];
    for r in 0..n {
        for c in 0..cols {
            let (orow, ocol) = owner[r][c];
            let e = &mut ext[orow][ocol];
            e.0 = e.0.max(c - ocol + 1);
            e.1 = e.1.max(r - orow + 1);
        }
    }
    let mut covered = vec![vec![false; cols]; n];
    for r in 0..n {
        for c in 0..cols {
            if owner[r][c] != (r, c) || covered[r][c] {
                continue;
            }
            let (cs, rs) = ext[r][c];
            for rr in r..(r + rs).min(n) {
                for cc in c..(c + cs).min(cols) {
                    if (rr, cc) != (r, c) {
                        covered[rr][cc] = true;
                    }
                }
            }
            info[r][c].colspan = cs;
            info[r][c].rowspan = rs;
            info[r][c].border = bordered != info[r][c].toggle;
        }
    }
    // covered cells take the frame of the cell they belong to (for RTF/Word merges)
    for r in 0..n {
        for c in 0..cols {
            if covered[r][c] {
                let (orow, ocol) = owner[r][c];
                let b = info[orow][ocol].border;
                info[r][c].colspan = 0;
                info[r][c].rowspan = 0;
                info[r][c].border = b;
            }
        }
    }
}

/// For RTF and Word: the cell whose rectangle covers (r, c), as (row, column).
pub fn anchor_of(info: &[Vec<CellInfo>], r: usize, c: usize) -> (usize, usize) {
    for rr in (0..=r).rev() {
        for cc in (0..=c).rev() {
            let i = info[rr][cc];
            if !i.covered() && rr + i.rowspan > r && cc + i.colspan > c {
                return (rr, cc);
            }
        }
    }
    (r, c)
}

// ---------------------------------------------------------------- editing

/// A table row of the text.
struct Row {
    bordered: bool,
    cells: Vec<String>,
}

fn split(line: &str) -> Option<Row> {
    let t = line.trim_start();
    let (bordered, rest) = match t.strip_prefix("+|") {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('|')?),
    };
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for ch in rest.chars() {
        if esc {
            cur.push(ch);
            esc = false;
        } else if ch == '\\' {
            cur.push(ch);
            esc = true;
        } else if ch == '|' {
            cells.push(std::mem::take(&mut cur).trim().to_string());
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        cells.push(cur.trim().to_string());
    }
    Some(Row { bordered, cells })
}

fn join(r: &Row) -> String {
    let mut s = String::from(if r.bordered { "+|" } else { "|" });
    for c in &r.cells {
        if c.is_empty() {
            s += " |";
        } else {
            s += &format!(" {c} |");
        }
    }
    s
}

/// The table around a cursor position (char index): its lines, the row and the
/// column of the cursor.
struct At {
    /// byte range of every table line in the text
    lines: Vec<(usize, usize)>,
    rows: Vec<Row>,
    row: usize,
    col: usize,
}

fn at(text: &str, pos: usize) -> Option<At> {
    let bpos = text.char_indices().nth(pos).map(|(b, _)| b).unwrap_or(text.len());
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for l in text.split('\n') {
        spans.push((start, start + l.len()));
        start += l.len() + 1;
    }
    let li = spans.iter().position(|(a, b)| bpos >= *a && bpos <= *b)?;
    split(&text[spans[li].0..spans[li].1])?;
    let mut first = li;
    while first > 0 && split(&text[spans[first - 1].0..spans[first - 1].1]).is_some() {
        first -= 1;
    }
    let mut last = li;
    while last + 1 < spans.len() && split(&text[spans[last + 1].0..spans[last + 1].1]).is_some() {
        last += 1;
    }
    let lines: Vec<(usize, usize)> = spans[first..=last].to_vec();
    let rows: Vec<Row> = lines.iter().map(|(a, b)| split(&text[*a..*b]).unwrap()).collect();
    // column: unescaped bars before the cursor, minus the leading one
    let line = &text[spans[li].0..bpos];
    let mut bars: usize = 0;
    let mut esc = false;
    for ch in line.chars() {
        if esc {
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '|' {
            bars += 1;
        }
    }
    let ncols = rows[li - first].cells.len().max(1);
    Some(At { lines, rows, row: li - first, col: bars.saturating_sub(1).min(ncols - 1) })
}

/// Whether the cursor is in a table.
pub fn in_table(text: &str, pos: usize) -> bool {
    at(text, pos).is_some()
}

/// Editing commands for the table under the cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    RowAbove,
    RowBelow,
    ColLeft,
    ColRight,
    DeleteRow,
    DeleteCol,
    MergeRight,
    MergeDown,
    Split,
    Border,
}

/// Applies a command; returns the new text and the cursor (char index) in the
/// same cell, or None when the cursor is not in a table or nothing can be done.
pub fn apply(text: &str, pos: usize, cmd: Cmd) -> Option<(String, usize)> {
    let At { lines, mut rows, mut row, mut col } = at(text, pos)?;
    let cols = rows.iter().map(|r| r.cells.len()).max().unwrap_or(1).max(1);
    for r in rows.iter_mut() {
        r.cells.resize(cols, String::new());
    }
    let bordered = rows[row].bordered;
    let empty_row = || Row { bordered, cells: vec![String::new(); cols] };
    match cmd {
        Cmd::RowAbove => rows.insert(row, empty_row()),
        Cmd::RowBelow => {
            // below the whole merged cell
            let mut info = infos(&rows);
            layout(&mut info, bordered);
            let (ar, ac) = anchor_of(&info, row, col);
            let below = ar + info[ar][ac].rowspan;
            rows.insert(below, empty_row());
            row = below;
        }
        Cmd::ColLeft | Cmd::ColRight => {
            let at = if cmd == Cmd::ColLeft { col } else { col + 1 };
            for r in rows.iter_mut() {
                r.cells.insert(at, String::new());
            }
            col = at;
        }
        Cmd::DeleteRow => {
            if rows.len() == 1 {
                return None;
            }
            // a merged cell that started here moves its text to the next row
            if row + 1 < rows.len() {
                for c in 0..cols {
                    if rows[row + 1].cells[c] == "^" {
                        rows[row + 1].cells[c] = rows[row].cells[c].clone();
                    }
                }
            }
            rows.remove(row);
            row = row.min(rows.len() - 1);
        }
        Cmd::DeleteCol => {
            if cols == 1 {
                return None;
            }
            for r in rows.iter_mut() {
                if col + 1 < r.cells.len() && r.cells[col + 1] == "<" {
                    r.cells[col + 1] = r.cells[col].clone();
                }
                r.cells.remove(col);
            }
            col = col.min(cols - 2);
        }
        Cmd::MergeRight | Cmd::MergeDown => {
            let mut info = infos(&rows);
            layout(&mut info, bordered);
            let (ar, ac) = anchor_of(&info, row, col);
            let (cs, rs) = (info[ar][ac].colspan, info[ar][ac].rowspan);
            if cmd == Cmd::MergeRight {
                let c = ac + cs;
                if c >= cols {
                    return None;
                }
                for r in ar..ar + rs {
                    let moved = std::mem::replace(&mut rows[r].cells[c], "<".into());
                    append_text(&mut rows[ar].cells[ac], &moved);
                }
            } else {
                let r = ar + rs;
                if r >= rows.len() {
                    return None;
                }
                for c in ac..ac + cs {
                    let mark = if c == ac { "^" } else { "<" };
                    let moved = std::mem::replace(&mut rows[r].cells[c], mark.into());
                    append_text(&mut rows[ar].cells[ac], &moved);
                }
            }
            row = ar;
            col = ac;
        }
        Cmd::Split => {
            let mut info = infos(&rows);
            layout(&mut info, bordered);
            let (ar, ac) = anchor_of(&info, row, col);
            let (cs, rs) = (info[ar][ac].colspan, info[ar][ac].rowspan);
            if cs == 1 && rs == 1 {
                return None;
            }
            for r in ar..ar + rs {
                for c in ac..ac + cs {
                    if (r, c) != (ar, ac) && (rows[r].cells[c] == "<" || rows[r].cells[c] == "^") {
                        rows[r].cells[c] = String::new();
                    }
                }
            }
            row = ar;
            col = ac;
        }
        Cmd::Border => {
            let mut info = infos(&rows);
            layout(&mut info, bordered);
            let (ar, ac) = anchor_of(&info, row, col);
            let c = &mut rows[ar].cells[ac];
            *c = match c.strip_prefix('!') {
                Some(r) => r.trim_start().to_string(),
                None => format!("!{c}").trim_end().to_string(),
            };
            row = ar;
            col = ac;
        }
    }
    let new_table: Vec<String> = rows.iter().map(join).collect();
    let (start, end) = (lines[0].0, lines[lines.len() - 1].1);
    let mut out = String::with_capacity(text.len() + 16);
    out += &text[..start];
    out += &new_table.join("\n");
    out += &text[end..];
    // cursor: start of the cell's text
    let line_start: usize = text[..start].chars().count() + new_table[..row].iter().map(|l| l.chars().count() + 1).sum::<usize>();
    let line = &new_table[row];
    let mut bars = 0;
    let mut cpos = line.chars().count();
    let mut esc = false;
    for (i, ch) in line.chars().enumerate() {
        if esc {
            esc = false;
            continue;
        }
        if ch == '\\' {
            esc = true;
        } else if ch == '|' {
            bars += 1;
            if bars == col + 1 {
                cpos = i + 2;
                break;
            }
        }
    }
    Some((out, line_start + cpos.min(line.chars().count())))
}

fn infos(rows: &[Row]) -> Vec<Vec<CellInfo>> {
    rows.iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| {
                    let (_, mark, toggle) = cell_marks(c);
                    CellInfo { mark, toggle, ..Default::default() }
                })
                .collect()
        })
        .collect()
}

/// Text of a merged-away cell goes into the remaining cell.
fn append_text(into: &mut String, moved: &str) {
    let m = moved.trim();
    if m.is_empty() || m == "<" || m == "^" {
        return;
    }
    if into.trim().is_empty() {
        *into = m.to_string();
    } else {
        *into = format!("{} {m}", into.trim_end());
    }
}
