//! Import of tasks from Word (.docx) and RTF documents.
//!
//! The document is read paragraph by paragraph into Klassenwerk markup
//! (formatting, lists, headings, tables, pictures; Word formulas become
//! Klassenwerk formulas). It is then cut into tasks at marker lines, the same
//! markers the "KDWord" helper of KlasseDozent uses, so documents prepared for
//! it can be read directly:
//!
//! - `Aufgabe=3` or `A#3` starts task 3, `Lösung=3` or `L#3` its solution
//!   (tasks and solutions are paired by number, in any order)
//! - on the marker line: `Fach=` / `F#`, `Schlagwort1=` / `Swort1=` / `S1#`,
//!   `Schlagwort2=` / `Swort2=` / `S2#`, `Punkte=` / `P#`, `Z#` (blank lines),
//!   `ZU#` (blank lines, underlined); words joined with `_`, at most two
//!   spaces between term and value, terms in any case
//! - points in brackets in the solution, e.g. "(2 Punkte)", "(3 P.)", are added up
//!
//! A document without markers becomes one task.

use crate::model::{DocObject, ObjKind};
use std::collections::HashMap;

// ------------------------------------------------------------------ text model

#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct St {
    b: bool,
    i: bool,
    u: bool,
    sup: bool,
    sub: bool,
    color: Option<[u8; 3]>,
}

/// A piece of a paragraph: text with a style, or ready markup (`{{n}}`).
struct Seg {
    text: String,
    st: St,
    raw: bool,
}

/// One paragraph (or table row) of the document.
#[derive(Clone, Debug, Default)]
pub struct Para {
    pub markup: String,
    /// the text without formatting; empty for table rows
    pub plain: String,
    pub table: bool,
}

impl Para {
    fn is_empty(&self) -> bool {
        !self.table && self.markup.trim().is_empty()
    }
}

/// Paragraphs with their pictures and formulas (ids unique in the document).
#[derive(Default)]
pub struct Parsed {
    pub paras: Vec<Para>,
    pub objects: Vec<DocObject>,
    pub notes: Vec<String>,
}

fn nearest_color(c: [u8; 3]) -> Option<[u8; 3]> {
    // black, near black and grey-black stay plain text
    if c.iter().all(|v| *v < 60) {
        return None;
    }
    let d = |a: [u8; 3]| -> i32 { (0..3).map(|k| (a[k] as i32 - c[k] as i32).pow(2)).sum() };
    crate::markup::COLORS.iter().min_by_key(|(_, v)| d(*v)).map(|(_, v)| *v)
}

/// Markup of a paragraph: neighbouring pieces with the same style are joined,
/// then wrapped in the markup for their style (spaces stay outside).
fn segs_markup(segs: &[Seg]) -> (String, String) {
    let mut merged: Vec<Seg> = Vec::new();
    for s in segs {
        if s.text.is_empty() {
            continue;
        }
        if let Some(last) = merged.last_mut() {
            if !last.raw && !s.raw && last.st == s.st {
                last.text.push_str(&s.text);
                continue;
            }
        }
        merged.push(Seg { text: s.text.clone(), st: s.st, raw: s.raw });
    }
    let mut out = String::new();
    let mut plain = String::new();
    for s in &merged {
        if s.raw {
            out += &s.text;
            continue;
        }
        plain += &s.text;
        let t = s.text.trim();
        if t.is_empty() {
            out += &s.text.replace('\t', "    ");
            continue;
        }
        let lead = &s.text[..s.text.len() - s.text.trim_start().len()];
        let trail = &s.text[s.text.trim_end().len()..];
        let mut inner = crate::rvf::escape(t);
        if s.st.b {
            inner = format!("**{inner}**");
        }
        if s.st.i {
            inner = format!("*{inner}*");
        }
        if s.st.u {
            inner = format!("++{inner}++");
        }
        if s.st.sup {
            inner = format!("^{{{inner}}}");
        } else if s.st.sub {
            inner = format!("_{{{inner}}}");
        }
        if let Some(c) = s.st.color {
            inner = format!("{{{}:{inner}}}", crate::markup::color_name(c));
        }
        out += &format!("{}{inner}{}", lead.replace('\t', "    "), trail.replace('\t', "    "));
    }
    (out, plain)
}

/// A metafile without its 22-byte placeable header (Klassenwerk stores them so).
fn strip_placeable(d: &[u8]) -> &[u8] {
    if d.len() > 22 && d.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        &d[22..]
    } else {
        d
    }
}

fn image_kind(data: &[u8]) -> Option<ObjKind> {
    if data.starts_with(b"\x89PNG") || data.starts_with(&[0xFF, 0xD8]) || data.starts_with(b"GIF8") || data.starts_with(b"BM") {
        Some(ObjKind::Image)
    } else if crate::formula::parse_wmf(strip_placeable(data)).is_some() {
        // a Windows metafile: old formula objects keep their picture this way
        Some(ObjKind::Formula)
    } else {
        None
    }
}

// ------------------------------------------------------------------ zip

/// The files of a zip archive (stored or deflated entries).
pub fn unzip(data: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let u16le = |p: usize| -> usize { data.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).unwrap_or(0) };
    let u32le = |p: usize| -> usize { data.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).unwrap_or(0) };
    let eocd =
        (0..data.len().saturating_sub(21)).rev().find(|&p| data[p..].starts_with(&[0x50, 0x4b, 0x05, 0x06])).ok_or("keine Word-Datei (kein Zip-Archiv)")?;
    let count = u16le(eocd + 10);
    let mut p = u32le(eocd + 16);
    let mut out = HashMap::new();
    for _ in 0..count {
        if !data.get(p..).map(|d| d.starts_with(&[0x50, 0x4b, 0x01, 0x02])).unwrap_or(false) {
            break;
        }
        let method = u16le(p + 10);
        let csize = u32le(p + 20);
        let (nlen, xlen, clen) = (u16le(p + 28), u16le(p + 30), u16le(p + 32));
        let local = u32le(p + 42);
        let name = String::from_utf8_lossy(data.get(p + 46..p + 46 + nlen).unwrap_or_default()).to_string();
        p += 46 + nlen + xlen + clen;
        let start = local + 30 + u16le(local + 26) + u16le(local + 28);
        let Some(raw) = data.get(start..start + csize) else { continue };
        let bytes = match method {
            0 => raw.to_vec(),
            8 => match miniz_oxide::inflate::decompress_to_vec(raw) {
                Ok(b) => b,
                Err(_) => continue,
            },
            _ => continue,
        };
        out.insert(name, bytes);
    }
    Ok(out)
}

