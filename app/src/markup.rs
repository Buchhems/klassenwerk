//! Parses the light markup of a task and shows it on screen or as HTML.

use crate::formula;
use crate::model::{Doc, DocObject, ObjKind};
use crate::table::CellInfo;
use base64::Engine;
use egui::{text::LayoutJob, Align, Color32, FontId, Stroke, TextFormat, Vec2};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Inl {
    Text { s: String, b: bool, i: bool, u: bool, sup: bool, sub: bool, color: Option<[u8; 3]> },
    Check(bool),
    Obj(u32),
    Link { text: String, url: String },
}

#[derive(Clone, Debug)]
pub enum Block {
    Para {
        inl: Vec<Inl>,
        bullet: bool,
        heading: bool,
    },
    Empty,
    Rule,
    /// blank lines for handwritten answers
    Lines(u32),
    Table {
        bordered: bool,
        rows: Vec<Vec<Vec<Vec<Inl>>>>,
        /// merged cells and frames, one entry per cell (see table.rs)
        cells: Vec<Vec<CellInfo>>,
    },
}

/// Text colours that can be written as `{blau:text}`.
pub const COLORS: [(&str, [u8; 3]); 7] = [
    ("blau", [0, 0, 255]),
    ("rot", [210, 0, 0]),
    ("gruen", [0, 128, 0]),
    ("grün", [0, 128, 0]),
    ("orange", [230, 120, 0]),
    ("lila", [128, 0, 128]),
    ("grau", [128, 128, 128]),
];

pub fn color_name(c: [u8; 3]) -> &'static str {
    COLORS.iter().find(|(_, v)| *v == c).map(|(n, _)| *n).unwrap_or("blau")
}

#[derive(Clone, Copy, PartialEq)]
enum Span {
    Sup,
    Sub,
    Color([u8; 3]),
}

pub fn parse_inline(s: &str) -> Vec<Inl> {
    let c: Vec<char> = s.chars().collect();
    let mut out: Vec<Inl> = Vec::new();
    let (mut b, mut it, mut u) = (false, false, false);
    let mut stack: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut Vec<Inl>, b: bool, it: bool, u: bool, stack: &Vec<Span>| {
        if !buf.is_empty() {
            let sup = stack.contains(&Span::Sup);
            let sub = stack.contains(&Span::Sub);
            let color = stack.iter().rev().find_map(|x| if let Span::Color(c) = x { Some(*c) } else { None });
            out.push(Inl::Text { s: std::mem::take(buf), b, i: it, u, sup, sub, color });
        }
    };
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let n1 = c.get(i + 1).copied();
        let n2 = c.get(i + 2).copied();
        if let Some(n) = n1.filter(|_| ch == '\\') {
            buf.push(n);
            i += 2;
            continue;
        }
        if ch == '*' && n1 == Some('*') {
            flush(&mut buf, &mut out, b, it, u, &stack);
            b = !b;
            i += 2;
            continue;
        }
        if ch == '*' {
            flush(&mut buf, &mut out, b, it, u, &stack);
            it = !it;
            i += 1;
            continue;
        }
        if ch == '+' && n1 == Some('+') {
            flush(&mut buf, &mut out, b, it, u, &stack);
            u = !u;
            i += 2;
            continue;
        }
        if (ch == '^' || ch == '_') && n1 == Some('{') {
            flush(&mut buf, &mut out, b, it, u, &stack);
            stack.push(if ch == '^' { Span::Sup } else { Span::Sub });
            i += 2;
            continue;
        }
        if ch == '{' && n1 != Some('{') {
            // {blau:text}
            let rest: String = c[i + 1..].iter().take(8).collect();
            if let Some((name, col)) = COLORS.iter().find(|(n, _)| rest.starts_with(&format!("{n}:"))) {
                flush(&mut buf, &mut out, b, it, u, &stack);
                stack.push(Span::Color(*col));
                i += 1 + name.chars().count() + 1;
                continue;
            }
        }
        if ch == '}' && !stack.is_empty() {
            flush(&mut buf, &mut out, b, it, u, &stack);
            stack.pop();
            i += 1;
            continue;
        }
        if ch == '[' && n2 == Some(']') && matches!(n1, Some(' ') | Some('x') | Some('X')) {
            flush(&mut buf, &mut out, b, it, u, &stack);
            out.push(Inl::Check(n1 != Some(' ')));
            i += 3;
            continue;
        }
        if ch == '[' {
            // [text](url)
            let rest: String = c[i + 1..].iter().collect();
            if let Some(close) = rest.find("](") {
                if let Some(end) = rest[close + 2..].find(')') {
                    let text = &rest[..close];
                    let url = &rest[close + 2..close + 2 + end];
                    if !text.contains('[') && !url.contains(' ') && (url.starts_with("http") || url.starts_with("mailto:") || url.starts_with("www.")) {
                        flush(&mut buf, &mut out, b, it, u, &stack);
                        out.push(Inl::Link { text: text.to_string(), url: url.to_string() });
                        i += 1 + text.chars().count() + 2 + url.chars().count() + 1;
                        continue;
                    }
                }
            }
        }
        if ch == '{' && n1 == Some('{') {
            let rest: String = c[i + 2..].iter().take(12).collect();
            if let Some(end) = rest.find("}}") {
                if let Ok(id) = rest[..end].trim().parse::<u32>() {
                    flush(&mut buf, &mut out, b, it, u, &stack);
                    out.push(Inl::Obj(id));
                    i += 2 + rest[..end].chars().count() + 2;
                    continue;
                }
            }
        }
        buf.push(ch);
        i += 1;
    }
    flush(&mut buf, &mut out, b, it, u, &stack);
    out
}

