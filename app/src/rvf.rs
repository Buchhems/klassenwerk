//! Converts the rich text stored in the old task files into the app's markup.
//!
//! Markup:
//!   **fett**  *kursiv*  ++unterstrichen++  x^{2}  x_{1}  [ ] / [x] Kästchen
//!   {{n}} Bild oder Formel n   • Aufzählung   # Überschrift   --- Linie
//!   | a | b |  Tabelle ohne Rahmen,  +| a | b |  Tabelle mit Rahmen

use crate::delphi::{cp1252, find, find_bytes, read_component, DVal, Reader};
use crate::formula;
use crate::model::{Doc, DocObject, ObjKind};

#[derive(Clone, Debug, Default)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    vshift: i64,
    symbol: bool,
    wingdings: bool,
    webdings: bool,
    size: i64,
    /// text colour other than black (Delphi colour of the style)
    color: Option<[u8; 3]>,
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied();
        match c {
            '\\' | '*' | '[' | '{' | '|' => {
                out.push('\\');
                out.push(c);
            }
            '+' if next == Some('+') => out.push_str("\\+"),
            '^' | '_' if next == Some('{') => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("    "),
            _ => out.push(c),
        }
    }
    out
}

struct Lines<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Lines<'a> {
    fn line(&mut self) -> Option<&'a [u8]> {
        if self.pos >= self.d.len() {
            return None;
        }
        let rest = &self.d[self.pos..];
        match rest.iter().position(|&b| b == b'\n') {
            Some(i) => {
                self.pos += i + 1;
                let l = &rest[..i];
                Some(l.strip_suffix(b"\r").unwrap_or(l))
            }
            None => {
                self.pos = self.d.len();
                Some(rest)
            }
        }
    }
    fn binary(&mut self) -> Option<&'a [u8]> {
        let b = self.d.get(self.pos..self.pos + 4)?;
        let n = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        self.pos += 4;
        let end = (self.pos + n).min(self.d.len());
        let s = &self.d[self.pos..end];
        self.pos = end;
        Some(s)
    }
}

fn parse_styles(data: &[u8]) -> Vec<Style> {
    let mut r = Reader::new(data);
    let mut out = Vec::new();
    if let Some(DVal::Collection(items)) = r.value() {
        for props in items {
            let mut s = Style { size: 10, ..Default::default() };
            if let Some(DVal::Set(set)) = find(&props, "Style") {
                s.bold = set.iter().any(|x| x == "fsBold");
                s.italic = set.iter().any(|x| x == "fsItalic");
                s.underline = set.iter().any(|x| x == "fsUnderline");
            }
            if let Some(v) = find(&props, "VShift").and_then(|v| v.as_int()) {
                s.vshift = v;
            }
            if let Some(v) = find(&props, "Size").and_then(|v| v.as_int()) {
                s.size = v;
            }
            let font = find(&props, "FontName").and_then(|v| v.as_str()).unwrap_or("Arial").to_lowercase();
            let charset = find(&props, "Charset").and_then(|v| v.as_str()).unwrap_or("");
            s.wingdings = font.contains("wingdings");
            s.webdings = font.contains("webdings");
            s.symbol = !s.wingdings && !s.webdings && (font == "symbol" || charset == "SYMBOL_CHARSET");
            s.color = find(&props, "Color").and_then(delphi_color);
            out.push(s);
        }
    }
    out
}

/// Delphi colour (name like clBlue or a 0x00BBGGRR number) -> RGB, None for black/default.
fn delphi_color(v: &DVal) -> Option<[u8; 3]> {
    let rgb = if let Some(n) = v.as_int() {
        if n < 0 {
            return None; // system colours
        }
        [(n & 0xFF) as u8, ((n >> 8) & 0xFF) as u8, ((n >> 16) & 0xFF) as u8]
    } else {
        match v.as_str()?.to_ascii_lowercase().as_str() {
            "clblue" | "clnavy" | "clhighlight" => [0, 0, 255],
            "clred" | "clmaroon" => [210, 0, 0],
            "clgreen" | "cllime" | "clolive" | "clteal" => [0, 128, 0],
            "clpurple" | "clfuchsia" => [128, 0, 128],
            "clgray" | "clsilver" | "clgrey" => [128, 128, 128],
            "clyellow" => [230, 120, 0],
            _ => return None,
        }
    };
    if rgb == [0, 0, 0] {
        return None;
    }
    // snap to the nearest colour the task text can show
    let best = crate::markup::COLORS.iter().min_by_key(|(_, c)| (0..3).map(|i| (c[i] as i32 - rgb[i] as i32).pow(2)).sum::<i32>()).map(|(_, c)| *c)?;
    Some(best)
}

