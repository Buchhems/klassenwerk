//! Print-ready documents for exams, exercises and scripts: templates with
//! placeholders, grading scale, grade overview; output as HTML (print / PDF)
//! and RTF (opens in Word, formulas included).

use crate::markup::{html_escape, parse, parse_inline, to_html_opts, Block, Inl};
use crate::model::{Database, Doc, DocObject, GradeScheme, ObjKind, Sheet, SheetItem, Task, Template};
use crate::rvf::escape;

/// One part of a composed document.
#[derive(Default)]
pub struct Section {
    /// (heading text, points text)
    pub heading: Option<(String, String)>,
    pub doc: Doc,
    pub lines: u32,
    /// underline the blank lines of this task (original: Int1 < 0)
    pub underline: bool,
    /// own font of the task (empty = standard) and size in pt (0 = standard)
    pub font: String,
    pub font_size: f32,
}

impl Section {
    /// The section's own font and size when they differ from the standard.
    pub fn own_font(&self, db: &Database) -> Option<(String, f32)> {
        let f = self.font.trim();
        let font = if f.is_empty() { db.settings.print_font.clone() } else { f.to_string() };
        let size = if self.font_size > 0.0 { self.font_size } else { db.settings.print_size };
        (font != db.settings.print_font || (size - db.settings.print_size).abs() > 0.01).then_some((font, size))
    }

    /// A section that is just text (markup), without heading or answer lines.
    pub fn text(markup: &str) -> Self {
        Section { heading: None, doc: Doc::plain(markup), ..Default::default() }
    }
}

pub struct Composed {
    pub title: String,
    pub sections: Vec<Section>,
    pub page_numbers: bool,
    pub footer: String,
}

pub fn fmt_pts(p: f32) -> String {
    if (p - p.round()).abs() < 0.01 {
        format!("{:.0}", p)
    } else {
        format!("{:.1}", p).replace('.', ",")
    }
}

/// A point bound of a grading key: whole numbers without decimals, otherwise
/// with one decimal ("28", "28,5"). The bounds are exact multiples of the key's
/// step, so nothing is hidden by rounding.
pub fn fmt_bound(p: f32) -> String {
    let r = (p as f64 * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}").replace('.', ",")
    }
}

fn logo_doc_obj(db: &Database, id: u32, height: f32) -> Option<DocObject> {
    if db.settings.logo.is_empty() {
        return None;
    }
    let (w, h) = image::load_from_memory(&db.settings.logo).map(|i| (i.width() as f32, i.height() as f32)).unwrap_or((height, height));
    Some(DocObject { id, kind: ObjKind::Image, data: db.settings.logo.clone(), width: w * height / h.max(1.0), height })
}

/// Fills placeholders; the logo becomes an embedded object.
pub fn fill(db: &Database, sheet: &Sheet, text: &str, nr: Option<(usize, &Task)>) -> Doc {
    let total = db.sheet_total(sheet);
    let n_tasks = sheet.task_ids().len();
    let mut t = text.to_string();
    let rep = |t: &mut String, k: &str, v: &str| *t = t.replace(k, &escape(v));
    rep(&mut t, "{Schule}", &db.settings.school);
    rep(&mut t, "{Adresse}", &db.settings.school_address);
    rep(&mut t, "{Lehrkraft}", &db.settings.teacher);
    if t.contains("{Gruppe}") {
        let g = if sheet.group.trim().is_empty() { String::new() } else { format!("Gruppe {}", sheet.group.trim()) };
        rep(&mut t, "{Gruppe}", &g);
        rep(&mut t, "{Titel}", &sheet.title);
    } else {
        // templates without {Gruppe}: the group goes into the title
        rep(&mut t, "{Titel}", &sheet.title_with_group());
    }
    rep(&mut t, "{Typ}", &sheet.kind);
    rep(&mut t, "{Fach}", &sheet.subject);
    rep(&mut t, "{Klasse}", &sheet.class_name);
    rep(&mut t, "{Datum}", &sheet.date);
    rep(&mut t, "{Dauer}", &sheet.minutes.to_string());
    rep(&mut t, "{Gesamtpunkte}", &total.to_string());
    rep(&mut t, "{Anzahl}", &n_tasks.to_string());
    if let Some((n, task)) = nr {
        rep(&mut t, "{Nr}", &n.to_string());
        rep(&mut t, "{Aufgabentitel}", &task.title());
        rep(&mut t, "{Punkte}", &task.points.to_string());
    }
    let mut doc = Doc { text: String::new(), objects: vec![] };
    if t.contains("{Logo}") {
        if let Some(o) = logo_doc_obj(db, 1, 56.0) {
            doc.objects.push(o);
            t = t.replace("{Logo}", "{{1}}");
        } else {
            t = t.replace("{Logo}", "");
            t = t.lines().map(|l| l.trim_start().to_string()).collect::<Vec<_>>().join("\n");
        }
    }
    // drop empty separators left by empty placeholders, e.g. "Klasse:  ·"
    doc.text = t;
    doc
}