fn split_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for ch in line.chars() {
        if esc {
            cur.push('\\');
            cur.push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '|' {
            cells.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        cells.push(cur);
    }
    cells
}

type TableRow = (bool, Vec<Vec<Vec<Inl>>>, Vec<CellInfo>);

fn table_row(line: &str) -> Option<TableRow> {
    let t = line.trim_start();
    let (bordered, rest) = match t.strip_prefix("+|") {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('|')?),
    };
    let mut cells = Vec::new();
    let mut info = Vec::new();
    for c in split_cells(rest) {
        let (text, mark, toggle) = crate::table::cell_marks(&c);
        cells.push(text.split("<br>").map(|l| parse_inline(l.trim())).collect());
        info.push(CellInfo { mark, toggle, ..Default::default() });
    }
    Some((bordered, cells, info))
}

/// Widths of table columns: by the length of their text, but a column with only
/// short entries ("2 P.", "a)") is at least as wide as they are, so they do not wrap.
/// `short` = longest short entry (chars) per column, `per_char` / `pad` in the unit of `avail`.
pub fn column_widths(weight: &[f32], short: &[f32], avail: f32, per_char: f32, pad: f32) -> Vec<f32> {
    let total: f32 = weight.iter().sum::<f32>().max(1.0);
    let need: Vec<f32> = short.iter().map(|s| if *s > 0.0 { s * per_char + pad } else { 0.0 }).collect();
    let mut w: Vec<f32> = weight.iter().map(|x| avail * x / total).collect();
    for (x, n) in w.iter_mut().zip(&need) {
        *x = x.max(*n);
    }
    // take the extra room from the columns that have more than they need
    let over = w.iter().sum::<f32>() - avail;
    if over > 0.0 {
        let spare: f32 = w.iter().zip(&need).map(|(x, n)| (x - n).max(0.0)).sum();
        if spare > 0.0 {
            for (x, n) in w.iter_mut().zip(&need) {
                let s = (*x - n).max(0.0);
                *x -= over.min(spare) * s / spare;
            }
        }
    }
    w
}