fn wingdings(b: u8) -> &'static str {
    match b {
        b'o' | b'p' | b'q' | b'r' | 0xA8 | 0x6D => "[ ]",
        b'x' | 0xFB | 0xFD | 0xFE | 0xFC => "[x]",
        b'l' => "●",
        b'n' => "■",
        b'u' | b'v' => "◆",
        b'J' => "☺",
        0xE0 | 0xE8 | 0xF0 => "→",
        0xDF | 0xE7 | 0xEF => "←",
        b' ' => " ",
        _ => "•",
    }
}

struct Conv<'o> {
    objs: &'o mut Vec<DocObject>,
    styles: Vec<Style>,
    lines: Vec<String>,
    cur: Option<String>,
}

impl<'o> Conv<'o> {
    fn new_para(&mut self) {
        if let Some(c) = self.cur.take() {
            self.lines.push(c);
        }
        self.cur = Some(String::new());
    }
    fn push(&mut self, s: &str) {
        self.cur.get_or_insert_with(String::new).push_str(s);
    }
    fn next_id(&self) -> u32 {
        self.objs.iter().map(|o| o.id).max().unwrap_or(0) + 1
    }
    fn add_obj(&mut self, kind: ObjKind, data: Vec<u8>, w: f32, h: f32) -> u32 {
        let id = self.next_id();
        self.objs.push(DocObject { id, kind, data, width: w, height: h });
        id
    }

    fn text(&mut self, style_no: i64, raw: &[u8]) {
        let st = self.styles.get(style_no as usize).cloned().unwrap_or_default();
        let s = if st.webdings {
            raw.iter()
                .map(|&b| match b {
                    // Webdings 'n' is a circle filling the whole letter height
                    b'n' => "⬤",
                    b'a' => "✓",
                    b'r' => "✗",
                    b'g' => "■",
                    b'c' => "□",
                    b' ' => " ",
                    _ => "●",
                })
                .collect::<String>()
        } else if st.wingdings {
            raw.iter().map(|&b| wingdings(b)).collect::<String>()
        } else if st.symbol {
            escape(&raw.iter().map(|&b| formula::symbol_char(b)).collect::<String>())
        } else {
            escape(&cp1252(raw))
        };
        if s.is_empty() {
            return;
        }
        let trimmed = s.trim();
        let mut out = s.clone();
        if let (Some(c), false) = (st.color, trimmed.is_empty()) {
            let lead = &s[..s.len() - s.trim_start().len()];
            let trail = &s[s.trim_end().len()..];
            let name = crate::markup::color_name(c);
            out = format!("{lead}{{{name}:{trimmed}}}{trail}");
            if st.wingdings || st.webdings {
                self.push(&out);
                return;
            }
        }
        if !trimmed.is_empty() && !st.wingdings && !st.webdings {
            let lead = &s[..s.len() - s.trim_start().len()];
            let trail = &s[s.trim_end().len()..];
            let mut inner = trimmed.to_string();
            if st.bold {
                inner = format!("**{inner}**");
            }
            if st.italic {
                inner = format!("*{inner}*");
            }
            if st.underline {
                inner = format!("++{inner}++");
            }
            if st.vshift > 0 {
                inner = format!("^{{{inner}}}");
            } else if st.vshift < 0 {
                inner = format!("_{{{inner}}}");
            }
            if let Some(c) = st.color {
                inner = format!("{{{}:{inner}}}", crate::markup::color_name(c));
            }
            out = format!("{lead}{inner}{trail}");
        }
        self.push(&out);
    }