/// Grading key as a bordered markup table for the printout: a row with the
/// grades, one with the point ranges, optionally the percentages and an empty
/// row "Anzahl" to fill in by hand. Keys with many grades are split into
/// several tables of at most eight grades, so they fit across the page.
pub fn scale_table(scheme: &GradeScheme, total: f32) -> String {
    let rows: Vec<(String, f32, f32)> = scheme.rows_for(total).into_iter().filter(|(_, a, b)| a <= b).collect();
    let mut sorted = scheme.rows.clone();
    sorted.sort_by(|a, b| b.min_percent.partial_cmp(&a.min_percent).unwrap_or(std::cmp::Ordering::Equal));
    let pct: std::collections::HashMap<&str, f32> = sorted.iter().map(|r| (r.label.as_str(), r.min_percent)).collect();
    let mut out = String::new();
    if !scheme.heading.trim().is_empty() {
        out += &format!("**{}**\n", escape(scheme.heading.trim()));
    }
    let parts = rows.len().div_ceil(8).max(1);
    let per = rows.len().div_ceil(parts).max(1);
    for (k, chunk) in rows.chunks(per).enumerate() {
        if k > 0 {
            out += "\n";
        }
        let mut line = |cap: &str, cells: Vec<String>| {
            out += &format!("+| **{}** |", escape(cap));
            for c in cells {
                out += &format!(" {c} |");
            }
            out += "\n";
        };
        line("Note", chunk.iter().map(|r| format!("**{}**", escape(&r.0))).collect());
        line(
            "Punkte",
            chunk.iter().map(|r| if (r.1 - r.2).abs() < 1e-6 { fmt_bound(r.1) } else { format!("{} – {}", fmt_bound(r.1), fmt_bound(r.2)) }).collect(),
        );
        if scheme.show_percent {
            line("ab %", chunk.iter().map(|r| pct.get(r.0.as_str()).map(|p| fmt_pts(*p)).unwrap_or_default()).collect());
        }
        if scheme.count_row {
            line("Anzahl", vec![" ".to_string(); chunk.len()]);
        }
    }
    out.trim_end().to_string()
}

pub struct Spiegel {
    pub labels: Vec<String>,
    pub counts: Vec<usize>,
    pub average: Option<f32>,
    pub avg_points: Option<f32>,
    pub failed: usize,
    /// index of the first grade that counts as failed
    pub fail_index: Option<usize>,
}

/// Grade overview (Notenspiegel) for a sheet with entered results.
pub fn spiegel(db: &Database, sheet: &Sheet) -> Option<Spiegel> {
    let scheme = db.settings.scheme(&sheet.scheme)?;
    let total = db.sheet_total(sheet) as f32;
    let rows = scheme.rows_for(total);
    let labels: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    let mut counts = vec![0usize; rows.len()];
    let (mut sum, mut n, mut psum, mut failed) = (0.0, 0, 0.0, 0);
    let fail_idx = labels.iter().position(|l| *l == scheme.fail_from).unwrap_or(usize::MAX);
    let present: Vec<&crate::model::ResultRow> = sheet.present_results().collect();
    for r in &present {
        psum += r.points;
        if let Some(i) = scheme.grade_for(total, r.points) {
            counts[i] += 1;
            sum += GradeScheme::value(&rows[i].0);
            n += 1;
            if i >= fail_idx {
                failed += 1;
            }
        }
    }
    Some(Spiegel {
        labels,
        counts,
        average: if n > 0 { Some(sum / n as f32) } else { None },
        avg_points: if present.is_empty() { None } else { Some(psum / present.len() as f32) },
        failed,
        fail_index: (fail_idx != usize::MAX).then_some(fail_idx),
    })
}