pub fn parse(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for line in text.lines() {
        if let Some((bordered, cells, info)) = table_row(line) {
            if let Some(Block::Table { bordered: tb, rows, cells: ci }) = out.last_mut() {
                if *tb == bordered {
                    rows.push(cells);
                    ci.push(info);
                    continue;
                }
            }
            out.push(Block::Table { bordered, rows: vec![cells], cells: vec![info] });
            continue;
        }
        let t = line.trim_end();
        if t.trim().is_empty() {
            out.push(Block::Empty);
        } else if t.trim() == "---" {
            out.push(Block::Rule);
        } else if let Some(n) = t.trim().strip_prefix("[[Linien:").and_then(|r| r.strip_suffix("]]")) {
            out.push(Block::Lines(n.trim().parse().unwrap_or(3)));
        } else if let Some(r) = t.strip_prefix("# ") {
            out.push(Block::Para { inl: parse_inline(r), bullet: false, heading: true });
        } else if let Some(r) = t.strip_prefix("• ").or_else(|| t.strip_prefix("- ")) {
            out.push(Block::Para { inl: parse_inline(r), bullet: true, heading: false });
        } else {
            out.push(Block::Para { inl: parse_inline(t), bullet: false, heading: false });
        }
    }
    for b in out.iter_mut() {
        if let Block::Table { bordered, rows, cells } = b {
            crate::table::layout(cells, *bordered);
            let cols = cells.first().map(|r| r.len()).unwrap_or(0);
            for r in rows.iter_mut() {
                r.resize(cols, Vec::new());
            }
        }
    }
    out
}

/// Plain text without markup (for lists and search previews)
pub fn plain(text: &str) -> String {
    let mut s = String::new();
    for b in parse(text) {
        let inl: Vec<Inl> = match b {
            Block::Para { inl, .. } => inl,
            // cells and lines of a table are separated by a space
            Block::Table { rows, .. } => rows
                .into_iter()
                .flatten()
                .flatten()
                .flat_map(|line| {
                    line.into_iter().chain(std::iter::once(Inl::Text { s: " ".into(), b: false, i: false, u: false, sup: false, sub: false, color: None }))
                })
                .collect(),
            _ => continue,
        };
        for i in inl {
            match i {
                Inl::Text { s: t, .. } => s.push_str(&t),
                Inl::Check(_) => s.push_str("☐ "),
                Inl::Obj(_) => s.push_str("▣ "),
                Inl::Link { text, .. } => s.push_str(&text),
            }
        }
        s.push(' ');
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Plain text of a document for previews: formulas as text ("CH₄"), pictures as [Bild].
/// With `vars`, the random numbers ({=…}) are filled in.
pub fn plain_doc(doc: &Doc, vars: Option<&HashMap<String, f64>>) -> String {
    let fill = |s: &str| match vars {
        Some(v) => crate::vary::apply(s, v),
        None => s.to_string(),
    };
    let mut text = String::new();
    let mut rest = fill(&doc.text);
    while let Some(p) = rest.find("{{") {
        text += &rest[..p];
        let after = rest[p + 2..].to_string();
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && after[digits.len()..].starts_with("}}") {
            let id: u32 = digits.parse().unwrap_or(0);
            text += &match doc.object(id) {
                Some(o) if o.kind == ObjKind::Math => {
                    // long formulas are shortened, so the preview still shows the text around them
                    let f = crate::math::to_text(&fill(&String::from_utf8_lossy(&o.data)));
                    let f = if f.chars().count() > 30 { format!("{}…", f.chars().take(29).collect::<String>().trim_end()) } else { f };
                    escape_markup(&f)
                }
                Some(o) if o.kind == ObjKind::Image => "[Bild]".to_string(),
                Some(_) => "[Formel]".to_string(),
                None => String::new(),
            };
            rest = after[digits.len() + 2..].to_string();
        } else {
            text += "{{";
            rest = after;
        }
    }
    text += &rest;
    plain(&text)
}

fn escape_markup(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if "*+^_{}[]|\\".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

// ---------------------------------------------------------------- objects

pub enum Loaded {
    Texture(egui::TextureHandle),
    Drawing(formula::Drawing),
    Failed,
}

#[derive(Default)]
pub struct ObjCache {
    map: HashMap<u64, Loaded>,
}

fn key(o: &DocObject) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    o.data.hash(&mut h);
    h.finish()
}

impl ObjCache {
    pub fn get(&mut self, ctx: &egui::Context, o: &DocObject) -> &Loaded {
        let k = key(o);
        self.map.entry(k).or_insert_with(|| match o.kind {
            ObjKind::Image => match image::load_from_memory(&o.data) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    Loaded::Texture(ctx.load_texture(format!("obj{k}"), ci, egui::TextureOptions::LINEAR))
                }
                Err(_) => Loaded::Failed,
            },
            ObjKind::Formula => match formula::parse_wmf(&o.data) {
                Some(d) => Loaded::Drawing(d),
                None => Loaded::Failed,
            },
            ObjKind::Math => Loaded::Drawing(crate::math::layout(&String::from_utf8_lossy(&o.data))),
        })
    }
}

