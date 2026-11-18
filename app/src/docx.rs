//! Word documents (.docx) for exams, solutions and grade overviews.
//! Written directly (Office Open XML in a ZIP file), no extra libraries.
//! New formulas become real Word formulas (editable in Word), formulas of the
//! old formula editor are embedded as the metafile pictures they are.

use crate::export::{answer_line_pt, Composed, PAGE_MARGINS_MM};
use crate::markup::{parse, Block, Inl};
use crate::model::{Database, Doc, DocObject, ObjKind};

// ------------------------------------------------------------------ zip (stored)

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

#[cfg(test)]
pub fn crc32_for_tests(d: &[u8]) -> u32 {
    crc32(d)
}

/// A ZIP archive without compression (enough for Word; pictures are compressed already).
pub fn zip_store(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    // DOS date 2026-01-01 00:00
    let (time, date) = (0u16, ((2026 - 1980) << 9 | 1 << 5 | 1) as u16);
    for (name, data) in files {
        let crc = crc32(data);
        let off = out.len() as u32;
        let n = name.as_bytes();
        out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
        out.extend_from_slice(&0u16.to_le_bytes()); // stored
        out.extend_from_slice(&time.to_le_bytes());
        out.extend_from_slice(&date.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // made by
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&time.to_le_bytes());
        central.extend_from_slice(&date.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 8]); // extra, comment, disk, internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(n);
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

// ------------------------------------------------------------------ document

fn x(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // characters XML 1.0 does not allow
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => o.push(c),
        }
    }
    o
}

const NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" \
xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"";

/// twips of the text width (A4 minus the left and right margins)
fn text_twips() -> i32 {
    ((210.0 - PAGE_MARGINS_MM[3] - PAGE_MARGINS_MM[1]) * 1440.0 / 25.4).round() as i32
}

fn mm_twips(mm: f32) -> i32 {
    (mm * 1440.0 / 25.4).round() as i32
}

struct Media {
    name: String,
    content_type: &'static str,
    data: Vec<u8>,
}

struct Writer<'a> {
    db: &'a Database,
    size2: i32,
    /// own font of the current task (rFonts on every run)
    font: Option<String>,
    /// own size of the current task (sz on every run)
    run_size: Option<i32>,
    media: Vec<Media>,
    /// relationship targets: (id, type, target, external)
    rels: Vec<(String, &'static str, String, bool)>,
    next_pic: u32,
}

const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_LINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

/// Adds the placeable header a stand-alone metafile needs (old formulas are stored without it).
fn placeable_wmf(data: &[u8], w_px: f32) -> Vec<u8> {
    if data.len() > 4 && data[0..4] == [0xD7, 0xCD, 0xC6, 0x9A] {
        return data.to_vec();
    }
    let Some(d) = crate::formula::parse_wmf(data) else { return data.to_vec() };
    let (l, t) = (d.org.x.round() as i16, d.org.y.round() as i16);
    let (r, b) = ((d.org.x + d.ext.x).round() as i16, (d.org.y + d.ext.y).round() as i16);
    let inch = if w_px > 0.0 { (d.ext.x.abs() / (w_px / 96.0)).round().clamp(1.0, 30000.0) as u16 } else { 1440 };
    let hdr: [u16; 10] = [0xCDD7, 0x9AC6, 0, l as u16, t as u16, r as u16, b as u16, inch, 0, 0];
    let sum = hdr.iter().fold(0u16, |a, v| a ^ v);
    let mut out = Vec::with_capacity(data.len() + 22);
    for v in hdr {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&sum.to_le_bytes());
    out.extend_from_slice(data);
    out
}