/// Bar chart of the grade overview as a drawing (1 px = 15 units), stored like
/// a picture of the old formula editor, so print, Word and RTF show it as a
/// vector graphic.
pub fn spiegel_chart(sp: &Spiegel, id: u32) -> DocObject {
    use crate::formula::{Drawing, Prim};
    use egui::{Color32, Pos2, Vec2};
    const K: f32 = 15.0;
    let n = sp.labels.len().max(1) as f32;
    let (w, h) = (560.0f32, 190.0f32);
    let (left, bottom, top) = (34.0f32, 150.0f32, 18.0f32);
    let maxc = sp.counts.iter().copied().max().unwrap_or(0).max(1);
    // a round step for the scale on the left
    let step = [1usize, 2, 5, 10, 20, 50, 100].into_iter().find(|s| maxc / s <= 6).unwrap_or(100);
    let top_val = maxc.div_ceil(step) * step;
    let y_of = |v: f32| bottom - (bottom - top) * v / top_val as f32;
    let ink = Color32::from_rgb(40, 40, 40);
    let grid = Color32::from_rgb(200, 200, 200);
    // every character at its own position: the metafile writes them one by one
    let text = |x: f32, y: f32, s: &str, size: f32, halign: u8, bold: bool| {
        let adv = |c: char| -> f32 {
            let em = match c {
                'i' | 'j' | 'l' | 'I' | '.' | ',' | ' ' => 0.28,
                'f' | 't' | 'r' => 0.33,
                'm' | 'M' | 'W' => 0.83,
                'w' => 0.72,
                'A'..='Z' => 0.67,
                _ => 0.556,
            };
            em * size * if bold { 1.05 } else { 1.0 }
        };
        let width: f32 = s.chars().map(adv).sum();
        let mut cx = match halign {
            1 => x - width / 2.0,
            2 => x - width,
            _ => x,
        };
        let mut xs = Vec::new();
        for c in s.chars() {
            xs.push((cx + adv(c) / 2.0) * K);
            cx += adv(c);
        }
        Prim::Text {
            xs,
            y: y * K,
            chars: s.chars().collect(),
            size: size * K,
            italic: false,
            bold,
            serif: false,
            color: ink,
            valign: 1,
            halign: 1,
            has_dx: true,
        }
    };
    let mut prims = Vec::new();
    let mut v = 0;
    while v <= top_val {
        let y = y_of(v as f32);
        prims.push(Prim::Lines { pts: vec![Pos2::new(left * K, y * K), Pos2::new(w * K, y * K)], width: K * 0.6, color: if v == 0 { ink } else { grid } });
        prims.push(text(left - 6.0, y + 4.0, &v.to_string(), 11.0, 2, false));
        v += step;
    }
    let bw = (w - left) / n;
    let fail = sp.fail_index.unwrap_or(usize::MAX);
    for (i, (l, c)) in sp.labels.iter().zip(&sp.counts).enumerate() {
        let x0 = left + i as f32 * bw;
        let (a, b) = (x0 + bw * 0.18, x0 + bw * 0.82);
        if *c > 0 {
            let y = y_of(*c as f32);
            let fill = if i >= fail { Color32::from_rgb(196, 120, 110) } else { Color32::from_rgb(110, 128, 196) };
            prims.push(Prim::Polygon {
                pts: vec![Pos2::new(a * K, y * K), Pos2::new(b * K, y * K), Pos2::new(b * K, bottom * K), Pos2::new(a * K, bottom * K)],
                fill: Some(fill),
                stroke: Some((K * 0.8, ink)),
            });
            prims.push(text((a + b) / 2.0, y - 4.0, &c.to_string(), 11.0, 1, true));
        }
        prims.push(text(x0 + bw / 2.0, bottom + 17.0, l, 12.0, 1, true));
    }
    prims.push(text(left, h - 4.0, "Anzahl je Note", 10.0, 0, false));
    let d = Drawing { org: Pos2::ZERO, ext: Vec2::new(w * K, h * K), prims };
    // stored without the placeable header, like the pictures of the old formula editor
    let wmf = crate::wmf::from_drawing(&d, 1440);
    DocObject { id, kind: ObjKind::Formula, data: wmf[22..].to_vec(), width: w, height: h }
}

pub fn spiegel_markup(sp: &Spiegel, participants: usize) -> String {
    let mut a = String::from("+| **Note** |");
    let mut b = String::from("+| **Anzahl** |");
    for (l, c) in sp.labels.iter().zip(&sp.counts) {
        a += &format!(" {} |", escape(l));
        b += &format!(" {c} |");
    }
    let mut s = format!("**Notenspiegel**\n{a}\n{b}\n");
    if let Some(avg) = sp.average {
        s += &format!(
            "Teilnehmer: {participants}   ·   Durchschnitt: **{}**{}   ·   nicht bestanden: {}",
            format!("{avg:.2}").replace('.', ","),
            sp.avg_points.map(|p| format!("   ·   Ø Punkte: {}", format!("{p:.1}").replace('.', ","))).unwrap_or_default(),
            sp.failed
        );
    }
    s
}

/// Builds the full document for a sheet.
pub fn compose(db: &Database, sheet: &Sheet, solution: bool) -> Composed {
    let tpl: Template = db.settings.template(&sheet.template);
    let mut sections = Vec::new();
    let mut head = fill(db, sheet, &tpl.header, None);
    if solution {
        head.text = format!("{}\n# Musterlösung", head.text.trim_end());
    }
    sections.push(Section { heading: None, doc: head, ..Default::default() });
    let mut nr = 0;
    for item in &sheet.items {
        match item {
            SheetItem::Text(t) => {
                if !solution {
                    sections.push(Section { heading: None, doc: Doc::plain(t), ..Default::default() });
                }
            }
            SheetItem::Task(id) => {
                let Some(task) = db.task(*id) else { continue };
                let task = &task.varied(sheet.seed);
                nr += 1;
                let heading = fill(db, sheet, &tpl.task_heading, Some((nr, task))).text.replace("{{1}}", "");
                let pts = if tpl.show_points && task.points > 0 { format!("{} P.", task.points) } else { String::new() };
                let doc = if solution { task.solution.clone() } else { task.task.clone() };
                let lines = if !solution && sheet.print_lines { task.answer_lines } else { 0 };
                sections.push(Section {
                    heading: Some((heading, pts)),
                    doc,
                    lines,
                    underline: task.underline_answer,
                    font: task.font.clone(),
                    font_size: task.font_size,
                });
            }
        }
    }
    let total = db.sheet_total(sheet) as f32;
    let mut tail = String::new();
    let mut tail_objects = Vec::new();
    if !solution {
        if !tpl.closing.trim().is_empty() {
            tail += &format!("{}\n", tpl.closing.trim());
        }
        if tpl.scale_table && total > 0.0 {
            if let Some(sc) = db.settings.scheme(&sheet.scheme) {
                tail += &format!("\n{}\n", scale_table(sc, total));
            }
        }
        if sheet.print_spiegel && !sheet.results.is_empty() {
            if let Some(sp) = spiegel(db, sheet) {
                tail += &format!("\n{}\n{{{{1}}}}\n", spiegel_markup(&sp, sheet.present_results().count()));
                tail_objects.push(spiegel_chart(&sp, 1));
            }
        }
    }
    if !tail.is_empty() {
        sections.push(Section { heading: None, doc: Doc { text: tail, objects: tail_objects }, ..Default::default() });
    }
    let title = if solution { format!("{} – Lösungen", sheet.title_with_group()) } else { sheet.title_with_group() };
    Composed { title, sections, page_numbers: tpl.page_numbers, footer: footer_text(db) }
}