// ---------------------------------------------------------------- egui

pub const INK: Color32 = Color32::from_rgb(28, 30, 36);
pub const PAPER: Color32 = Color32::WHITE;

pub struct View<'a> {
    pub doc: &'a Doc,
    pub cache: &'a mut ObjCache,
    pub size: f32,
    pub zoom: f32,
}

impl<'a> View<'a> {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let blocks = parse(&self.doc.text);
        ui.spacing_mut().item_spacing.y = 3.0;
        for b in &blocks {
            self.block(ui, b);
        }
    }

    fn block(&mut self, ui: &mut egui::Ui, b: &Block) {
        match b {
            Block::Empty => {
                ui.add_space(self.size * 0.55);
            }
            Block::Rule => {
                ui.add_space(4.0);
                let r = ui.available_rect_before_wrap();
                let y = r.top();
                ui.painter().hline(r.x_range(), y, Stroke::new(1.0_f32, Color32::from_gray(150)));
                ui.add_space(6.0);
            }
            Block::Lines(n) => {
                for _ in 0..*n {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), self.size * 1.9), egui::Sense::hover());
                    ui.painter().hline(r.x_range(), r.bottom() - 2.0, Stroke::new(1.0_f32, Color32::from_gray(185)));
                }
            }
            Block::Para { inl, bullet, heading } => {
                let size = if *heading { self.size * 1.3 } else { self.size };
                if *bullet {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("•  ").size(size).color(INK));
                        ui.vertical(|ui| self.para(ui, inl, size, false));
                    });
                } else {
                    self.para(ui, inl, size, *heading);
                }
            }
            Block::Table { rows, cells, .. } => self.table(ui, rows, cells),
        }
    }

    fn fmt(&self, size: f32, b: bool, i: bool, u: bool, sup: bool, sub: bool) -> TextFormat {
        let small = sup || sub;
        let sz = if small { size * 0.7 } else { size };
        TextFormat {
            font_id: FontId::new(sz, crate::fonts::family(false, i, b)),
            color: INK,
            underline: if u { Stroke::new(1.0_f32, INK) } else { Stroke::NONE },
            valign: if sup {
                Align::TOP
            } else if sub {
                Align::BOTTOM
            } else {
                Align::Center
            },
            ..Default::default()
        }
    }

    pub fn para(&mut self, ui: &mut egui::Ui, inl: &[Inl], size: f32, heading: bool) {
        if inl.is_empty() {
            ui.add_space(size * 0.55);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().interact_size.y = size * 1.2;
            let mut job = LayoutJob::default();
            let flush = |ui: &mut egui::Ui, job: &mut LayoutJob| {
                if !job.sections.is_empty() {
                    let j = std::mem::take(job);
                    ui.add(egui::Label::new(j).selectable(false));
                }
            };
            for x in inl {
                match x {
                    Inl::Text { s, b, i, u, sup, sub, color } => {
                        let mut f = self.fmt(size, *b || heading, *i, *u, *sup, *sub);
                        if let Some(c) = color {
                            f.color = Color32::from_rgb(c[0], c[1], c[2]);
                        }
                        job.append(s, 0.0, f);
                    }
                    Inl::Check(on) => {
                        flush(ui, &mut job);
                        let side = size * 0.95;
                        let (r, _) = ui.allocate_exact_size(Vec2::new(side + 6.0, side), egui::Sense::hover());
                        let bx = egui::Rect::from_center_size(r.center(), Vec2::splat(side * 0.85));
                        ui.painter().rect_stroke(bx, 1.0, Stroke::new(1.3_f32, INK), egui::StrokeKind::Inside);
                        if *on {
                            let s = Stroke::new(1.8_f32, Color32::from_rgb(200, 30, 30));
                            ui.painter().line_segment([bx.left_top() + Vec2::splat(2.0), bx.right_bottom() - Vec2::splat(2.0)], s);
                            ui.painter().line_segment([bx.right_top() + Vec2::new(-2.0, 2.0), bx.left_bottom() + Vec2::new(2.0, -2.0)], s);
                        }
                    }
                    Inl::Obj(id) => {
                        flush(ui, &mut job);
                        self.object(ui, *id);
                    }
                    Inl::Link { text, url } => {
                        flush(ui, &mut job);
                        let full = if url.starts_with("www.") { format!("https://{url}") } else { url.clone() };
                        ui.hyperlink_to(egui::RichText::new(text).size(size).color(Color32::from_rgb(40, 80, 200)), full);
                    }
                }
            }
            flush(ui, &mut job);
        });
    }

    fn object(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(o) = self.doc.object(id) else {
            ui.label(egui::RichText::new(format!("[fehlt: {id}]")).color(Color32::RED));
            return;
        };
        let zoom = self.zoom * self.size / 15.0;
        let max_w = ui.available_width().max(60.0);
        let ctx = ui.ctx().clone();
        match self.cache.get(&ctx, o) {
            Loaded::Texture(t) => {
                let nat = t.size_vec2();
                let mut sz = if o.width > 0.0 && o.height > 0.0 { Vec2::new(o.width, o.height) } else { nat };
                sz *= zoom;
                if sz.x > max_w {
                    sz *= max_w / sz.x;
                }
                ui.add(egui::Image::new((t.id(), sz)));
            }
            Loaded::Drawing(d) => {
                let mut sz = if o.width > 0.0 && o.height > 0.0 { Vec2::new(o.width, o.height) } else { Vec2::new(d.ext.x.abs() / 15.0, d.ext.y.abs() / 15.0) };
                sz *= zoom;
                if sz.x > max_w {
                    sz *= max_w / sz.x;
                }
                let (r, _) = ui.allocate_exact_size(sz, egui::Sense::hover());
                if ui.is_rect_visible(r) {
                    formula::paint(d, ui.painter(), r, Some(INK));
                }
            }
            Loaded::Failed => {
                ui.label(egui::RichText::new("[Objekt kann nicht angezeigt werden]").italics().color(Color32::GRAY));
            }
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn table(&mut self, ui: &mut egui::Ui, rows: &[Vec<Vec<Vec<Inl>>>], info: &[Vec<CellInfo>]) {
        let cols = info.first().map(|r| r.len()).unwrap_or(1).max(1);
        let cell_info = |r: usize, c: usize| info.get(r).and_then(|x| x.get(c)).copied().unwrap_or_default();
        let mut weight = vec![2.0f32; cols];
        let mut short = vec![0.0f32; cols];
        for (ri, r) in rows.iter().enumerate() {
            for (ci, cell) in r.iter().enumerate().take(cols) {
                // merged cells do not make a single column wider
                if cell_info(ri, ci).colspan != 1 {
                    continue;
                }
                let mut len = 0.0;
                for line in cell {
                    let mut l = 0.0;
                    for x in line {
                        l += match x {
                            Inl::Text { s, .. } => s.chars().count() as f32,
                            Inl::Check(_) => 2.0,
                            Inl::Link { text, .. } => text.chars().count() as f32,
                            Inl::Obj(id) => self.doc.object(*id).map(|o| o.width / 7.0).unwrap_or(10.0),
                        };
                    }
                    len = f32::max(len, l);
                }
                weight[ci] = weight[ci].max(len.min(70.0));
                if len <= 14.0 {
                    short[ci] = short[ci].max(len);
                }
            }
        }
        let avail = ui.available_width() - 2.0;
        let widths: Vec<f32> = column_widths(&weight, &short, avail, self.size * 0.58, 14.0).into_iter().map(|w| w.max(28.0)).collect();
        let mut xs = vec![0.0f32; cols + 1];
        for c in 0..cols {
            xs[c + 1] = xs[c] + widths[c];
        }
        let stroke = Stroke::new(1.0_f32, Color32::from_gray(120));
        ui.spacing_mut().item_spacing.y = 0.0;
        let x0 = ui.cursor().min.x;
        let n = rows.len();
        let (mut tops, mut bottoms) = (vec![0.0f32; n], vec![0.0f32; n]);
        // cells over several rows: (last row, top, height of their content)
        let mut tall: Vec<(usize, f32, f32)> = Vec::new();
        for (ri, r) in rows.iter().enumerate() {
            let top = ui.cursor().min.y;
            let mut height = 0.0f32;
            for ci in 0..cols {
                let inf = cell_info(ri, ci);
                if inf.covered() {
                    continue;
                }
                let span = inf.colspan.clamp(1, cols - ci);
                let w = xs[ci + span] - xs[ci];
                let rect = egui::Rect::from_min_size(egui::pos2(x0 + xs[ci], top), Vec2::new(w, 100_000.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(Align::LEFT)));
                child.spacing_mut().item_spacing.y = 0.0;
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(5, 3)).show(&mut child, |ui| {
                    ui.set_width(w - 10.0);
                    if let Some(cell) = r.get(ci) {
                        for line in cell {
                            self.para(ui, line, self.size, false);
                        }
                    }
                });
                let h = child.min_rect().height();
                if inf.rowspan <= 1 {
                    height = height.max(h);
                } else {
                    tall.push(((ri + inf.rowspan - 1).min(n - 1), top, h));
                }
            }
            for (_, t, h) in tall.iter().filter(|x| x.0 == ri) {
                height = height.max(h - (top - t));
            }
            ui.allocate_exact_size(Vec2::new(xs[cols], height.max(1.0)), egui::Sense::hover());
            tops[ri] = top;
            bottoms[ri] = top + height.max(1.0);
        }
        for ri in 0..n {
            for ci in 0..cols {
                let inf = cell_info(ri, ci);
                if inf.covered() || !inf.border {
                    continue;
                }
                let last = (ri + inf.rowspan.max(1) - 1).min(n - 1);
                let span = inf.colspan.clamp(1, cols - ci);
                let rr = egui::Rect::from_min_max(egui::pos2(x0 + xs[ci], tops[ri]), egui::pos2(x0 + xs[ci + span], bottoms[last]));
                ui.painter().rect_stroke(rr, 0.0, stroke, egui::StrokeKind::Middle);
            }
        }
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.add_space(4.0);
    }
}