// ------------------------------------------------------------------ docx

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn attr<'a>(n: roxmltree::Node<'a, '_>, ns: &str, name: &str) -> Option<&'a str> {
    n.attribute((ns, name))
}
fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, ns: &str, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.tag_name().name() == name && c.tag_name().namespace() == Some(ns))
}
fn is(n: roxmltree::Node, ns: &str, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(ns)
}

struct Docx<'f> {
    files: &'f HashMap<String, Vec<u8>>,
    rels: HashMap<String, String>,
    out: Parsed,
    next_id: u32,
}

impl<'f> Docx<'f> {
    fn add_obj(&mut self, kind: ObjKind, data: Vec<u8>, w: f32, h: f32) -> String {
        let (w, h) = if kind == ObjKind::Math { crate::math::size_px(&crate::math::layout(&String::from_utf8_lossy(&data))) } else { (w, h) };
        self.next_id += 1;
        let id = self.next_id;
        self.out.objects.push(DocObject { id, kind, data, width: w, height: h });
        format!("{{{{{id}}}}}")
    }

    fn picture(&mut self, rid: &str, w: f32, h: f32) -> Option<String> {
        let target = self.rels.get(rid)?.clone();
        let path = if let Some(t) = target.strip_prefix('/') { t.to_string() } else { format!("word/{target}") };
        let data = self.files.get(&path)?.clone();
        match image_kind(&data) {
            Some(ObjKind::Formula) => Some(self.add_obj(ObjKind::Formula, strip_placeable(&data).to_vec(), w, h)),
            Some(k) => Some(self.add_obj(k, data, w, h)),
            None => {
                let ext = path.rsplit('.').next().unwrap_or("").to_uppercase();
                self.out.notes.push(format!("Ein Bild im Format {ext} wurde nicht übernommen."));
                None
            }
        }
    }

    fn body(&mut self, n: roxmltree::Node) {
        for c in n.children().filter(|c| c.is_element()) {
            if is(c, W, "p") {
                let p = self.para(c);
                self.out.paras.extend(p);
            } else if is(c, W, "tbl") {
                self.table(c);
            } else if is(c, W, "sdt") {
                if let Some(content) = child(c, W, "sdtContent") {
                    self.body(content);
                }
            } else if is(c, W, "customXml") {
                self.body(c);
            }
        }
    }

    /// A paragraph; a line break inside it starts a new line (same paragraph style).
    fn para(&mut self, p: roxmltree::Node) -> Vec<Para> {
        let mut prefix = "";
        if let Some(ppr) = child(p, W, "pPr") {
            let style = child(ppr, W, "pStyle").and_then(|s| attr(s, W, "val")).unwrap_or("").to_lowercase();
            if style.starts_with("heading") || style.contains("berschrift") || style == "title" || style == "titel" {
                prefix = "# ";
            } else if child(ppr, W, "numPr").is_some() || style.contains("list") || style.contains("liste") {
                prefix = "• ";
            }
        }
        let mut lines: Vec<Vec<Seg>> = vec![Vec::new()];
        self.inline(p, St::default(), &mut lines);
        lines
            .into_iter()
            .enumerate()
            .map(|(k, segs)| {
                let (m, plain) = segs_markup(&segs);
                let pre = if k == 0 || prefix == "# " { prefix } else { "" };
                let markup = if m.trim().is_empty() { String::new() } else { format!("{pre}{m}") };
                Para { markup, plain, table: false }
            })
            .collect()
    }

    fn inline(&mut self, n: roxmltree::Node, base: St, lines: &mut Vec<Vec<Seg>>) {
        for c in n.children().filter(|c| c.is_element()) {
            let name = c.tag_name().name();
            if is(c, W, "r") {
                self.run(c, base, lines);
            } else if is(c, M, "oMath") {
                let tex = omml(c);
                if !tex.trim().is_empty() {
                    let m = self.add_obj(ObjKind::Math, tex.into_bytes(), 0.0, 0.0);
                    lines.last_mut().unwrap().push(Seg { text: m, st: St::default(), raw: true });
                }
            } else if is(c, M, "oMathPara") {
                for m in c.children().filter(|x| is(*x, M, "oMath")) {
                    let tex = omml(m);
                    if !tex.trim().is_empty() {
                        let s = self.add_obj(ObjKind::Math, tex.into_bytes(), 0.0, 0.0);
                        lines.last_mut().unwrap().push(Seg { text: s, st: St::default(), raw: true });
                    }
                }
            } else if c.tag_name().namespace() == Some(W) && ["hyperlink", "ins", "smartTag", "fldSimple", "customXml", "sdt", "sdtContent"].contains(&name) {
                self.inline(c, base, lines);
            }
        }
    }