pub fn compose_spiegel(db: &Database, sheet: &Sheet) -> Composed {
    let mut head = fill(
        db,
        sheet,
        "{Logo}  **{Schule}**\n# Notenspiegel: {Titel}\n{Typ} · Fach: {Fach} · Klasse: {Klasse} · Datum: {Datum} · Gesamtpunkte: {Gesamtpunkte}\n---",
        None,
    );
    let total = db.sheet_total(sheet) as f32;
    let mut body = String::new();
    if let Some(sp) = spiegel(db, sheet) {
        body += &spiegel_markup(&sp, sheet.present_results().count());
        // the chart (object 2; object 1 is the school logo)
        body += "\n{{2}}\n";
        head.objects.push(spiegel_chart(&sp, 2));
        // points per task, when they were entered
        let cols = if sheet.results.iter().any(|r| !r.scores.is_empty()) { db.score_columns(sheet) } else { vec![] };
        body += "\n**Einzelergebnisse**\n+| **Nr.** | **Name** |";
        for c in &cols {
            body += &format!(" **{}** |", escape(&c.label));
        }
        body += " **Punkte** | **Note** |\n";
        if !cols.is_empty() {
            body += "+| | *max.* |";
            for c in &cols {
                body += &format!(" *{}* |", fmt_pts(c.max));
            }
            body += &format!(" *{}* | |\n", fmt_pts(total));
        }
        let sc = db.settings.scheme(&sheet.scheme).unwrap();
        let rows = sc.rows_for(total);
        for (i, r) in sheet.results.iter().enumerate() {
            let g = if r.absent { "fehlt".to_string() } else { sc.grade_for(total, r.points).map(|g| rows[g].0.clone()).unwrap_or_default() };
            body += &format!("+| {} | {} |", i + 1, escape(&r.name));
            for c in &cols {
                body += &format!(" {} |", r.scores.get(&c.key).map(|v| fmt_pts(*v)).unwrap_or_default());
            }
            body += &format!(" {} | {} |\n", if r.absent { String::new() } else { fmt_pts(r.points) }, escape(&g));
        }
        if !cols.is_empty() {
            // how well each task went, in percent of its points
            body += "+| | *Ø %* |";
            for c in &cols {
                let v: Vec<f32> = sheet.present_results().filter_map(|r| r.scores.get(&c.key).copied()).collect();
                let t = if v.is_empty() || c.max <= 0.0 { String::new() } else { format!("{:.0}", v.iter().sum::<f32>() / v.len() as f32 / c.max * 100.0) };
                body += &format!(" *{t}* |");
            }
            body += " | |\n";
        }
    } else {
        body += "Für diese Mappe ist kein Notenschlüssel gewählt.";
    }
    if !sheet.remarks.trim().is_empty() {
        body += &format!("\n**Bemerkungen:** {}\n", escape(&sheet.remarks));
    }
    body += "\n\n\nDatum, Unterschrift Lehrkraft: ____________________________\n\nKenntnisnahme Schulleitung: ____________________________";
    head.text = format!("{}\n{}", head.text, body);
    Composed {
        title: format!("Notenspiegel – {}", sheet.title),
        sections: vec![Section { heading: None, doc: head, ..Default::default() }],
        page_numbers: true,
        footer: footer_text(db),
    }
}