    fn run(&mut self, data: &[u8]) {
        let mut l = Lines { d: data, pos: 0 };
        while let Some(hl) = l.line() {
            let hs = String::from_utf8_lossy(hl);
            let h: Vec<i64> = hs.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if h.len() < 2 {
                continue;
            }
            let style = h[0];
            if style == -8 {
                continue; // version
            }
            let count = h[1].max(0) as usize;
            let para = h.get(2).copied().unwrap_or(0);
            let opts = h.get(3).copied().unwrap_or(0);
            let mode = h.get(4).copied().unwrap_or(0);
            let starts = para != -1 && opts & 1 == 0;

            if style == -9 {
                let kind = h.get(6).copied().unwrap_or(0);
                let mut bin: &[u8] = &[];
                for i in 0..count {
                    if i + 1 == count && mode == 2 {
                        bin = l.binary().unwrap_or(&[]);
                    } else {
                        l.line();
                    }
                }
                if kind == 1 {
                    self.styles = parse_styles(bin);
                }
                continue;
            }

            if style >= 0 {
                for i in 0..count {
                    let t = l.line().unwrap_or(&[]);
                    if i > 0 || starts {
                        self.new_para();
                    }
                    self.text(style, t);
                }
                if count == 0 && starts {
                    self.new_para();
                }
                continue;
            }

            // special items: text lines, last one binary when mode == 2
            let mut texts: Vec<&[u8]> = Vec::new();
            let mut bin: &[u8] = &[];
            for i in 0..count {
                if i + 1 == count && mode == 2 {
                    bin = l.binary().unwrap_or(&[]);
                } else {
                    texts.push(l.line().unwrap_or(&[]));
                }
            }
            if starts {
                self.new_para();
            }
            let extra = |name: &str| -> Option<f32> {
                texts.iter().find_map(|t| {
                    let s = String::from_utf8_lossy(t);
                    s.strip_prefix(&format!("{name}=")).and_then(|v| v.trim().parse::<f32>().ok())
                })
            };
            match style {
                -1 => self.push("---"),
                -11 => self.push("• "),
                -12 => self.push("    "),
                -3 | -6 | -10 => {
                    if bin.is_empty() {
                        continue;
                    }
                    let (mut w, mut hh) = (extra("width").unwrap_or(0.0), extra("height").unwrap_or(0.0));
                    let img_start = [&b"\x89PNG"[..], b"\xFF\xD8\xFF", b"GIF8", b"BM"]
                        .iter()
                        .filter_map(|sig| find_bytes(&bin[..bin.len().min(64)], sig))
                        .min()
                        .unwrap_or(0);
                    let bytes = bin[img_start..].to_vec();
                    if w <= 0.0 || hh <= 0.0 {
                        if let Ok(r) = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format() {
                            if let Ok((iw, ih)) = r.into_dimensions() {
                                w = iw as f32;
                                hh = ih as f32;
                            }
                        }
                    }
                    let id = self.add_obj(ObjKind::Image, bytes, w, hh);
                    self.push(&format!("{{{{{id}}}}}"));
                }
                -5 => {
                    let mut placed = false;
                    if let Some((_class, props)) = read_component(bin) {
                        let w = find(&props, "Width").and_then(|v| v.as_int()).unwrap_or(0) as f32;
                        let hh = find(&props, "Height").and_then(|v| v.as_int()).unwrap_or(0) as f32;
                        if let Some(DVal::Binary(d)) = find(&props, "Data") {
                            if let Some(wmf) = formula::wmf_from_ole(d) {
                                let id = self.add_obj(ObjKind::Formula, wmf, w, hh);
                                self.push(&format!("{{{{{id}}}}}"));
                                placed = true;
                            }
                        }
                    }
                    if !placed {
                        self.push("[Objekt]");
                    }
                }
                -60 => {
                    if let Some(rows) = self.table(bin) {
                        // a table always stands in its own lines
                        if self.cur.as_deref().map(|c| !c.is_empty()).unwrap_or(false) {
                            self.new_para();
                        }
                        self.cur = None;
                        for r in rows {
                            self.lines.push(r);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn table(&mut self, bin: &[u8]) -> Option<Vec<String>> {
        let start = find_bytes(bin, b"TPF0")?;
        let mut r = Reader::new(&bin[start + 4..]);
        r.skip_flags()?;
        let _class = r.shortstr()?;
        let _name = r.shortstr()?;
        let mut rows = 0usize;
        let mut cols = 0usize;
        let mut border = 0i64;
        let mut cells: Vec<Option<CellSpan>> = Vec::new();
        loop {
            if r.peek()? == 0 {
                break;
            }
            let name = r.shortstr()?;
            if name == "C" {
                rows = r.value()?.as_int()?.max(0) as usize;
                cols = r.value()?.as_int()?.max(0) as usize;
                if r.u8()? != 1 {
                    return None;
                }
                loop {
                    let p = r.peek()?;
                    if p == 0 {
                        r.u8();
                        break;
                    }
                    if p == 13 {
                        r.u8();
                        cells.push(None);
                        continue;
                    }
                    let rs = r.value()?.as_int().unwrap_or(1).max(1) as usize;
                    let cs = r.value()?.as_int().unwrap_or(1).max(1) as usize;
                    if r.peek()? == 13 {
                        r.u8();
                        cells.push(None);
                        continue;
                    }
                    r.skip_flags()?;
                    let _c = r.shortstr()?;
                    let _n = r.shortstr()?;
                    let props = r.props()?;
                    if r.peek()? != 0 {
                        // nested components are not expected; give up on this table
                        return None;
                    }
                    r.u8();
                    cells.push(Some((rs, cs, props)));
                }
                break;
            } else {
                let v = r.value()?;
                if name == "T.BorderWidth" || name == "T.CellBorderWidth" {
                    border = border.max(v.as_int().unwrap_or(0));
                }
            }
        }
        if rows == 0 || cols == 0 {
            return None;
        }
        let mut grid: Vec<Vec<Option<String>>> = vec![vec![None; cols]; rows];
        let mut all_hidden = true;
        let mut it = cells.into_iter();
        for ri in 0..rows {
            for ci in 0..cols {
                if grid[ri][ci].is_some() {
                    continue;
                }
                let Some(cell) = it.next() else { break };
                let Some((rs, cs, props)) = cell else {
                    grid[ri][ci] = Some(String::new());
                    continue;
                };
                if !matches!(find(&props, "C.VisibleBorders.Left"), Some(DVal::Bool(false))) {
                    all_hidden = false;
                }
                let mut text = String::new();
                if let Some(DVal::Binary(d)) = find(&props, "C.Data") {
                    let inner = if d.len() >= 4 { &d[4..] } else { &d[..] };
                    let mut sub = Conv { objs: &mut *self.objs, styles: self.styles.clone(), lines: vec![], cur: None };
                    sub.run(inner);
                    if let Some(c) = sub.cur.take() {
                        sub.lines.push(c);
                    }
                    while sub.lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                        sub.lines.pop();
                    }
                    text = sub.lines.iter().map(|l| l.replace('|', "\\|")).collect::<Vec<_>>().join(" <br> ");
                }
                for row in grid.iter_mut().take((ri + rs).min(rows)).skip(ri) {
                    for cell in row.iter_mut().take((ci + cs).min(cols)).skip(ci) {
                        *cell = Some(String::new());
                    }
                }
                grid[ri][ci] = Some(text);
            }
        }
        let prefix = if border > 0 && !all_hidden { "+|" } else { "|" };
        Some(
            grid.into_iter()
                .map(|row| {
                    let cells: Vec<String> = row.into_iter().map(|c| c.unwrap_or_default()).collect();
                    format!("{prefix} {} |", cells.join(" | "))
                })
                .collect(),
        )
    }
}

/// A table cell read from the old format: row span, column span, properties.
type CellSpan = (usize, usize, Vec<(String, DVal)>);

fn to_cp1252(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            '€' => 0x80,
            '„' => 0x84,
            '…' => 0x85,
            '“' => 0x93,
            '”' => 0x94,
            '–' => 0x96,
            '—' => 0x97,
            '•' => 0x95,
            c if (c as u32) < 256 => c as u32 as u8,
            _ => b'?',
        })
        .collect()
}

/// Writes a document in the old rich text format (text and pictures; the
/// old program will show formatting as plain text).
pub fn write(doc: &Doc) -> Vec<u8> {
    use crate::markup::{parse_inline, Inl};
    let mut out: Vec<u8> = b"-8 1 3\r\n".to_vec();
    for line in doc.text.lines() {
        let line = line.trim_start_matches("+|").trim_start_matches('|');
        let line = line.replace(" | ", "\t").replace(" <br> ", " ").trim_end_matches('|').trim_end().to_string();
        let mut first = true;
        let inl = parse_inline(&line);
        if inl.is_empty() {
            out.extend_from_slice(b"0 1 0 0 0 0\r\n\r\n");
            continue;
        }
        for x in inl {
            let para: &[u8] = if first { b"0" } else { b"-1" };
            let opts: &[u8] = if first { b"0" } else { b"1" };
            match x {
                Inl::Text { s, .. } => {
                    out.extend_from_slice(b"0 1 ");
                    out.extend_from_slice(para);
                    out.push(b' ');
                    out.extend_from_slice(opts);
                    out.extend_from_slice(b" 0 0\r\n");
                    out.extend(to_cp1252(&s));
                    out.extend_from_slice(b"\r\n");
                }
                Inl::Check(on) => {
                    out.extend_from_slice(b"0 1 ");
                    out.extend_from_slice(para);
                    out.push(b' ');
                    out.extend_from_slice(opts);
                    out.extend_from_slice(b" 0 0\r\n");
                    out.extend_from_slice(if on { b"[X]" } else { b"[ ]" });
                    out.extend_from_slice(b"\r\n");
                }
                Inl::Link { text, url } => {
                    out.extend_from_slice(b"0 1 ");
                    out.extend_from_slice(para);
                    out.push(b' ');
                    out.extend_from_slice(opts);
                    out.extend_from_slice(b" 0 0\r\n");
                    out.extend(to_cp1252(&format!("{text} ({url})")));
                    out.extend_from_slice(b"\r\n");
                }
                Inl::Obj(id) => {
                    let Some(o) = doc.object(id) else { continue };
                    if o.kind != ObjKind::Image {
                        out.extend_from_slice(b"0 1 ");
                        out.extend_from_slice(para);
                        out.push(b' ');
                        out.extend_from_slice(opts);
                        if o.kind == ObjKind::Math {
                            let t = format!(" 0 0\r\n[Formel: {}]\r\n", crate::math::to_plain(&String::from_utf8_lossy(&o.data)));
                            out.extend(to_cp1252(&t));
                        } else {
                            out.extend_from_slice(b" 0 0\r\n[Formel]\r\n");
                        }
                        first = false;
                        continue;
                    }
                    let class: &[u8] = if o.data.starts_with(b"\x89PNG") {
                        b"TPNGObject"
                    } else if o.data.starts_with(b"GIF8") {
                        b"TGIFImage"
                    } else if o.data.starts_with(b"BM") {
                        b"TBitmap"
                    } else {
                        b"TJPEGImage"
                    };
                    out.extend_from_slice(b"-3 5 ");
                    out.extend_from_slice(para);
                    out.push(b' ');
                    out.extend_from_slice(opts);
                    out.extend_from_slice(b" 2 0 0\r\n\r\n");
                    out.extend_from_slice(class);
                    out.extend_from_slice(format!("\r\nwidth={:.0}\r\nheight={:.0}\r\n", o.width, o.height).as_bytes());
                    out.extend_from_slice(&(o.data.len() as u32).to_le_bytes());
                    out.extend_from_slice(&o.data);
                }
            }
            first = false;
        }
    }
    out
}

pub fn to_doc(data: &[u8]) -> Doc {
    let mut objs = Vec::new();
    let mut c = Conv { objs: &mut objs, styles: vec![], lines: vec![], cur: None };
    c.run(data);
    if let Some(cur) = c.cur.take() {
        c.lines.push(cur);
    }
    let mut lines = c.lines;
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    // join "**a****b**" artefacts from adjacent runs with the same style
    let text = lines.join("\n").replace("****", "").replace("^{}", "").replace("_{}", "");
    Doc { text, objects: objs }
}