// ---------------------------------------------------------------- HTML

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG") {
        "image/png"
    } else if data.starts_with(b"GIF8") {
        "image/gif"
    } else if data.starts_with(b"BM") {
        "image/bmp"
    } else {
        "image/jpeg"
    }
}

/// An embedded object as HTML: pictures as data URIs, formulas as SVG.
pub fn obj_html(o: &crate::model::DocObject) -> String {
    let mut s = String::new();
    match o.kind {
        ObjKind::Image => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&o.data);
            let size = if o.width > 0.0 { format!(" width=\"{:.0}\" height=\"{:.0}\"", o.width, o.height) } else { String::new() };
            s += &format!("<img src=\"data:{};base64,{b64}\"{size} style=\"vertical-align:middle;max-width:100%\">", mime(&o.data));
        }
        ObjKind::Formula => {
            if let Some(d) = formula::parse_wmf(&o.data) {
                let (w, h) = if o.width > 0.0 { (o.width, o.height) } else { (d.ext.x.abs() / 15.0, d.ext.y.abs() / 15.0) };
                s += &formula::to_svg(&d, w, h);
            }
        }
        ObjKind::Math => {
            let d = crate::math::layout(&String::from_utf8_lossy(&o.data));
            let (w, h) = crate::math::size_px(&d);
            // put the formula's baseline on the text baseline
            let svg = formula::to_svg(&d, w, h);
            s += &svg.replacen("vertical-align:middle", &format!("vertical-align:-{:.1}px", crate::math::depth_px(&d)), 1);
        }
    }
    s
}