/// Erwartungshorizont of a sheet: for every task its Teilaufgaben with the
/// expected answer and the points, followed by the model solution (unless the
/// Teilaufgaben already describe everything), and the grading key at the end.
pub fn compose_expectation(db: &Database, sheet: &Sheet) -> Composed {
    let head = fill(
        db,
        sheet,
        "{Logo}  **{Schule}**\n# Erwartungshorizont: {Titel}\n{Typ} · Fach: {Fach} · Klasse: {Klasse} · Datum: {Datum} · Gesamtpunkte: {Gesamtpunkte}\n---",
        None,
    );
    let mut sections = vec![Section { heading: None, doc: head, ..Default::default() }];
    let mut nr = 0;
    for id in sheet.task_ids() {
        let Some(t) = db.task(id) else { continue };
        let t = &t.varied(sheet.seed);
        nr += 1;
        let mut text = String::new();
        if !t.parts.is_empty() {
            text += "+| **Teil** | **Erwartete Leistung** | **Punkte** |\n";
            for p in &t.parts {
                let exp = if p.expectation.trim().is_empty() { "–".to_string() } else { p.expectation.trim().replace('|', "\\|").replace('\n', "<br>") };
                text += &format!("+| {nr}{} | {} | {} |\n", escape(&p.label), exp, p.points);
            }
            text += &format!("+| | **Summe** | **{}** |\n", t.points);
        }
        let complete = !t.parts.is_empty() && t.parts.iter().all(|p| !p.expectation.trim().is_empty());
        let mut doc = Doc { text, objects: vec![] };
        if !complete && !t.solution.text.trim().is_empty() {
            // the model solution with its pictures and formulas (the table above has no objects)
            doc.objects = t.solution.objects.clone();
            doc.text += &format!("{}**Musterlösung**\n{}", if doc.text.is_empty() { "" } else { "\n" }, t.solution.text);
        }
        sections.push(Section { heading: Some((format!("Aufgabe {nr}: {}", escape(&t.title())), format!("{} P.", t.points))), doc, ..Default::default() });
    }
    let total = db.sheet_total(sheet) as f32;
    if total > 0.0 {
        if let Some(sc) = db.settings.scheme(&sheet.scheme) {
            sections.push(Section::text(&format!("\n{}", scale_table(sc, total))));
        }
    }
    Composed { title: format!("Erwartungshorizont – {}", sheet.title_with_group()), sections, page_numbers: true, footer: footer_text(db) }
}

pub fn compose_task(db: &Database, t: &Task) -> Composed {
    let t = &t.varied(0);
    let head = Doc::plain(&format!("# {}\n{} · {} · {} Punkte\n---", escape(&t.title()), escape(&t.subject), escape(&t.topic), t.points));
    Composed {
        title: t.title(),
        sections: vec![
            Section { heading: None, doc: head, ..Default::default() },
            Section {
                heading: Some(("Aufgabe".into(), String::new())),
                doc: t.task.clone(),
                lines: t.answer_lines,
                underline: t.underline_answer,
                font: t.font.clone(),
                font_size: t.font_size,
            },
            Section {
                heading: Some(("Lösung".into(), String::new())),
                doc: t.solution.clone(),
                font: t.font.clone(),
                font_size: t.font_size,
                ..Default::default()
            },
        ],
        page_numbers: false,
        footer: footer_text(db),
    }
}

fn footer_text(db: &Database) -> String {
    let mut f = Vec::new();
    if !db.settings.school.is_empty() {
        f.push(db.settings.school.clone());
    }
    if !db.settings.footer.is_empty() {
        f.push(db.settings.footer.clone());
    }
    f.join(" · ")
}

/// Page margins in mm (top, right, bottom, left): 20 mm all round and a wider
/// left margin of 25 mm for punching and filing.
pub const PAGE_MARGINS_MM: [f32; 4] = [20.0, 20.0, 20.0, 25.0];
/// Distance of the blank answer lines in mm: fixed, independent of the font
/// size, so there is the same room for handwriting in every task.
pub const ANSWER_LINE_MM: f32 = 9.0;
/// The same in points.
pub fn answer_line_pt() -> f32 {
    ANSWER_LINE_MM * 72.0 / 25.4
}

fn mm_twips(mm: f32) -> i32 {
    (mm * 1440.0 / 25.4).round() as i32
}

// ---------------------------------------------------------------- HTML