impl Writer<'_> {
    fn rel(&mut self, kind: &'static str, target: String, external: bool) -> String {
        let id = format!("rId{}", self.rels.len() + 10);
        self.rels.push((id.clone(), kind, target, external));
        id
    }

    fn run_props(&self, b: bool, i: bool, u: bool, sup: bool, sub: bool, color: Option<[u8; 3]>) -> String {
        let mut p = String::new();
        if let Some(f) = &self.font {
            let f = x(f);
            p += &format!("<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\"/>");
        }
        if b {
            p += "<w:b/>";
        }
        if i {
            p += "<w:i/>";
        }
        if let Some(c) = color {
            p += &format!("<w:color w:val=\"{:02X}{:02X}{:02X}\"/>", c[0], c[1], c[2]);
        }
        if let Some(sz) = self.run_size {
            p += &format!("<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>");
        }
        if u {
            p += "<w:u w:val=\"single\"/>";
        }
        if sup {
            p += "<w:vertAlign w:val=\"superscript\"/>";
        } else if sub {
            p += "<w:vertAlign w:val=\"subscript\"/>";
        }
        if p.is_empty() {
            p
        } else {
            format!("<w:rPr>{p}</w:rPr>")
        }
    }

    fn text_run(&self, t: &str, props: &str) -> String {
        let mut s = String::new();
        for (k, part) in t.split('\t').enumerate() {
            if k > 0 {
                s += &format!("<w:r>{props}<w:tab/></w:r>");
            }
            if !part.is_empty() {
                s += &format!("<w:r>{props}<w:t xml:space=\"preserve\">{}</w:t></w:r>", x(part));
            }
        }
        s
    }

    fn picture(&mut self, data: Vec<u8>, ext: &str, ct: &'static str, w_px: f32, h_px: f32, max_twips: i32) -> String {
        self.next_pic += 1;
        let n = self.next_pic;
        let name = format!("image{n}.{ext}");
        self.media.push(Media { name: name.clone(), content_type: ct, data });
        let rid = self.rel(REL_IMAGE, format!("media/{name}"), false);
        let max_emu = max_twips as f64 * 635.0;
        let (mut cx, mut cy) = (w_px as f64 * 9525.0, h_px as f64 * 9525.0);
        if cx > max_emu {
            cy *= max_emu / cx;
            cx = max_emu;
        }
        let (cx, cy) = (cx.round().max(1.0) as i64, cy.round().max(1.0) as i64);
        format!(
            "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
<wp:docPr id=\"{n}\" name=\"Bild {n}\"/><wp:cNvGraphicFramePr/>\
<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic>\
<pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"{name}\"/><pic:cNvPicPr/></pic:nvPicPr>\
<pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
<pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"
        )
    }

    fn object(&mut self, o: &DocObject, max_twips: i32) -> String {
        match o.kind {
            ObjKind::Math => crate::math::to_omml(&String::from_utf8_lossy(&o.data)),
            ObjKind::Formula => {
                let (mut w, mut h) = (o.width, o.height);
                if w <= 0.0 || h <= 0.0 {
                    if let Some(d) = crate::formula::parse_wmf(&o.data) {
                        w = d.ext.x.abs() / 15.0;
                        h = d.ext.y.abs() / 15.0;
                    }
                }
                let data = placeable_wmf(&o.data, w);
                self.picture(data, "wmf", "image/x-wmf", w, h, max_twips)
            }
            ObjKind::Image => {
                let (data, ext, ct) = if o.data.starts_with(b"\x89PNG") {
                    (o.data.clone(), "png", "image/png")
                } else if o.data.starts_with(b"\xFF\xD8") {
                    (o.data.clone(), "jpeg", "image/jpeg")
                } else if o.data.starts_with(b"GIF8") {
                    (o.data.clone(), "gif", "image/gif")
                } else {
                    match image::load_from_memory(&o.data) {
                        Ok(img) => {
                            let mut v = Vec::new();
                            if img.write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png).is_err() {
                                return String::new();
                            }
                            (v, "png", "image/png")
                        }
                        Err(_) => return String::new(),
                    }
                };
                let (mut w, mut h) = (o.width, o.height);
                if w <= 0.0 || h <= 0.0 {
                    if let Ok(i) = image::load_from_memory(&data) {
                        w = i.width() as f32;
                        h = i.height() as f32;
                    }
                }
                self.picture(data, ext, ct, w, h, max_twips)
            }
        }
    }

    fn inlines(&mut self, doc: &Doc, inl: &[Inl], max_twips: i32, bold_all: bool) -> String {
        let mut s = String::new();
        for it in inl {
            match it {
                Inl::Text { s: t, b, i, u, sup, sub, color } => {
                    let p = self.run_props(*b || bold_all, *i, *u, *sup, *sub, *color);
                    s += &self.text_run(t, &p);
                }
                Inl::Check(on) => {
                    let (c, col) = if *on { ("☒", "<w:color w:val=\"C00000\"/>") } else { ("☐", "") };
                    s += &format!("<w:r><w:rPr><w:rFonts w:ascii=\"Segoe UI Symbol\" w:hAnsi=\"Segoe UI Symbol\"/>{col}</w:rPr><w:t xml:space=\"preserve\">{c} </w:t></w:r>");
                }
                Inl::Obj(id) => {
                    if let Some(o) = doc.object(*id) {
                        s += &self.object(o, max_twips);
                    }
                }
                Inl::Link { text, url } => {
                    let full = if url.starts_with("www.") { format!("https://{url}") } else { url.clone() };
                    let rid = self.rel(REL_LINK, x(&full), true);
                    s += &format!(
                        "<w:hyperlink r:id=\"{rid}\"><w:r><w:rPr><w:color w:val=\"2850C8\"/><w:u w:val=\"single\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:hyperlink>",
                        x(text)
                    );
                }
            }
        }
        s
    }

    fn answer_line(&self, underline: bool) -> String {
        // fixed line distance (export::ANSWER_LINE_MM); underlined with a tab leader across the text width
        let twips = (answer_line_pt() * 20.0).round() as i32;
        if underline {
            format!(
                "<w:p><w:pPr><w:tabs><w:tab w:val=\"right\" w:leader=\"underscore\" w:pos=\"{}\"/></w:tabs><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{twips}\" w:lineRule=\"exact\"/></w:pPr><w:r><w:tab/></w:r></w:p>",
                text_twips()
            )
        } else {
            format!("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"{twips}\" w:lineRule=\"exact\"/></w:pPr></w:p>")
        }
    }

    fn doc(&mut self, doc: &Doc, underline: bool) -> String {
        let mut s = String::new();
        let tw = text_twips();
        for b in parse(&doc.text) {
            match b {
                Block::Empty => s += "<w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr></w:p>",
                Block::Rule => {
                    s += "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"808080\"/></w:pBdr><w:spacing w:after=\"120\"/></w:pPr></w:p>"
                }
                Block::Lines(n) => {
                    for _ in 0..n {
                        s += &self.answer_line(underline);
                    }
                }
                Block::Para { inl, bullet, heading } => {
                    if heading {
                        let sz = self.size2 + 8;
                        let keep = self.run_size.replace(sz);
                        let body = self.inlines(doc, &inl, tw, true);
                        self.run_size = keep;
                        s += &format!("<w:p><w:pPr><w:keepNext/><w:spacing w:before=\"60\" w:after=\"100\"/><w:rPr><w:sz w:val=\"{sz}\"/></w:rPr></w:pPr>{}</w:p>", with_size(&body, sz));
                    } else if bullet {
                        let body = self.inlines(doc, &inl, tw - 360, false);
                        s += &format!(
                            "<w:p><w:pPr><w:tabs><w:tab w:val=\"left\" w:pos=\"360\"/></w:tabs><w:spacing w:after=\"40\"/><w:ind w:left=\"360\" w:hanging=\"240\"/></w:pPr><w:r><w:t>•</w:t></w:r><w:r><w:tab/></w:r>{body}</w:p>"
                        );
                    } else {
                        let body = self.inlines(doc, &inl, tw, false);
                        s += &format!("<w:p><w:pPr><w:spacing w:after=\"40\"/></w:pPr>{body}</w:p>");
                    }
                }
                Block::Table { bordered, rows, cells } => {
                    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                    let mut weight = vec![2.0f32; cols];
                    let mut short = vec![0.0f32; cols];
                    for (ri, r) in rows.iter().enumerate() {
                        for (ci, cell) in r.iter().enumerate() {
                            if cells[ri][ci].colspan != 1 {
                                continue;
                            }
                            let l: f32 = cell
                                .iter()
                                .map(|line| {
                                    line.iter()
                                        .map(|x| match x {
                                            Inl::Text { s, .. } => s.chars().count() as f32,
                                            Inl::Obj(id) => doc.object(*id).map(|o| o.width / 7.0).unwrap_or(10.0),
                                            _ => 2.0,
                                        })
                                        .sum::<f32>()
                                })
                                .fold(0.0, f32::max);
                            weight[ci] = weight[ci].max(l.min(70.0));
                            if l <= 14.0 {
                                short[ci] = short[ci].max(l);
                            }
                        }
                    }
                    // twips per character: about 0.55 of the font size
                    let per_char = self.size2 as f32 * 5.5;
                    let widths: Vec<i32> = crate::markup::column_widths(&weight, &short, tw as f32, per_char, 220.0).into_iter().map(|w| w.round() as i32).collect();
                    let borders = if bordered {
                        "<w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:color=\"555555\"/><w:left w:val=\"single\" w:sz=\"4\" w:color=\"555555\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"555555\"/><w:right w:val=\"single\" w:sz=\"4\" w:color=\"555555\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:color=\"555555\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:color=\"555555\"/></w:tblBorders>"
                    } else {
                        ""
                    };
                    s += &format!(
                        "<w:tbl><w:tblPr><w:tblW w:w=\"{tw}\" w:type=\"dxa\"/>{borders}<w:tblLayout w:type=\"fixed\"/><w:tblCellMar><w:left w:w=\"70\" w:type=\"dxa\"/><w:right w:w=\"70\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>"
                    );
                    for w in &widths {
                        s += &format!("<w:gridCol w:w=\"{w}\"/>");
                    }
                    s += "</w:tblGrid>";
                    let edge = |v: &str| format!("<w:tcBorders><w:top w:val=\"{v}\" w:sz=\"4\" w:color=\"555555\"/><w:left w:val=\"{v}\" w:sz=\"4\" w:color=\"555555\"/><w:bottom w:val=\"{v}\" w:sz=\"4\" w:color=\"555555\"/><w:right w:val=\"{v}\" w:sz=\"4\" w:color=\"555555\"/></w:tcBorders>");
                    for (ri, r) in rows.iter().enumerate() {
                        s += "<w:tr>";
                        let mut ci = 0;
                        while ci < cols {
                            let inf = cells[ri][ci];
                            // (column with text or None for the lower part of a merged cell, columns, frame, vMerge)
                            let (start, span, border, vmerge) = if inf.covered() {
                                let (ar, ac) = crate::table::anchor_of(&cells, ri, ci);
                                if ar < ri && ac == ci {
                                    (None, cells[ar][ac].colspan, cells[ar][ac].border, "<w:vMerge/>")
                                } else {
                                    ci += 1;
                                    continue;
                                }
                            } else {
                                (Some(ci), inf.colspan, inf.border, if inf.rowspan > 1 { "<w:vMerge w:val=\"restart\"/>" } else { "" })
                            };
                            let span = span.clamp(1, cols - ci);
                            let w: i32 = widths[ci..ci + span].iter().sum();
                            let grid = if span > 1 { format!("<w:gridSpan w:val=\"{span}\"/>") } else { String::new() };
                            let borders = if border == bordered {
                                String::new()
                            } else if border {
                                edge("single")
                            } else {
                                edge("nil")
                            };
                            s += &format!("<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/>{grid}{vmerge}{borders}</w:tcPr><w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr>");
                            if let Some(cell) = start.and_then(|c| r.get(c)) {
                                for (li, line) in cell.iter().enumerate() {
                                    if li > 0 {
                                        s += "<w:r><w:br/></w:r>";
                                    }
                                    s += &self.inlines(doc, line, w - 140, false);
                                }
                            }
                            s += "</w:p></w:tc>";
                            ci += span;
                        }
                        s += "</w:tr>";
                    }
                    s += "</w:tbl><w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr></w:p>";
                }
            }
        }
        s
    }
}