fn inl_html_with(doc: &Doc, inl: &[Inl], obj: &mut dyn FnMut(&crate::model::DocObject) -> String) -> String {
    let mut s = String::new();
    for x in inl {
        match x {
            Inl::Text { s: t, b, i, u, sup, sub, color } => {
                let mut h = html_escape(t).replace("  ", "&nbsp; ");
                if let Some(c) = color {
                    h = format!("<span style=\"color:#{:02x}{:02x}{:02x}\">{h}</span>", c[0], c[1], c[2]);
                }
                if *b {
                    h = format!("<b>{h}</b>");
                }
                if *i {
                    h = format!("<i>{h}</i>");
                }
                if *u {
                    h = format!("<u>{h}</u>");
                }
                if *sup {
                    h = format!("<sup>{h}</sup>");
                }
                if *sub {
                    h = format!("<sub>{h}</sub>");
                }
                s += &h;
            }
            Inl::Check(on) => {
                s += if *on { "<span class=\"box on\">✗</span>" } else { "<span class=\"box\"></span>" };
            }
            Inl::Link { text, url } => {
                let full = if url.starts_with("www.") { format!("https://{url}") } else { url.clone() };
                s += &format!("<a href=\"{}\">{}</a>", html_escape(&full), html_escape(text));
            }
            Inl::Obj(id) => {
                if let Some(o) = doc.object(*id) {
                    s += &obj(o);
                }
            }
        }
    }
    s
}