pub fn to_html(db: &Database, c: &Composed, underline: bool) -> String {
    let font = &db.settings.print_font;
    let size = db.settings.print_size;
    let pages = if c.page_numbers {
        "@page{size:A4;margin:20mm 20mm 20mm 25mm;@bottom-right{content:\"Seite \" counter(page) \" von \" counter(pages);font-size:9pt;color:#666}}"
    } else {
        "@page{size:A4;margin:20mm 20mm 20mm 25mm}"
    };
    let line_pt = answer_line_pt();
    let mut s = format!(
        "<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"utf-8\"><title>{}</title><style>{pages}
body{{font-family:'{font}',Arial,sans-serif;font-size:{size}pt;color:#111;max-width:165mm;margin:10mm auto;line-height:1.35}}
.task{{break-inside:avoid;page-break-inside:avoid;margin:0 0 6mm 0}}
.task h2{{font-size:{h2}pt;margin:3mm 0 2mm 0;display:flex;justify-content:space-between;border-bottom:1px solid #999;padding-bottom:1mm}}
.pts{{font-weight:normal;color:#333}}
p{{margin:0 0 1mm 0}}.gap{{height:3mm}}h3{{font-size:{h3}pt;margin:1mm 0}}
table{{border-collapse:collapse;margin:1mm 0 2mm 0;width:100%}}td{{vertical-align:top;padding:1mm 2mm}}
table.grid td{{border:1px solid #555;text-align:center}}table.grid td:first-child{{text-align:left}}
.box{{display:inline-block;width:3.2mm;height:3.2mm;border:1px solid #111;margin:0 1.5mm;vertical-align:middle;text-align:center;line-height:3.2mm;font-size:9pt;color:#c00}}
.ln{{height:var(--ln,{line_pt:.1}pt)}}.ln.u{{border-bottom:1px solid #777}}
footer{{margin-top:10mm;font-size:9pt;color:#777;text-align:center}}
.noprint{{background:#eef;border:1px solid #99c;padding:3mm;margin-bottom:6mm;font-size:10pt;font-family:Segoe UI,Arial}}
@media print{{body{{margin:0 auto}}.noprint{{display:none}}}}
</style></head><body>
<div class=\"noprint\">Druckvorschau – mit <b>Strg+P</b> drucken oder „Als PDF speichern“ wählen.</div>
",
        html_escape(&c.title),
        h2 = size + 1.0,
        h3 = size + 4.0
    );
    for sec in &c.sections {
        if let Some((h, p)) = &sec.heading {
            s += "<section class=\"task\">";
            let hh: String = crate::markup::plain(h);
            s += &format!("<h2><span>{}</span><span class=\"pts\">{}</span></h2>", html_escape(&hh), html_escape(p));
            let own = sec.own_font(db);
            if let Some((f, sz)) = &own {
                s += &format!("<div style=\"font-family:'{}',Arial,sans-serif;font-size:{sz}pt\">", html_escape(&f.replace('\'', "")));
            }
            s += &to_html_opts(&sec.doc, true, underline);
            if sec.lines > 0 {
                s += &crate::markup::lines_html(sec.lines, underline || sec.underline);
            }
            if own.is_some() {
                s += "</div>";
            }
            s += "</section>\n";
        } else {
            s += "<section>";
            s += &to_html_opts(&sec.doc, true, underline);
            s += "</section>\n";
        }
    }
    if !c.footer.is_empty() {
        s += &format!("<footer>{}</footer>", html_escape(&c.footer));
    }
    s += "</body></html>";
    s
}

// ---------------------------------------------------------------- RTF

fn rtf_text(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => o += "\\\\",
            '{' => o += "\\{",
            '}' => o += "\\}",
            '\t' => o += "\\tab ",
            c if (c as u32) < 128 => o.push(c),
            c => {
                let v = c as u32;
                if v < 0x10000 {
                    o += &format!("\\u{}?", v as u16 as i16);
                } else {
                    o.push('?');
                }
            }
        }
    }
    o
}

fn hexs(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2 + b.len() / 64);
    for (i, x) in b.iter().enumerate() {
        if i % 64 == 0 {
            s.push('\n');
        }
        s += &format!("{x:02x}");
    }
    s
}

/// Text width in twips: A4 (210 mm) minus the left and right margins.
const PAGE_TW: f32 = (210.0 - PAGE_MARGINS_MM[3] - PAGE_MARGINS_MM[1]) * 1440.0 / 25.4;

fn rtf_obj(o: &DocObject, max_tw: f32) -> String {
    let (mut w, mut h) = (o.width, o.height);
    match o.kind {
        ObjKind::Image => {
            let (data, blip) = if o.data.starts_with(b"\x89PNG") {
                (o.data.clone(), "pngblip")
            } else if o.data.starts_with(b"\xFF\xD8") {
                (o.data.clone(), "jpegblip")
            } else {
                match image::load_from_memory(&o.data) {
                    Ok(img) => {
                        let mut v = Vec::new();
                        let _ = img.write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png);
                        (v, "pngblip")
                    }
                    Err(_) => return String::new(),
                }
            };
            if w <= 0.0 || h <= 0.0 {
                if let Ok(i) = image::load_from_memory(&data) {
                    w = i.width() as f32;
                    h = i.height() as f32;
                }
            }
            let mut tw = w * 15.0;
            let mut th = h * 15.0;
            if tw > max_tw {
                th *= max_tw / tw;
                tw = max_tw;
            }
            format!("{{\\pict\\{blip}\\picw{:.0}\\pich{:.0}\\picwgoal{:.0}\\pichgoal{:.0}{}}}", w, h, tw, th, hexs(&data))
        }
        ObjKind::Math => {
            let src = String::from_utf8_lossy(&o.data);
            let d = crate::math::layout(&src);
            let (w, h) = crate::math::size_px(&d);
            let wmf = crate::wmf::from_drawing(&d, (crate::math::U * 6.0) as u16);
            let mut tw = w * 15.0;
            let mut th = h * 15.0;
            if tw > max_tw {
                th *= max_tw / tw;
                tw = max_tw;
            }
            // the picture data of \wmetafile is the metafile without its placeable header;
            // \dn lowers the picture (in half points) so the formula sits on the text baseline
            let dn = (crate::math::depth_px(&d) * 0.75 * 2.0).round() as i32;
            format!(
                "{{\\dn{dn} {{\\pict\\wmetafile8\\picw{:.0}\\pich{:.0}\\picwgoal{:.0}\\pichgoal{:.0}{}}}}}",
                w * 2540.0 / 96.0,
                h * 2540.0 / 96.0,
                tw,
                th,
                hexs(&wmf[22..])
            )
        }
        ObjKind::Formula => {
            if w <= 0.0 || h <= 0.0 {
                if let Some(d) = crate::formula::parse_wmf(&o.data) {
                    w = d.ext.x.abs() / 15.0;
                    h = d.ext.y.abs() / 15.0;
                }
            }
            let mut tw = w * 15.0;
            let mut th = h * 15.0;
            if tw > max_tw {
                th *= max_tw / tw;
                tw = max_tw;
            }
            format!("{{\\pict\\wmetafile8\\picw{:.0}\\pich{:.0}\\picwgoal{:.0}\\pichgoal{:.0}{}}}", w * 2540.0 / 96.0, h * 2540.0 / 96.0, tw, th, hexs(&o.data))
        }
    }
}

/// Colour table of the RTF output: 1 black, 2 red and 3 blue (used for marks),
/// then the text colours of markup::COLORS from index 4.
fn rtf_colortbl() -> String {
    let mut s = String::from("{\\colortbl;\\red0\\green0\\blue0;\\red200\\green30\\blue30;\\red40\\green80\\blue200;");
    for (_, c) in crate::markup::COLORS.iter() {
        s += &format!("\\red{}\\green{}\\blue{};", c[0], c[1], c[2]);
    }
    s + "}"
}

fn rtf_color_index(c: [u8; 3]) -> usize {
    crate::markup::COLORS.iter().position(|(_, v)| *v == c).map(|i| i + 4).unwrap_or(1)
}

fn rtf_inl(doc: &Doc, inl: &[Inl], max_tw: f32) -> String {
    let mut s = String::new();
    for x in inl {
        match x {
            Inl::Text { s: t, b, i, u, sup, sub, color } => {
                let mut pre = String::new();
                if let Some(c) = color {
                    pre += &format!("\\cf{}", rtf_color_index(*c));
                }
                if *b {
                    pre += "\\b";
                }
                if *i {
                    pre += "\\i";
                }
                if *u {
                    pre += "\\ul";
                }
                if *sup {
                    pre += "\\super";
                }
                if *sub {
                    pre += "\\sub";
                }
                if pre.is_empty() {
                    s += &rtf_text(t);
                } else {
                    s += &format!("{{{pre} {}}}", rtf_text(t));
                }
            }
            Inl::Check(on) => s += if *on { "{\\cf2\\u9746?}" } else { "\\u9744?" },
            Inl::Obj(id) => {
                if let Some(o) = doc.object(*id) {
                    s += &rtf_obj(o, max_tw);
                }
            }
            Inl::Link { text, url } => {
                s += &format!("{{\\field{{\\*\\fldinst HYPERLINK \"{}\"}}{{\\fldrslt\\ul\\cf3 {}}}}}", rtf_text(url), rtf_text(text));
            }
        }
    }
    s
}

/// One blank answer line: 175 % of a text line high, optionally underlined across the text width.
fn answer_line_rtf(size2: i32, underline: bool) -> String {
    let _ = size2;
    let sl = (answer_line_pt() * 20.0).round() as i32;
    if underline {
        format!("\\pard\\sl{sl}\\slmult0\\tqr\\tlul\\tx{}\\tab\\par\n", PAGE_TW.round() as i32)
    } else {
        format!("\\pard\\sl{sl}\\slmult0\\par\n")
    }
}

fn rtf_doc(doc: &Doc, size2: i32, underline: bool) -> String {
    let mut s = String::new();
    for b in parse(&doc.text) {
        match b {
            Block::Empty => s += "\\pard\\sa0\\par\n",
            Block::Rule => s += "\\pard\\brdrb\\brdrs\\brdrw10\\brsp20\\sa120\\par\n",
            Block::Lines(n) => {
                for _ in 0..n {
                    s += &answer_line_rtf(size2, underline);
                }
            }
            Block::Para { inl, bullet, heading } => {
                if heading {
                    s += &format!("\\pard\\sa100\\b\\fs{} {}\\b0\\fs{}\\par\n", size2 + 8, rtf_inl(doc, &inl, PAGE_TW), size2);
                } else if bullet {
                    s += &format!("\\pard\\li360\\fi-240\\sa40\\u8226?\\tab {}\\par\n", rtf_inl(doc, &inl, PAGE_TW));
                } else {
                    s += &format!("\\pard\\sa40 {}\\par\n", rtf_inl(doc, &inl, PAGE_TW));
                }
            }
            Block::Table { bordered: _, rows, cells } => {
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
                // twips per character: about 0.55 of the font size (half points x 10)
                let widths = crate::markup::column_widths(&weight, &short, PAGE_TW, size2 as f32 * 5.5, 200.0);
                for (ri, r) in rows.iter().enumerate() {
                    s += "\\trowd\\trgaph70";
                    let mut x = 0.0;
                    // the RTF cells of this row: (column with text, or None for the lower part of a merged cell; width)
                    let mut out_cells: Vec<(Option<usize>, f32)> = Vec::new();
                    let mut ci = 0;
                    while ci < cols {
                        let inf = cells[ri][ci];
                        let (start, span, border, merge) = if inf.covered() {
                            let (ar, ac) = crate::table::anchor_of(&cells, ri, ci);
                            if ar < ri && ac == ci {
                                (None, cells[ar][ac].colspan, cells[ar][ac].border, "\\clvmrg")
                            } else {
                                ci += 1;
                                continue;
                            }
                        } else {
                            (Some(ci), inf.colspan, inf.border, if inf.rowspan > 1 { "\\clvmgf" } else { "" })
                        };
                        let span = span.clamp(1, cols - ci);
                        let w: f32 = widths[ci..ci + span].iter().sum();
                        x += w;
                        s += merge;
                        if border {
                            s += "\\clbrdrt\\brdrs\\brdrw10\\clbrdrl\\brdrs\\brdrw10\\clbrdrb\\brdrs\\brdrw10\\clbrdrr\\brdrs\\brdrw10";
                        }
                        s += &format!("\\cellx{:.0}", x);
                        out_cells.push((start, w));
                        ci += span;
                    }
                    s += "\n";
                    for (start, w) in out_cells {
                        s += "\\pard\\intbl ";
                        if let Some(cell) = start.and_then(|c| r.get(c)) {
                            let parts: Vec<String> = cell.iter().map(|l| rtf_inl(doc, l, w - 140.0)).collect();
                            s += &parts.join("\\line ");
                        }
                        s += "\\cell\n";
                    }
                    s += "\\row\n";
                }
                s += "\\pard\\par\n";
            }
        }
    }
    s
}

pub fn to_rtf(db: &Database, c: &Composed, underline: bool) -> String {
    let size2 = (db.settings.print_size * 2.0).round() as i32;
    // own fonts of single tasks come after the three standard fonts (\f3, \f4, …)
    let mut extra: Vec<String> = Vec::new();
    for sec in &c.sections {
        if let Some((f, _)) = sec.own_font(db) {
            if f != db.settings.print_font && !extra.contains(&f) {
                extra.push(f);
            }
        }
    }
    let extra_tbl: String = extra.iter().enumerate().map(|(i, f)| format!("{{\\f{}\\fnil {};}}", i + 3, rtf_text(f))).collect();
    let mut s = format!(
        "{{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1{{\\fonttbl{{\\f0\\fswiss {};}}{{\\f1\\froman Times New Roman;}}{{\\f2\\fnil Segoe UI Symbol;}}{extra_tbl}}}\n{ctbl}\n\\paperw11906\\paperh16838\\margl{ml}\\margr{mr}\\margt{mt}\\margb{mb}\\f0\\fs{size2}\n",
        rtf_text(&db.settings.print_font),
        mt = mm_twips(PAGE_MARGINS_MM[0]),
        mr = mm_twips(PAGE_MARGINS_MM[1]),
        mb = mm_twips(PAGE_MARGINS_MM[2]),
        ml = mm_twips(PAGE_MARGINS_MM[3]),
        ctbl = rtf_colortbl(),
    );
    if c.page_numbers || !c.footer.is_empty() {
        s += "{\\footer\\pard\\qc\\fs16 ";
        s += &rtf_text(&c.footer);
        if c.page_numbers {
            s += "\\tab Seite {\\field{\\*\\fldinst PAGE}{\\fldrslt 1}} von {\\field{\\*\\fldinst NUMPAGES}{\\fldrslt 1}}";
        }
        s += "\\par}\n";
    }
    for sec in &c.sections {
        if let Some((h, p)) = &sec.heading {
            s += &format!(
                "\\pard\\keepn\\sb200\\sa80\\brdrb\\brdrs\\brdrw5\\tqr\\tx{}\\b\\fs{} {}\\b0\\tab {}\\fs{}\\par\n",
                PAGE_TW.round() as i32,
                size2 + 2,
                rtf_text(&crate::markup::plain(h)),
                rtf_text(p),
                size2
            );
        }
        match sec.own_font(db) {
            Some((f, sz)) => {
                let fi = extra.iter().position(|x| *x == f).map(|i| i + 3).unwrap_or(0);
                let sz2 = (sz * 2.0).round() as i32;
                s += &format!("{{\\f{fi}\\fs{sz2} ");
                s += &rtf_doc(&sec.doc, sz2, underline);
                for _ in 0..sec.lines {
                    s += &answer_line_rtf(sz2, underline || sec.underline);
                }
                s += "}\n";
            }
            None => {
                s += &rtf_doc(&sec.doc, size2, underline);
                for _ in 0..sec.lines {
                    s += &answer_line_rtf(size2, underline || sec.underline);
                }
            }
        }
    }
    s += "}";
    s
}

pub fn safe_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if s.is_empty() {
        "Dokument".into()
    } else {
        s
    }
}

/// Writes HTML to a temporary file and opens it in the default browser.
pub fn open_html(name: &str, html: &str) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join("Klassenwerk");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.html", safe_name(name)));
    std::fs::write(&path, html).map_err(|e| e.to_string())?;
    open_path(&path);
    Ok(path)
}

pub fn open_path(path: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

#[allow(dead_code)]
pub fn inline_preview(s: &str) -> Vec<Inl> {
    parse_inline(s)
}