/// Gives every run of a heading that has no size yet the heading size
/// (in the order Word expects: after rFonts/b/i/color, before u/vertAlign).
fn with_size(runs: &str, sz: i32) -> String {
    let tag = format!("<w:sz w:val=\"{sz}\"/>");
    let mut out = String::new();
    for (k, part) in runs.split("<w:r>").enumerate() {
        if k > 0 {
            out += "<w:r>";
        }
        if k == 0 || part.starts_with("<w:rPr><w:sz ") {
            out += part;
        } else if let Some(rest) = part.strip_prefix("<w:rPr>") {
            let end = rest.find("</w:rPr>").unwrap_or(0);
            let (props, tail) = rest.split_at(end);
            if props.contains("<w:sz ") {
                out += part;
                continue;
            }
            let at = ["<w:u ", "<w:vertAlign"].iter().filter_map(|t| props.find(t)).min().unwrap_or(props.len());
            out += &format!("<w:rPr>{}{tag}{}{tail}", &props[..at], &props[at..]);
        } else if part.starts_with("<w:t") {
            out += &format!("<w:rPr>{tag}</w:rPr>{part}");
        } else {
            out += part;
        }
    }
    out
}

pub fn to_docx(db: &Database, c: &Composed, underline: bool) -> Vec<u8> {
    let size2 = (db.settings.print_size * 2.0).round() as i32;
    let mut w = Writer { db, size2, font: None, run_size: None, media: Vec::new(), rels: Vec::new(), next_pic: 0 };
    let tw = text_twips();
    let mut body = String::new();
    for sec in &c.sections {
        if let Some((h, p)) = &sec.heading {
            body += &format!(
                "<w:p><w:pPr><w:keepNext/><w:pBdr><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"999999\"/></w:pBdr><w:tabs><w:tab w:val=\"right\" w:pos=\"{tw}\"/></w:tabs><w:spacing w:before=\"240\" w:after=\"100\"/></w:pPr>\
<w:r><w:rPr><w:b/><w:sz w:val=\"{}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:rPr><w:sz w:val=\"{}\"/></w:rPr><w:tab/><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                size2 + 2,
                x(&crate::markup::plain(h)),
                size2 + 2,
                x(p)
            );
        }
        let own = sec.own_font(db);
        if let Some((f, sz)) = &own {
            w.size2 = (sz * 2.0).round() as i32;
            w.run_size = (w.size2 != size2).then_some(w.size2);
            w.font = (*f != db.settings.print_font).then(|| f.clone());
        }
        body += &w.doc(&sec.doc, underline);
        for _ in 0..sec.lines {
            body += &w.answer_line(underline || sec.underline);
        }
        if own.is_some() {
            (w.size2, w.run_size, w.font) = (size2, None, None);
        }
    }
    let footer_rid = if c.page_numbers || !c.footer.is_empty() {
        Some(w.rel("http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer", "footer1.xml".into(), false))
    } else {
        None
    };
    let sect = format!(
        "<w:sectPr>{}<w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"340\" w:footer=\"340\" w:gutter=\"0\"/></w:sectPr>",
        footer_rid.as_ref().map(|r| format!("<w:footerReference w:type=\"default\" r:id=\"{r}\"/>")).unwrap_or_default(),
        mm_twips(PAGE_MARGINS_MM[0]),
        mm_twips(PAGE_MARGINS_MM[1]),
        mm_twips(PAGE_MARGINS_MM[2]),
        mm_twips(PAGE_MARGINS_MM[3]),
    );
    let document = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document {NS}><w:body>{body}{sect}</w:body></w:document>");

    let mut footer = String::new();
    if footer_rid.is_some() {
        let mut runs = format!("<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", x(&c.footer));
        if c.page_numbers {
            if !c.footer.is_empty() {
                runs += "<w:r><w:t xml:space=\"preserve\">   ·   </w:t></w:r>";
            }
            runs += "<w:r><w:t xml:space=\"preserve\">Seite </w:t></w:r><w:fldSimple w:instr=\"PAGE\"><w:r><w:t>1</w:t></w:r></w:fldSimple><w:r><w:t xml:space=\"preserve\"> von </w:t></w:r><w:fldSimple w:instr=\"NUMPAGES\"><w:r><w:t>1</w:t></w:r></w:fldSimple>";
        }
        footer = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:ftr {NS}><w:p><w:pPr><w:jc w:val=\"center\"/><w:rPr><w:sz w:val=\"16\"/><w:color w:val=\"777777\"/></w:rPr></w:pPr>{}</w:p></w:ftr>",
            runs.replace("<w:r><w:t", "<w:r><w:rPr><w:sz w:val=\"16\"/><w:color w:val=\"777777\"/></w:rPr><w:t")
        );
    }

    let font = x(&db.settings.print_font);
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\" w:cs=\"{font}\" w:eastAsia=\"{font}\"/><w:sz w:val=\"{size2}\"/><w:szCs w:val=\"{size2}\"/><w:lang w:val=\"de-DE\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"40\" w:line=\"264\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Standard\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"table\" w:default=\"1\" w:styleId=\"NormaleTabelle\"><w:name w:val=\"Normal Table\"/><w:tblPr><w:tblCellMar><w:left w:w=\"70\" w:type=\"dxa\"/><w:right w:w=\"70\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr></w:style>\
</w:styles>"
    );
    let settings = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><w:defaultTabStop w:val=\"708\"/><w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat><m:mathPr><m:mathFont m:val=\"Cambria Math\"/><m:dispDef/><m:defJc m:val=\"left\"/></m:mathPr></w:settings>";

    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings\" Target=\"settings.xml\"/>",
    );
    for (id, kind, target, external) in &w.rels {
        rels += &format!("<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{target}\"{}/>", if *external { " TargetMode=\"External\"" } else { "" });
    }
    rels += "</Relationships>";

    let mut types = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>",
    );
    let mut seen: Vec<String> = Vec::new();
    for m in &w.media {
        let ext = m.name.rsplit('.').next().unwrap_or("").to_string();
        if !seen.contains(&ext) {
            types += &format!("<Default Extension=\"{ext}\" ContentType=\"{}\"/>", m.content_type);
            seen.push(ext);
        }
    }
    types += "<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>";
    if footer_rid.is_some() {
        types += "<Override PartName=\"/word/footer1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/>";
    }
    types += "</Types>";

    let root_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>";
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%SZ");
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title><dc:creator>{}</dc:creator><dcterms:created xsi:type=\"dcterms:W3CDTF\">{now}</dcterms:created></cp:coreProperties>",
        x(&c.title),
        x(&w.db.settings.teacher)
    );

    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), types.into_bytes()),
        ("_rels/.rels".into(), root_rels.as_bytes().to_vec()),
        ("docProps/core.xml".into(), core.into_bytes()),
        ("word/document.xml".into(), document.into_bytes()),
        ("word/styles.xml".into(), styles.into_bytes()),
        ("word/settings.xml".into(), settings.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels".into(), rels.into_bytes()),
    ];
    if footer_rid.is_some() {
        files.push(("word/footer1.xml".into(), footer.into_bytes()));
    }
    for m in w.media {
        files.push((format!("word/media/{}", m.name), m.data));
    }
    zip_store(&files)
}