pub fn lines_html(n: u32, underline: bool) -> String {
    let cls = if underline { "ln u" } else { "ln" };
    (0..n).map(|_| format!("<div class=\"{cls}\"></div>")).collect::<String>() + "\n"
}

pub fn to_html_opts(doc: &Doc, lines: bool, underline: bool) -> String {
    to_html_with(doc, lines, underline, &mut obj_html)
}

/// HTML of a document with an own way of writing the embedded objects.
pub fn to_html_with(doc: &Doc, lines: bool, underline: bool, obj: &mut dyn FnMut(&crate::model::DocObject) -> String) -> String {
    let mut s = String::new();
    for b in parse(&doc.text) {
        match b {
            Block::Empty => s += "<div class=\"gap\"></div>\n",
            Block::Rule => s += "<hr>\n",
            Block::Lines(n) => {
                if lines {
                    s += &lines_html(n, underline);
                }
            }
            Block::Para { inl, bullet, heading } => {
                let body = inl_html_with(doc, &inl, obj);
                if heading {
                    s += &format!("<h3>{body}</h3>\n");
                } else if bullet {
                    s += &format!("<p class=\"li\">•&nbsp;&nbsp;{body}</p>\n");
                } else {
                    s += &format!("<p>{body}</p>\n");
                }
            }
            Block::Table { bordered, rows, cells } => {
                s += &format!("<table class=\"{}\">", if bordered { "grid" } else { "plain" });
                for (ri, r) in rows.iter().enumerate() {
                    s += "<tr>";
                    for (ci, c) in r.iter().enumerate() {
                        let inf = cells.get(ri).and_then(|x| x.get(ci)).copied().unwrap_or_default();
                        if inf.covered() {
                            continue;
                        }
                        let mut attrs = String::new();
                        if inf.colspan > 1 {
                            attrs += &format!(" colspan=\"{}\"", inf.colspan);
                        }
                        if inf.rowspan > 1 {
                            attrs += &format!(" rowspan=\"{}\"", inf.rowspan);
                        }
                        if inf.border != bordered {
                            attrs += if inf.border { " style=\"border:1px solid #555\"" } else { " style=\"border:none\"" };
                        }
                        let cell: Vec<String> = c.iter().map(|l| inl_html_with(doc, l, obj)).collect();
                        s += &format!("<td{attrs}>{}</td>", cell.join("<br>"));
                    }
                    s += "</tr>";
                }
                s += "</table>\n";
            }
        }
    }
    s
}