    fn run(&mut self, r: roxmltree::Node, base: St, lines: &mut Vec<Vec<Seg>>) {
        let mut st = base;
        if let Some(rpr) = child(r, W, "rPr") {
            let on = |name: &str| child(rpr, W, name).map(|x| !matches!(attr(x, W, "val"), Some("0") | Some("false") | Some("none"))).unwrap_or(false);
            st.b = on("b");
            st.i = on("i");
            st.u = on("u");
            if let Some(v) = child(rpr, W, "vertAlign").and_then(|x| attr(x, W, "val")) {
                st.sup = v == "superscript";
                st.sub = v == "subscript";
            }
            if let Some(c) = child(rpr, W, "color").and_then(|x| attr(x, W, "val")) {
                if c != "auto" && c.len() == 6 {
                    let v = u32::from_str_radix(c, 16).unwrap_or(0);
                    st.color = nearest_color([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
                }
            }
            if child(rpr, W, "vanish").is_some() {
                return;
            }
        }
        for c in r.children().filter(|c| c.is_element()) {
            let name = c.tag_name().name();
            let seg = |t: String| Seg { text: t, st, raw: false };
            match name {
                "t" if is(c, W, "t") => lines.last_mut().unwrap().push(seg(c.text().unwrap_or("").to_string())),
                "tab" => lines.last_mut().unwrap().push(seg("\t".into())),
                "noBreakHyphen" => lines.last_mut().unwrap().push(seg("-".into())),
                "sym" => {
                    let ch = attr(c, W, "char").and_then(|h| u32::from_str_radix(h, 16).ok()).unwrap_or(0);
                    let font = attr(c, W, "font").unwrap_or("").to_lowercase();
                    let s = if font.contains("symbol") {
                        crate::formula::symbol_char((ch & 0xFF) as u8).to_string()
                    } else {
                        char::from_u32(ch).map(|c| c.to_string()).unwrap_or_default()
                    };
                    lines.last_mut().unwrap().push(seg(s));
                }
                "br" | "cr" => {
                    if attr(c, W, "type") != Some("page") {
                        lines.push(Vec::new());
                    }
                }
                "drawing" => {
                    let ext = c.descendants().find(|x| x.tag_name().name() == "extent");
                    let px = |a: &str| ext.and_then(|e| e.attribute(a)).and_then(|v| v.parse::<f32>().ok()).map(|v| v / 9525.0).unwrap_or(0.0);
                    let (w, h) = (px("cx"), px("cy"));
                    if let Some(rid) = c.descendants().find(|x| x.tag_name().name() == "blip").and_then(|b| b.attribute((R, "embed"))) {
                        if let Some(m) = self.picture(rid, w, h) {
                            lines.last_mut().unwrap().push(Seg { text: m, st: St::default(), raw: true });
                        }
                    }
                }
                "pict" | "object" => {
                    // old pictures and OLE objects (MathType, Equation 3.0): their picture
                    let shape = c.descendants().find(|x| x.tag_name().name() == "shape");
                    let (w, h) = shape.and_then(|s| s.attribute("style")).map(vml_size).unwrap_or((0.0, 0.0));
                    if let Some(rid) = c.descendants().find(|x| x.tag_name().name() == "imagedata").and_then(|b| b.attribute((R, "id"))) {
                        if let Some(m) = self.picture(rid, w, h) {
                            lines.last_mut().unwrap().push(Seg { text: m, st: St::default(), raw: true });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn table(&mut self, t: roxmltree::Node) {
        // a frame if the table or its style says so (most Word tables have one)
        let bordered = child(t, W, "tblPr")
            .map(|p| {
                let explicit =
                    child(p, W, "tblBorders").map(|b| b.children().any(|e| e.is_element() && !matches!(attr(e, W, "val"), Some("nil") | Some("none"))));
                explicit.unwrap_or_else(|| child(p, W, "tblStyle").is_some())
            })
            .unwrap_or(false);
        for row in t.children().filter(|c| is(*c, W, "tr")) {
            let mut cells: Vec<String> = Vec::new();
            for cell in row.children().filter(|c| is(*c, W, "tc")) {
                let mut parts: Vec<String> = Vec::new();
                for p in cell.children().filter(|c| is(*c, W, "p")) {
                    for para in self.para(p) {
                        if !para.markup.trim().is_empty() {
                            parts.push(para.markup);
                        }
                    }
                }
                let span = child(cell, W, "tcPr")
                    .and_then(|p| child(p, W, "gridSpan"))
                    .and_then(|g| attr(g, W, "val"))
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1);
                let vcont = child(cell, W, "tcPr").and_then(|p| child(p, W, "vMerge")).map(|v| attr(v, W, "val") != Some("restart")).unwrap_or(false);
                cells.push(if vcont { "^".into() } else { parts.join("<br>") });
                for _ in 1..span {
                    cells.push("<".into());
                }
            }
            let pre = if bordered { "+|" } else { "|" };
            let mut line = pre.to_string();
            for c in cells {
                line += &if c.is_empty() { " |".to_string() } else { format!(" {c} |") };
            }
            self.out.paras.push(Para { markup: line, plain: String::new(), table: true });
        }
        // a table ends with an empty line, so the next paragraph is not glued to it
        self.out.paras.push(Para::default());
    }
}

/// "width:120pt;height:30.5pt" -> pixels
fn vml_size(style: &str) -> (f32, f32) {
    let get = |k: &str| {
        style.split(';').find_map(|p| {
            let (a, b) = p.split_once(':')?;
            if a.trim() != k {
                return None;
            }
            let b = b.trim();
            let (num, unit) = b.split_at(b.find(|c: char| c.is_alphabetic()).unwrap_or(b.len()));
            let v: f32 = num.parse().ok()?;
            Some(match unit {
                "pt" => v * 96.0 / 72.0,
                "in" => v * 96.0,
                "cm" => v * 96.0 / 2.54,
                "mm" => v * 96.0 / 25.4,
                _ => v,
            })
        })
    };
    (get("width").unwrap_or(0.0), get("height").unwrap_or(0.0))
}

/// Reads a Word document (.docx).
pub fn read_docx(data: &[u8]) -> Result<Parsed, String> {
    let files = unzip(data)?;
    let doc = files.get("word/document.xml").ok_or("keine Word-Datei (word/document.xml fehlt)")?;
    let xml = String::from_utf8_lossy(doc);
    let tree = roxmltree::Document::parse(&xml).map_err(|e| format!("Word-Datei nicht lesbar: {e}"))?;
    let mut rels = HashMap::new();
    if let Some(r) = files.get("word/_rels/document.xml.rels") {
        let s = String::from_utf8_lossy(r);
        if let Ok(t) = roxmltree::Document::parse(&s) {
            for n in t.descendants().filter(|n| n.tag_name().name() == "Relationship") {
                if let (Some(id), Some(target)) = (n.attribute("Id"), n.attribute("Target")) {
                    rels.insert(id.to_string(), target.to_string());
                }
            }
        }
    }
    let mut d = Docx { files: &files, rels, out: Parsed::default(), next_id: 0 };
    let body = tree.root_element().children().find(|c| is(*c, W, "body")).ok_or("Word-Datei ohne Inhalt")?;
    d.body(body);
    Ok(d.out)
}

// ------------------------------------------------------------------ OMML -> formula source

fn omml_children(n: roxmltree::Node) -> String {
    n.children().filter(|c| c.is_element()).map(omml).collect::<Vec<_>>().join("")
}

fn mpart(n: roxmltree::Node, name: &str) -> String {
    child(n, M, name).map(omml_children).unwrap_or_default()
}

fn mprop<'a>(n: roxmltree::Node<'a, '_>, pr: &str, name: &str) -> Option<&'a str> {
    child(n, M, pr).and_then(|p| child(p, M, name)).map(|x| attr(x, M, "val").unwrap_or(""))
}

fn tex_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '{' => out += "\\{",
            '}' => out += "\\}",
            '\\' => out += "\\backslash ",
            '·' | '⋅' => out += "\\cdot ",
            '×' => out += "\\times ",
            '\u{2061}' => {}
            _ => out.push(c),
        }
    }
    out
}

fn delim(c: &str, left: bool) -> String {
    match c {
        "" => ".".into(),
        "{" => if left { "\\{" } else { "\\}" }.into(),
        "}" => "\\}".into(),
        "⟨" | "〈" => "\\langle".into(),
        "⟩" | "〉" => "\\rangle".into(),
        "‖" => "\\|".into(),
        c => c.into(),
    }
}

/// Word formula (Office Math) as Klassenwerk formula source (LaTeX-like).
pub fn omml(n: roxmltree::Node) -> String {
    if !n.is_element() || n.tag_name().namespace() != Some(M) {
        // text runs of Word inside a formula
        if is(n, W, "r") {
            return n.children().filter(|c| is(*c, W, "t")).map(|c| tex_text(c.text().unwrap_or(""))).collect();
        }
        return String::new();
    }
    match n.tag_name().name() {
        "r" => {
            let t: String = n.children().filter(|c| is(*c, M, "t")).map(|c| c.text().unwrap_or("").to_string()).collect();
            let normal = child(n, M, "rPr").and_then(|p| child(p, M, "nor")).is_some()
                || child(n, M, "rPr").and_then(|p| child(p, M, "sty")).and_then(|s| attr(s, M, "val")) == Some("p");
            if normal && t.chars().filter(|c| c.is_alphabetic()).count() > 1 {
                format!("\\text{{{}}}", t.replace(['{', '}'], ""))
            } else {
                tex_text(&t)
            }
        }
        "f" => format!("\\frac{{{}}}{{{}}}", mpart(n, "num"), mpart(n, "den")),
        "sSup" => format!("{{{}}}^{{{}}}", mpart(n, "e"), mpart(n, "sup")),
        "sSub" => format!("{{{}}}_{{{}}}", mpart(n, "e"), mpart(n, "sub")),
        "sSubSup" => format!("{{{}}}_{{{}}}^{{{}}}", mpart(n, "e"), mpart(n, "sub"), mpart(n, "sup")),
        "sPre" => format!("{{}}_{{{}}}^{{{}}}{{{}}}", mpart(n, "sub"), mpart(n, "sup"), mpart(n, "e")),
        "rad" => {
            let deg = mpart(n, "deg");
            if deg.trim().is_empty() || mprop(n, "radPr", "degHide").map(|v| v != "0" && v != "off").unwrap_or(false) {
                format!("\\sqrt{{{}}}", mpart(n, "e"))
            } else {
                format!("\\sqrt[{deg}]{{{}}}", mpart(n, "e"))
            }
        }
        "d" => {
            let beg = mprop(n, "dPr", "begChr").map(|s| s.to_string()).unwrap_or_else(|| "(".into());
            let end = mprop(n, "dPr", "endChr").map(|s| s.to_string()).unwrap_or_else(|| ")".into());
            let sep = mprop(n, "dPr", "sepChr").map(|s| s.to_string()).unwrap_or_else(|| "|".into());
            let es: Vec<roxmltree::Node> = n.children().filter(|c| is(*c, M, "e")).collect();
            // a matrix in brackets
            if es.len() == 1 {
                let inner: Vec<roxmltree::Node> = es[0].children().filter(|c| c.is_element()).collect();
                if inner.len() == 1 && is(inner[0], M, "m") {
                    let env = match (beg.as_str(), end.as_str()) {
                        ("(", ")") => Some("pmatrix"),
                        ("[", "]") => Some("bmatrix"),
                        ("|", "|") => Some("vmatrix"),
                        ("{", "}") => Some("Bmatrix"),
                        _ => None,
                    };
                    if let Some(env) = env {
                        return format!("\\begin{{{env}}}{}\\end{{{env}}}", matrix_rows(inner[0]));
                    }
                }
                // a curly bracket only on the left with rows: cases
                if beg == "{" && end.is_empty() && inner.len() == 1 && is(inner[0], M, "eqArr") {
                    let rows: Vec<String> = inner[0].children().filter(|c| is(*c, M, "e")).map(omml_children).collect();
                    return format!("\\begin{{cases}}{}\\end{{cases}}", rows.join("\\\\"));
                }
            }
            let parts: Vec<String> = es.iter().map(|e| omml_children(*e)).collect();
            let sep = if sep.is_empty() { String::new() } else { tex_text(&sep) };
            format!("\\left{}{}\\right{}", delim(&beg, true), parts.join(&sep), delim(&end, false))
        }
        "nary" => {
            let chr = mprop(n, "naryPr", "chr").unwrap_or("∫");
            let op = match chr {
                "∑" => "\\sum",
                "∏" => "\\prod",
                "∬" => "\\iint",
                "∮" => "\\oint",
                "⋃" => "\\bigcup",
                "⋂" => "\\bigcap",
                _ => "\\int",
            };
            let (sub, sup) = (mpart(n, "sub"), mpart(n, "sup"));
            let mut s = op.to_string();
            if !sub.trim().is_empty() {
                s += &format!("_{{{sub}}}");
            }
            if !sup.trim().is_empty() {
                s += &format!("^{{{sup}}}");
            }
            format!("{s} {}", mpart(n, "e"))
        }
        "func" => {
            let name = mpart(n, "fName");
            let known = ["sin", "cos", "tan", "cot", "ln", "log", "exp", "lim", "max", "min", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh"];
            let plain: String = name.chars().filter(|c| c.is_alphabetic()).collect();
            let head = if known.contains(&plain.as_str()) && name.trim() == plain { format!("\\{plain}") } else { name };
            format!("{head} {}", mpart(n, "e"))
        }
        "limLow" => {
            let e = mpart(n, "e");
            let lim = mpart(n, "lim");
            if e.trim() == "lim" || e.trim() == "\\lim" {
                format!("\\lim_{{{lim}}}")
            } else {
                format!("{{{e}}}_{{{lim}}}")
            }
        }
        "limUpp" => format!("{{{}}}^{{{}}}", mpart(n, "e"), mpart(n, "lim")),
        "acc" => {
            let chr = mprop(n, "accPr", "chr").unwrap_or("\u{302}");
            let cmd = match chr {
                "\u{20D7}" | "→" => "vec",
                "\u{305}" | "\u{304}" | "¯" => "bar",
                "\u{307}" | "˙" => "dot",
                _ => "hat",
            };
            format!("\\{cmd}{{{}}}", mpart(n, "e"))
        }
        "bar" => format!("\\overline{{{}}}", mpart(n, "e")),
        "m" => format!("\\begin{{matrix}}{}\\end{{matrix}}", matrix_rows(n)),
        "eqArr" => {
            let rows: Vec<String> = n.children().filter(|c| is(*c, M, "e")).map(omml_children).collect();
            format!("\\begin{{aligned}}{}\\end{{aligned}}", rows.join("\\\\"))
        }
        "groupChr" | "box" | "borderBox" | "phant" => mpart(n, "e"),
        name if name.ends_with("Pr") => String::new(),
        _ => omml_children(n),
    }
}

fn matrix_rows(m: roxmltree::Node) -> String {
    m.children()
        .filter(|c| is(*c, M, "mr"))
        .map(|r| r.children().filter(|c| is(*c, M, "e")).map(omml_children).collect::<Vec<_>>().join("&"))
        .collect::<Vec<_>>()
        .join("\\\\")
}

// ------------------------------------------------------------------ rtf

#[derive(Clone, Copy, Default)]
struct RState {
    st: St,
    font: i32,
    /// text of this group is not shown (font table, header, picture …)
    skip: bool,
    uc: usize,
    color_idx: usize,
}

/// Reads an RTF document.
pub fn read_rtf(data: &[u8]) -> Result<Parsed, String> {
    if !data.starts_with(b"{\\rtf") {
        return Err("keine RTF-Datei".into());
    }
    let mut out = Parsed::default();
    let mut next_id = 0u32;
    let mut stack: Vec<RState> = Vec::new();
    let mut cur = RState { uc: 1, ..Default::default() };
    let mut colors: Vec<Option<[u8; 3]>> = Vec::new();
    let mut col = [0u8; 3];
    let mut symbol_fonts: Vec<i32> = Vec::new();
    let mut font_name = String::new();
    let mut font_no = 0i32;
    // destinations: 1 font table, 2 color table, 3 picture, 4 list text
    let mut dest_stack: Vec<u8> = Vec::new();
    let mut dest: u8 = 0;
    let mut pict_hex = String::new();
    let mut pict_kind: Option<&str> = None;
    let mut pict_size = (0.0f32, 0.0f32);
    let mut segs: Vec<Seg> = Vec::new();
    let mut bullet = false;
    let mut heading = false;
    let mut in_table = false;
    let mut cell_parts: Vec<String> = Vec::new();
    let mut row_cells: Vec<String> = Vec::new();
    let mut row_bordered = false;
    let mut skip_chars = 0usize;
    let mut ignorable = false;
    let mut i = 0usize;

    let end_para = |segs: &mut Vec<Seg>, bullet: &mut bool, heading: &mut bool, in_table: bool, cell_parts: &mut Vec<String>, out: &mut Parsed| {
        let (m, plain) = segs_markup(segs);
        segs.clear();
        if in_table {
            if !m.trim().is_empty() {
                cell_parts.push(m);
            }
        } else {
            let pre = if *heading {
                "# "
            } else if *bullet {
                "• "
            } else {
                ""
            };
            let markup = if m.trim().is_empty() { String::new() } else { format!("{pre}{m}") };
            out.paras.push(Para { markup, plain, table: false });
        }
        *bullet = false;
        *heading = false;
    };

    while i < data.len() {
        let c = data[i];
        match c {
            b'{' => {
                stack.push(cur);
                dest_stack.push(dest);
                i += 1;
            }
            b'}' => {
                if dest == 3 && dest_stack.last() != Some(&3) {
                    // end of a picture
                    let bytes: Vec<u8> = (0..pict_hex.len() / 2).filter_map(|k| u8::from_str_radix(&pict_hex[2 * k..2 * k + 2], 16).ok()).collect();
                    let kind = match pict_kind {
                        // RTF holds the metafile without its placeable header, as Klassenwerk stores it
                        Some("wmf") => crate::formula::parse_wmf(&bytes).is_some().then_some((ObjKind::Formula, bytes)),
                        Some(_) => image_kind(&bytes).map(|k| (k, bytes)),
                        None => None,
                    };
                    if let Some((k, d)) = kind {
                        next_id += 1;
                        out.objects.push(DocObject { id: next_id, kind: k, data: d, width: pict_size.0, height: pict_size.1 });
                        segs.push(Seg { text: format!("{{{{{next_id}}}}}"), st: St::default(), raw: true });
                    } else if pict_kind.is_some() {
                        out.notes.push("Ein Bild in einem nicht unterstützten Format wurde nicht übernommen.".into());
                    }
                    pict_hex.clear();
                    pict_kind = None;
                }
                if dest == 1 && dest_stack.last() != Some(&1) {
                    // nothing: fonts are finished at ';'
                }
                cur = stack.pop().unwrap_or_default();
                dest = dest_stack.pop().unwrap_or(0);
                i += 1;
            }
            b'\\' => {
                i += 1;
                let Some(&n) = data.get(i) else { break };
                if n.is_ascii_alphabetic() {
                    let s = i;
                    while i < data.len() && data[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word = std::str::from_utf8(&data[s..i]).unwrap_or("");
                    let ns = i;
                    if i < data.len() && (data[i] == b'-' || data[i].is_ascii_digit()) {
                        i += 1;
                        while i < data.len() && data[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let param: Option<i32> = std::str::from_utf8(&data[ns..i]).ok().and_then(|p| p.parse().ok());
                    if i < data.len() && data[i] == b' ' {
                        i += 1;
                    }
                    if ignorable {
                        ignorable = false;
                        // pictures of shapes and the stand-in picture of Word formulas are shown
                        if !matches!(word, "shppict" | "listtext" | "pntext" | "mmathPict") {
                            cur.skip = true;
                            dest = 9;
                            continue;
                        }
                    }
                    let on = param.map(|p| p != 0).unwrap_or(true);
                    match word {
                        "fonttbl" => {
                            dest = 1;
                            cur.skip = true;
                        }
                        "colortbl" => {
                            dest = 2;
                            cur.skip = true;
                        }
                        "stylesheet" | "info" | "header" | "footer" | "headerl" | "headerr" | "headerf" | "footerl" | "footerr" | "footerf" | "footnote"
                        | "fldinst" | "objdata" | "themedata" | "datastore" | "latentstyles" | "rsidtbl" | "generator" | "xmlnstbl" | "listtable"
                        | "listoverridetable" | "nonshppict" | "bkmkstart" | "bkmkend" => {
                            cur.skip = true;
                            dest = 9;
                        }
                        "pntext" | "listtext" => {
                            bullet = true;
                            cur.skip = true;
                            dest = 4;
                        }
                        "pict" => {
                            dest = 3;
                            cur.skip = true;
                            pict_size = (0.0, 0.0);
                        }
                        "pngblip" => pict_kind = Some("png"),
                        "jpegblip" => pict_kind = Some("jpg"),
                        "wmetafile" => pict_kind = Some("wmf"),
                        "emfblip" => pict_kind = Some("emf"),
                        "picwgoal" => pict_size.0 = param.unwrap_or(0) as f32 / 15.0,
                        "pichgoal" => pict_size.1 = param.unwrap_or(0) as f32 / 15.0,
                        "f" if dest == 1 => font_no = param.unwrap_or(0),
                        "f" => cur.font = param.unwrap_or(0),
                        "red" => col[0] = param.unwrap_or(0) as u8,
                        "green" => col[1] = param.unwrap_or(0) as u8,
                        "blue" => col[2] = param.unwrap_or(0) as u8,
                        "b" => cur.st.b = on,
                        "i" => cur.st.i = on,
                        "ul" => cur.st.u = on,
                        "ulnone" => cur.st.u = false,
                        "super" => {
                            cur.st.sup = true;
                            cur.st.sub = false;
                        }
                        "sub" => {
                            cur.st.sub = true;
                            cur.st.sup = false;
                        }
                        "nosupersub" => {
                            cur.st.sup = false;
                            cur.st.sub = false;
                        }
                        "plain" => {
                            cur.st = St::default();
                        }
                        "cf" => {
                            cur.color_idx = param.unwrap_or(0).max(0) as usize;
                            cur.st.color = colors.get(cur.color_idx).copied().flatten().and_then(nearest_color);
                        }
                        "uc" => cur.uc = param.unwrap_or(1).max(0) as usize,
                        "u" => {
                            let v = param.unwrap_or(0);
                            let v = if v < 0 { v + 65536 } else { v } as u32;
                            if !cur.skip {
                                if let Some(ch) = char::from_u32(v) {
                                    segs.push(Seg { text: ch.to_string(), st: cur.st, raw: false });
                                }
                            }
                            skip_chars = cur.uc;
                        }
                        "par" if !cur.skip => end_para(&mut segs, &mut bullet, &mut heading, in_table, &mut cell_parts, &mut out),
                        "line" if !cur.skip => {
                            if in_table {
                                segs.push(Seg { text: "<br>".into(), st: St::default(), raw: true });
                            } else {
                                end_para(&mut segs, &mut bullet, &mut heading, false, &mut cell_parts, &mut out);
                            }
                        }
                        "tab" if !cur.skip => segs.push(Seg { text: "\t".into(), st: cur.st, raw: false }),
                        "outlinelevel" => heading = true,
                        "pard" => {
                            in_table = false;
                        }
                        "intbl" => in_table = true,
                        "trowd" => {
                            row_bordered = false;
                            row_cells.clear();
                        }
                        "clbrdrt" | "clbrdrb" | "clbrdrl" | "clbrdrr" | "trbrdrt" | "trbrdrb" => row_bordered = true,
                        "cell" if !cur.skip => {
                            let (m, _) = segs_markup(&segs);
                            segs.clear();
                            if !m.trim().is_empty() {
                                cell_parts.push(m);
                            }
                            row_cells.push(cell_parts.join("<br>"));
                            cell_parts.clear();
                        }
                        "row" if !cur.skip => {
                            let mut line = if row_bordered { "+|" } else { "|" }.to_string();
                            for c in row_cells.drain(..) {
                                line += &if c.is_empty() { " |".to_string() } else { format!(" {c} |") };
                            }
                            out.paras.push(Para { markup: line, plain: String::new(), table: true });
                            in_table = false;
                        }
                        "emdash" => segs.push(Seg { text: "—".into(), st: cur.st, raw: false }),
                        "endash" => segs.push(Seg { text: "–".into(), st: cur.st, raw: false }),
                        "bullet" => segs.push(Seg { text: "•".into(), st: cur.st, raw: false }),
                        "lquote" => segs.push(Seg { text: "‘".into(), st: cur.st, raw: false }),
                        "rquote" => segs.push(Seg { text: "’".into(), st: cur.st, raw: false }),
                        "ldblquote" => segs.push(Seg { text: "“".into(), st: cur.st, raw: false }),
                        "rdblquote" => segs.push(Seg { text: "”".into(), st: cur.st, raw: false }),
                        _ => {}
                    }
                } else {
                    i += 1;
                    match n {
                        b'*' => ignorable = true,
                        b'\'' => {
                            let h = std::str::from_utf8(data.get(i..i + 2).unwrap_or(b"")).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                            i += 2;
                            if skip_chars > 0 {
                                skip_chars -= 1;
                            } else if let Some(b) = h {
                                if dest == 1 {
                                    font_name.push(b as char);
                                } else if !cur.skip {
                                    let s =
                                        if symbol_fonts.contains(&cur.font) { crate::formula::symbol_char(b).to_string() } else { crate::delphi::cp1252(&[b]) };
                                    segs.push(Seg { text: s, st: cur.st, raw: false });
                                }
                            }
                        }
                        b'~' if !cur.skip => segs.push(Seg { text: "\u{a0}".into(), st: cur.st, raw: false }),
                        b'_' if !cur.skip => segs.push(Seg { text: "-".into(), st: cur.st, raw: false }),
                        b'\\' | b'{' | b'}' if !cur.skip => segs.push(Seg { text: (n as char).to_string(), st: cur.st, raw: false }),
                        b'\n' | b'\r' if !cur.skip => end_para(&mut segs, &mut bullet, &mut heading, in_table, &mut cell_parts, &mut out),
                        _ => {}
                    }
                }
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                i += 1;
                if skip_chars > 0 {
                    skip_chars -= 1;
                    continue;
                }
                match dest {
                    1 => {
                        if c == b';' {
                            if font_name.to_lowercase().contains("symbol") {
                                symbol_fonts.push(font_no);
                            }
                            font_name.clear();
                        } else {
                            font_name.push(c as char);
                        }
                    }
                    2 => {
                        if c == b';' {
                            colors.push(if colors.is_empty() && col == [0, 0, 0] { None } else { Some(col) });
                            col = [0, 0, 0];
                        }
                    }
                    3 => {
                        if c.is_ascii_hexdigit() {
                            pict_hex.push(c as char);
                        }
                    }
                    _ if !cur.skip => {
                        let s = if symbol_fonts.contains(&cur.font) { crate::formula::symbol_char(c).to_string() } else { crate::delphi::cp1252(&[c]) };
                        segs.push(Seg { text: s, st: cur.st, raw: false });
                    }
                    _ => {}
                }
            }
        }
    }
    if !segs.is_empty() {
        end_para(&mut segs, &mut bullet, &mut heading, false, &mut cell_parts, &mut out);
    }
    Ok(out)
}

// ------------------------------------------------------------------ markers and tasks

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marker {
    pub solution: bool,
    pub nr: u32,
    pub subject: Option<String>,
    pub topic: Option<String>,
    pub subtopic: Option<String>,
    pub points: Option<u32>,
    /// blank lines and whether they are underlined
    pub lines: Option<(u32, bool)>,
    /// text after the markers on the same line
    pub rest: String,
}

/// Reads a marker line ("Aufgabe=3 Fach=Physik P#4", "L#3").
pub fn marker(line: &str) -> Option<Marker> {
    let chars: Vec<char> = line.trim().chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let starts = |pos: usize, s: &str| -> bool {
        let sc: Vec<char> = s.chars().collect();
        lower.get(pos..pos + sc.len()).map(|x| x == sc.as_slice()).unwrap_or(false)
    };
    let skip_sp = |mut p: usize, max: usize| {
        let mut n = 0;
        while p < chars.len() && chars[p] == ' ' && n < max {
            p += 1;
            n += 1;
        }
        p
    };
    let mut m = Marker::default();
    let mut p = 0;
    let mut found = false;
    for (key, sol) in [("aufgabe=", false), ("a#", false), ("lösung=", true), ("loesung=", true), ("l#", true)] {
        if starts(0, key) {
            p = skip_sp(key.chars().count(), 2);
            let s = p;
            while p < chars.len() && chars[p].is_ascii_digit() {
                p += 1;
            }
            if p == s {
                return None;
            }
            m.nr = chars[s..p].iter().collect::<String>().parse().ok()?;
            m.solution = sol;
            found = true;
            break;
        }
    }
    if !found {
        return None;
    }
    let keys: [(&str, u8); 16] = [
        ("schlagwort1=", 1),
        ("schlagwort2=", 2),
        ("swort1=", 1),
        ("swort2=", 2),
        ("fach=", 0),
        ("punkte=", 3),
        ("s1#", 1),
        ("s2#", 2),
        ("zu#", 5),
        ("f#", 0),
        ("p#", 3),
        ("z#", 4),
        ("titel=", 2),
        ("t#", 2),
        ("thema=", 1),
        ("zeilen=", 4),
    ];
    loop {
        while p < chars.len() && chars[p].is_whitespace() {
            p += 1;
        }
        if p >= chars.len() {
            break;
        }
        let Some((key, what)) = keys.iter().find(|(k, _)| starts(p, k)) else {
            m.rest = chars[p..].iter().collect::<String>().trim().to_string();
            break;
        };
        p = skip_sp(p + key.chars().count(), 2);
        let s = p;
        while p < chars.len() && !chars[p].is_whitespace() {
            p += 1;
        }
        let value = chars[s..p].iter().collect::<String>().replace('_', " ").trim().to_string();
        match what {
            0 => m.subject = Some(value),
            1 => m.topic = Some(value),
            2 => m.subtopic = Some(value),
            3 => m.points = value.parse().ok(),
            4 => m.lines = value.parse().ok().map(|n| (n, false)),
            _ => m.lines = value.parse().ok().map(|n| (n, true)),
        }
    }
    Some(m)
}

/// Points in brackets: "(2)", "(2 P.)", "(3 Punkte)", "(4p)"; only whole numbers.
pub fn bracket_points(text: &str) -> Option<u32> {
    let c: Vec<char> = text.chars().collect();
    let mut sum = 0;
    let mut any = false;
    let mut k = 0;
    while k < c.len() {
        if c[k] == '(' && (k == 0 || c[k - 1].is_whitespace() || c[k - 1] == '}') {
            let mut p = k + 1;
            while p < c.len() && c[p] == ' ' {
                p += 1;
            }
            let s = p;
            while p < c.len() && c[p].is_ascii_digit() {
                p += 1;
            }
            if p > s {
                let n: u32 = c[s..p].iter().collect::<String>().parse().unwrap_or(0);
                let mut q = p;
                while q < c.len() && c[q] == ' ' {
                    q += 1;
                }
                let ok = if c.get(q) == Some(&')') {
                    true
                } else if matches!(c.get(q), Some('P') | Some('p')) {
                    // "(2 P.)", "(2 Punkte)": the rest up to the closing bracket
                    let close = c[q..].iter().position(|x| *x == ')').map(|o| q + o);
                    close.map(|e| !c[q..e].contains(&'(') && !c[q..e].iter().any(|x| x.is_ascii_digit())).unwrap_or(false)
                } else {
                    false
                };
                if ok {
                    sum += n;
                    any = true;
                }
            }
        }
        k += 1;
    }
    any.then_some(sum)
}

/// Settings of the import dialog.
#[derive(Clone, Debug)]
pub struct Options {
    /// fill in when the marker has no subject / topic
    pub subject: String,
    pub topic: String,
    /// blank lines at the end of a task become answer lines (a single one counts 0)
    pub count_blank: bool,
    pub underline: bool,
    /// add up points in brackets in the solution
    pub points_from_solution: bool,
    /// warn about tasks without solution
    pub need_solution: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { subject: String::new(), topic: String::new(), count_blank: true, underline: false, points_from_solution: true, need_solution: false }
    }
}

/// A task found in the document.
#[derive(Clone, Debug)]
pub struct Found {
    pub nr: u32,
    pub task: crate::model::Task,
}

/// Cuts the document into tasks. Returns the tasks and remarks for the user.
pub fn tasks(doc: &Parsed, opt: &Options) -> (Vec<Found>, Vec<String>) {
    struct Entry {
        marker: Option<Marker>,
        task: Vec<Para>,
        sol: Vec<Para>,
        has_task: bool,
        has_sol: bool,
    }
    let mut notes = doc.notes.clone();
    let mut entries: Vec<(u32, Entry)> = Vec::new();
    let any_marker = doc.paras.iter().any(|p| !p.table && marker(&p.plain).is_some());
    let mut target: Option<(usize, bool)> = None;
    let mut before = 0;
    if !any_marker {
        entries.push((1, Entry { marker: None, task: doc.paras.clone(), sol: vec![], has_task: true, has_sol: false }));
        notes.push("Keine Markierungen (Aufgabe=1 / A#1 …) gefunden: das ganze Dokument wird eine Aufgabe.".into());
    } else {
        for p in &doc.paras {
            if let Some(m) = (!p.table).then(|| marker(&p.plain)).flatten() {
                let idx = match entries.iter().position(|(nr, e)| *nr == m.nr && (if m.solution { !e.has_sol } else { !e.has_task })) {
                    Some(i) => i,
                    None => {
                        if entries.iter().any(|(nr, e)| *nr == m.nr && if m.solution { e.has_sol } else { e.has_task }) {
                            notes.push(format!("{} {} kommt mehrfach vor.", if m.solution { "Lösung" } else { "Aufgabe" }, m.nr));
                        }
                        entries.push((m.nr, Entry { marker: None, task: vec![], sol: vec![], has_task: false, has_sol: false }));
                        entries.len() - 1
                    }
                };
                let e = &mut entries[idx].1;
                let rest = Para { markup: crate::rvf::escape(&m.rest), plain: m.rest.clone(), table: false };
                if m.solution {
                    e.has_sol = true;
                    if !m.rest.is_empty() {
                        e.sol.push(rest);
                    }
                } else {
                    e.has_task = true;
                    if !m.rest.is_empty() {
                        e.task.push(rest);
                    }
                    e.marker = Some(m.clone());
                }
                target = Some((idx, m.solution));
                continue;
            }
            match target {
                Some((i, true)) => entries[i].1.sol.push(p.clone()),
                Some((i, false)) => entries[i].1.task.push(p.clone()),
                None => {
                    if !p.is_empty() {
                        before += 1;
                    }
                }
            }
        }
    }
    if before > 0 {
        notes.push(format!("{before} Absätze vor der ersten Markierung werden nicht übernommen."));
    }
    let mut out = Vec::new();
    for (nr, e) in entries {
        if !e.has_task {
            notes.push(format!("Lösung {nr} hat keine Aufgabe und wird nicht übernommen."));
            continue;
        }
        if opt.need_solution && !e.has_sol {
            notes.push(format!("Aufgabe {nr} hat keine Lösung."));
        }
        let mut task_paras = e.task;
        while task_paras.first().map(|p| p.is_empty()).unwrap_or(false) {
            task_paras.remove(0);
        }
        let mut blank = 0u32;
        while task_paras.last().map(|p| p.is_empty()).unwrap_or(false) {
            task_paras.pop();
            blank += 1;
        }
        let mut sol_paras = e.sol;
        while sol_paras.first().map(|p| p.is_empty()).unwrap_or(false) {
            sol_paras.remove(0);
        }
        while sol_paras.last().map(|p| p.is_empty()).unwrap_or(false) {
            sol_paras.pop();
        }
        let m = e.marker.unwrap_or_default();
        let mut t = crate::model::Task {
            subject: m.subject.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| opt.subject.trim().to_string()),
            topic: m.topic.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| opt.topic.trim().to_string()),
            subtopic: m.subtopic.clone().unwrap_or_default(),
            created: Some(chrono::Local::now().naive_local()),
            ..Default::default()
        };
        if !opt.count_blank {
            // keep the blank lines in the text
            for _ in 0..blank {
                task_paras.push(Para::default());
            }
            blank = 0;
        }
        t.answer_lines = if blank <= 1 { 0 } else { blank };
        t.underline_answer = opt.underline && t.answer_lines > 0;
        if let Some((n, u)) = m.lines {
            t.answer_lines = n;
            t.underline_answer = u;
        }
        t.task = doc_of(&task_paras, &doc.objects);
        t.solution = doc_of(&sol_paras, &doc.objects);
        // points: marker, otherwise Teilaufgaben in the task, otherwise brackets in the solution
        let parts = crate::model::parts_from_text(&t.task.text);
        if let Some(p) = m.points {
            t.points = p as i32;
        } else if parts.len() > 1 {
            t.parts = parts;
            t.sync_points();
        } else if opt.points_from_solution {
            let plain: String = sol_paras.iter().map(|p| p.plain.as_str()).collect::<Vec<_>>().join("\n");
            if let Some(p) = bracket_points(&plain) {
                t.points = p as i32;
            }
        }
        if t.subtopic.is_empty() {
            // a short title from the first words
            let first = task_paras.iter().map(|p| p.plain.trim()).find(|p| !p.is_empty()).unwrap_or("");
            let words: Vec<&str> = first.split_whitespace().take(6).collect();
            let mut title = words.join(" ");
            if first.split_whitespace().count() > 6 {
                title += " …";
            }
            t.subtopic = title.trim().trim_end_matches([':', '.', ',']).trim_end().to_string();
        }
        if t.task.text.trim().is_empty() {
            notes.push(format!("Aufgabe {nr} ist leer und wird nicht übernommen."));
            continue;
        }
        out.push(Found { nr, task: t });
    }
    (out, notes)
}

/// Markup and the objects it uses, numbered from 1.
fn doc_of(paras: &[Para], objects: &[DocObject]) -> crate::model::Doc {
    let mut text = paras.iter().map(|p| p.markup.as_str()).collect::<Vec<_>>().join("\n");
    let mut objs = Vec::new();
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(s) = rest.find("{{") {
        out += &rest[..s];
        let after = &rest[s + 2..];
        match after.find("}}").and_then(|e| after[..e].parse::<u32>().ok().map(|n| (e, n))) {
            Some((e, n)) => {
                let new = match map.get(&n) {
                    Some(x) => *x,
                    None => {
                        let id = map.len() as u32 + 1;
                        map.insert(n, id);
                        if let Some(o) = objects.iter().find(|o| o.id == n) {
                            objs.push(DocObject { id, ..o.clone() });
                        }
                        id
                    }
                };
                out += &format!("{{{{{new}}}}}");
                rest = &after[e + 2..];
            }
            None => {
                out += "{{";
                rest = after;
            }
        }
    }
    out += rest;
    text = out;
    crate::model::Doc { text, objects: objs }
}

/// Reads the bytes of a .docx or .rtf file.
pub fn parse_bytes(data: &[u8], ext: &str) -> Result<Parsed, String> {
    if data.starts_with(b"{\\rtf") {
        read_rtf(data)
    } else if data.starts_with(b"PK") {
        read_docx(data)
    } else if ext == "doc" || data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        Err("Alte Word-Dateien (.doc) bitte in Word als .docx oder .rtf speichern.".into())
    } else {
        Err("Unbekanntes Dateiformat – erwartet wird .docx oder .rtf.".into())
    }
}
