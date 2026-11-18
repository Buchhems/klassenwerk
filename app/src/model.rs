//! Data model: tasks, exams/worksheets, grading scales, print templates and
//! the database file.

use base64::Engine;
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

pub mod b64 {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ObjKind {
    /// PNG/JPEG/GIF/BMP bytes
    Image,
    /// Formula picture (Windows metafile) from the old formula editor
    Formula,
    /// New formula written in the formula editor; data = source text (UTF-8), see math.rs
    Math,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DocObject {
    pub id: u32,
    pub kind: ObjKind,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    /// display size in pixels (0 = natural size)
    pub width: f32,
    pub height: f32,
}

/// A rich text document: light markup plus embedded objects referenced as {{id}}.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Doc {
    pub text: String,
    pub objects: Vec<DocObject>,
}

impl Doc {
    pub fn plain(text: &str) -> Self {
        Doc { text: text.to_string(), objects: vec![] }
    }
    pub fn next_obj_id(&self) -> u32 {
        self.objects.iter().map(|o| o.id).max().unwrap_or(0) + 1
    }
    pub fn object(&self, id: u32) -> Option<&DocObject> {
        self.objects.iter().find(|o| o.id == id)
    }
    /// The text as the editor shows it: formulas of the formula editor written
    /// as `$source$`, a literal dollar sign as `\$`. Pictures and old formulas
    /// stay `{{n}}`; they are returned as the second value.
    pub fn to_dollar_text(&self) -> (String, Vec<DocObject>) {
        let mut out = String::new();
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '{' && chars.get(i + 1) == Some(&'{') {
                let digits: String = chars[i + 2..].iter().take_while(|c| c.is_ascii_digit()).collect();
                let j = i + 2 + digits.len();
                if !digits.is_empty() && chars.get(j) == Some(&'}') && chars.get(j + 1) == Some(&'}') {
                    if let Some(o) = digits.parse().ok().and_then(|id| self.object(id)).filter(|o| o.kind == ObjKind::Math) {
                        let src = String::from_utf8_lossy(&o.data).replace(['\n', '\r'], " ");
                        out += &format!("${}$", src.trim());
                        i = j + 2;
                        continue;
                    }
                }
            }
            if c == '$' && (i == 0 || chars[i - 1] != '\\') {
                out.push('\\');
            }
            out.push(c);
            i += 1;
        }
        let objects = self.objects.iter().filter(|o| o.kind != ObjKind::Math).cloned().collect();
        (out, objects)
    }

    /// Back from the editor's text: every `$…$` becomes a formula object, `\$` a
    /// dollar sign. A `$` only opens a formula when a non-space follows, and only
    /// closes one after a non-space ("5 $ und 3 $" stays text).
    pub fn from_dollar_text(text: &str, objects: &[DocObject]) -> Doc {
        let mut doc = Doc { text: String::new(), objects: objects.to_vec() };
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' && chars.get(i + 1) == Some(&'$') {
                doc.text.push('$');
                i += 2;
                continue;
            }
            if c == '$' && chars.get(i + 1).map(|n| !n.is_whitespace() && *n != '$').unwrap_or(false) {
                if let Some(j) = closing_dollar(&chars, i + 1) {
                    let src: String = chars[i + 1..j].iter().collect();
                    let d = crate::math::layout(&src);
                    let (w, h) = crate::math::size_px(&d);
                    let id = doc.next_obj_id();
                    doc.objects.push(DocObject { id, kind: ObjKind::Math, data: src.into_bytes(), width: w, height: h });
                    doc.text += &format!("{{{{{id}}}}}");
                    i = j + 1;
                    continue;
                }
            }
            doc.text.push(c);
            i += 1;
        }
        doc
    }

    /// Removes objects no longer referenced in the text.
    pub fn prune(&mut self) {
        let text = self.text.clone();
        self.objects.retain(|o| text.contains(&format!("{{{{{}}}}}", o.id)));
    }
}

/// Label of a Teilaufgabe at the start of a cell or line: "a)", "b.", "aa)" → "a", "b", "aa".
fn part_label(s: &str) -> Option<(String, usize)> {
    let t = s.trim_start();
    let lead = s.len() - t.len();
    let letters: String = t.chars().take_while(|c| c.is_ascii_lowercase()).collect();
    if letters.is_empty() || letters.len() > 2 {
        return None;
    }
    let rest = &t[letters.len()..];
    if rest.starts_with(')') || rest.starts_with(". ") || rest == "." {
        Some((letters.clone(), lead + letters.len() + 1))
    } else {
        None
    }
}

/// Points written as "2 P.", "2 P", "2 Punkte", "(2 P.)", "[2 BE]" → 2.
fn points_in(s: &str) -> Option<i32> {
    let t = s.trim().trim_matches(|c| c == '(' || c == ')' || c == '[' || c == ']').trim();
    let low = t.to_lowercase();
    for unit in ["punkte", "punkt", "p.", "p", "be"] {
        if let Some(n) = low.strip_suffix(unit) {
            if let Ok(v) = n.trim().replace(',', ".").parse::<f32>() {
                return Some(v.round() as i32);
            }
        }
    }
    None
}

fn table_cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim_start();
    let rest = t.strip_prefix("+|").or_else(|| t.strip_prefix('|'))?;
    let mut cells: Vec<String> = rest.split('|').map(|c| c.to_string()).collect();
    if cells.last().map(|c| c.trim().is_empty()).unwrap_or(false) {
        cells.pop();
    }
    Some(cells)
}

/// Teilaufgaben as they are written in a task text: table rows such as
/// "| a) | Text | 2 P. |" (as inserted by the toolbar) or lines such as
/// "a) Text (2 P.)". Lines without points are only taken in table form.
pub fn parts_from_text(text: &str) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    for line in text.lines() {
        let found = if let Some(cells) = table_cells(line) {
            let Some((label, _)) = cells.first().and_then(|c| part_label(c)).filter(|(_, n)| *n >= cells[0].trim_end().len()) else { continue };
            let points = cells.iter().skip(1).rev().find_map(|c| points_in(c)).unwrap_or(0);
            Some((label, points))
        } else if let Some((label, _)) = part_label(line) {
            // points at the end of the line: "(2 P.)"
            let t = line.trim_end();
            let open = t.rfind(['(', '[']);
            open.and_then(|o| points_in(&t[o..])).map(|p| (label, p))
        } else {
            None
        };
        if let Some((label, points)) = found {
            if !out.iter().any(|p| p.label == label) {
                out.push(Part { label, points, expectation: String::new() });
            }
        }
    }
    out
}

/// Writes the points of the Teilaufgaben into the task text: existing rows
/// "| a) | … | 2 P. |" and lines "a) … (2 P.)" get the current points, missing
/// Teilaufgaben are added as table rows at the end.
pub fn write_parts_into_text(text: &str, parts: &[Part]) -> String {
    let mut done: HashSet<String> = HashSet::new();
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let mut l = line.to_string();
        if let Some(cells) = table_cells(line) {
            if let Some(p) = cells.first().and_then(|c| part_label(c)).and_then(|(lab, _)| parts.iter().find(|p| p.label == lab)) {
                let mut cells = cells;
                match cells.iter().rposition(|c| points_in(c).is_some()) {
                    Some(i) if i > 0 => cells[i] = format!(" {} P. ", p.points),
                    _ => cells.push(format!(" {} P. ", p.points)),
                }
                let prefix = if line.trim_start().starts_with("+|") { "+|" } else { "|" };
                l = format!("{prefix}{}|", cells.join("|"));
                done.insert(p.label.clone());
            }
        } else if let Some(p) = part_label(line).and_then(|(lab, _)| parts.iter().find(|p| p.label == lab)) {
            let t = line.trim_end();
            if let Some(o) = t.rfind(['(', '[']).filter(|o| points_in(&t[*o..]).is_some()) {
                l = format!("{}({} P.)", &t[..o], p.points);
                done.insert(p.label.clone());
            }
        }
        lines.push(l);
    }
    let missing: Vec<&Part> = parts.iter().filter(|p| !done.contains(&p.label)).collect();
    if !missing.is_empty() {
        while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            lines.pop();
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        for p in missing {
            lines.push(format!("| {}) | … | {} P. |", p.label, p.points));
        }
    }
    lines.join("\n")
}

/// Position of the `$` that closes a formula starting at `from`: not escaped,
/// after a non-space, and before an empty line.
fn closing_dollar(chars: &[char], from: usize) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        match chars[j] {
            '\n' if chars.get(j + 1) == Some(&'\n') => return None,
            '\\' => j += 1,
            '$' if !chars[j - 1].is_whitespace() => return Some(j),
            _ => {}
        }
        j += 1;
    }
    None
}

/// The `$…$` formula around a cursor position in the editor's text:
/// (start, end) in chars, end after the closing `$`, and the formula source.
pub fn dollar_at(text: &str, cur: usize) -> Option<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == '$' && chars.get(i + 1).map(|n| !n.is_whitespace() && *n != '$').unwrap_or(false) {
            if let Some(j) = closing_dollar(&chars, i + 1) {
                if cur >= i && cur <= j + 1 {
                    return Some((i, j + 1, chars[i + 1..j].iter().collect()));
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    None
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Usage {
    pub date: String,
    pub sheet: String,
    pub exam: bool,
}

/// A part of a task (Teilaufgabe a, b, c) with its own points and the
/// expected answer (Erwartungshorizont). Not in the original.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Part {
    /// "a", "b", …
    pub label: String,
    pub points: i32,
    /// what a full answer contains (markup; may be empty)
    pub expectation: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Task {
    pub id: u64,
    pub subject: String,
    pub topic: String,
    pub subtopic: String,
    pub desc4: String,
    pub desc5: String,
    pub subject_line: String,
    pub points: i32,
    pub value1: i32,
    pub int3: i32,
    pub int4: i32,
    pub int5: i32,
    pub minutes: u32,
    /// 0 = not set, 1 = leicht, 2 = mittel, 3 = schwer
    pub difficulty: u8,
    pub favorite: bool,
    pub tags: Vec<String>,
    /// internal remarks for the teacher (never printed)
    pub notes: String,
    /// empty lines for handwritten answers below the task
    /// (XML field Int1 = "Leerzeilen": |Int1| lines, negative = underlined)
    pub answer_lines: u32,
    /// underline those lines (original: Int1 < 0, editor box "unterstreichen")
    pub underline_answer: bool,
    pub author_new: String,
    pub author_upd: String,
    pub created: Option<NaiveDateTime>,
    pub updated: Option<NaiveDateTime>,
    pub lesson: Option<NaiveDateTime>,
    pub exam: Option<NaiveDateTime>,
    pub first_used: Option<NaiveDateTime>,
    pub last_used: Option<NaiveDateTime>,
    pub exam_first: Option<NaiveDateTime>,
    pub exam_last: Option<NaiveDateTime>,
    pub indent: String,
    pub position: i32,
    pub usage: Vec<Usage>,
    pub task: Doc,
    pub solution: Doc,
    /// original stored bodies from an import (kept for a lossless XML export)
    #[serde(with = "b64")]
    pub orig_task: Vec<u8>,
    #[serde(with = "b64")]
    pub orig_solution: Vec<u8>,
    pub changed: bool,
    /// who created it outside the program, e.g. "KI-Assistent" (MCP server); empty = the teacher
    pub origin: String,
    /// Teilaufgaben; when there are any, `points` is their sum
    pub parts: Vec<Part>,
    /// random numbers for variants, used in the text as {=name} (see vary.rs)
    pub vars: Vec<crate::vary::Var>,
    /// own font for this task when printing (empty = the standard print font)
    pub font: String,
    /// own font size in pt (0 = the standard print size)
    pub font_size: f32,
}

/// Fonts offered for printing (standard and per task).
pub const PRINT_FONTS: [&str; 10] =
    ["Arial", "Calibri", "Times New Roman", "Cambria", "Segoe UI", "Verdana", "Georgia", "Comic Sans MS", "Courier New", "Tahoma"];

impl Task {
    /// Font and size for printing this task: its own or the standard.
    pub fn print_font<'a>(&'a self, st: &'a Settings) -> (&'a str, f32) {
        let f = if self.font.trim().is_empty() { st.print_font.as_str() } else { self.font.trim() };
        let s = if self.font_size > 0.0 { self.font_size } else { st.print_size };
        (f, s)
    }

    /// With Teilaufgaben the task's points are their sum.
    pub fn sync_points(&mut self) {
        if !self.parts.is_empty() {
            self.points = self.parts.iter().map(|p| p.points.max(0)).sum();
        }
    }
    /// Label for the next Teilaufgabe: a, b, … z, then aa, ab …
    pub fn next_part_label(&self) -> String {
        let n = self.parts.len();
        let letters: Vec<char> = ('a'..='z').collect();
        if n < 26 {
            letters[n].to_string()
        } else {
            format!("{}{}", letters[(n / 26 - 1).min(25)], letters[n % 26])
        }
    }
    pub fn title(&self) -> String {
        if !self.subtopic.trim().is_empty() {
            self.subtopic.clone()
        } else if !self.topic.trim().is_empty() {
            self.topic.clone()
        } else {
            "Neue Aufgabe".into()
        }
    }
    pub fn last_any_use(&self) -> Option<NaiveDateTime> {
        [self.last_used, self.exam_last].into_iter().flatten().max()
    }
    /// Key used to detect duplicates on import
    pub fn dup_key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.subject.to_lowercase(),
            self.topic.to_lowercase(),
            self.subtopic.to_lowercase(),
            self.created.map(|d| d.to_string()).unwrap_or_default(),
            self.task.text.chars().take(80).collect::<String>()
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum SheetItem {
    Task(u64),
    /// additional text between tasks (e.g. hints, a reading text)
    Text(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ResultRow {
    pub name: String,
    pub points: f32,
    /// student of the class list this result belongs to
    pub student: Option<u64>,
    /// did not take part (not counted in the grade overview)
    pub absent: bool,
    /// points per task / Teilaufgabe (key: `ScoreCol::key`); when entered, `points` is their sum
    pub scores: std::collections::BTreeMap<String, f32>,
}

/// One column of the per-task point entry: a task without Teilaufgaben or one Teilaufgabe.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreCol {
    /// stable key: task id, plus ":label" for a Teilaufgabe, plus "#n" for a task used twice
    pub key: String,
    /// what the teacher sees, e.g. "1", "2a"
    pub label: String,
    pub max: f32,
}

impl ResultRow {
    /// Recomputes the total from the points per task, if there are any.
    pub fn sum_scores(&mut self, cols: &[ScoreCol]) {
        if !self.scores.is_empty() {
            self.points = cols.iter().filter_map(|c| self.scores.get(&c.key)).sum::<f32>().max(0.0);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Sheet {
    pub id: u64,
    pub title: String,
    /// type, e.g. Klausur, Übung, Skript (freely definable)
    pub kind: String,
    pub subject: String,
    pub class_name: String,
    pub date: String,
    pub minutes: u32,
    pub items: Vec<SheetItem>,
    /// name of the grading scale ("" = none)
    pub scheme: String,
    /// name of the print template
    pub template: String,
    pub used_marked: bool,
    pub archived: bool,
    pub allow_duplicates: bool,
    pub print_lines: bool,
    pub underline_lines: bool,
    pub print_spiegel: bool,
    pub created: Option<NaiveDateTime>,
    pub results: Vec<ResultRow>,
    pub remarks: String,
    /// group of a parallel exam ("A", "B", …; empty = no groups)
    pub group: String,
    /// the sheet this one was made from as a parallel version (Gruppe A/B)
    pub parallel_of: Option<u64>,
    /// class (Klassenliste) that writes this sheet
    pub course_id: Option<u64>,
    /// weight in the grade book (1 = normal, 2 = counts double, 0 = not counted)
    pub weight: f32,
    /// who created it outside the program, e.g. "KI-Assistent" (MCP server)
    pub origin: String,
    /// enter results per task / Teilaufgabe instead of only the total
    pub detail_entry: bool,
    /// chooses the random numbers of tasks with variables (Aufgabenvarianten)
    pub seed: u64,
}

impl Default for Sheet {
    fn default() -> Self {
        Sheet {
            id: 0,
            title: String::new(),
            kind: "Klausur".into(),
            subject: String::new(),
            class_name: String::new(),
            date: String::new(),
            minutes: 90,
            items: vec![],
            scheme: "IHK".into(),
            template: "Klausur".into(),
            used_marked: false,
            archived: false,
            allow_duplicates: false,
            print_lines: true,
            underline_lines: true,
            print_spiegel: false,
            created: None,
            results: vec![],
            remarks: String::new(),
            group: String::new(),
            parallel_of: None,
            course_id: None,
            weight: 1.0,
            origin: String::new(),
            detail_entry: false,
            seed: 0,
        }
    }
}

impl Sheet {
    /// Results of everyone who took part (not marked as absent).
    pub fn present_results(&self) -> impl Iterator<Item = &ResultRow> {
        self.results.iter().filter(|r| !r.absent)
    }
    /// Title with the group, e.g. "Integralrechnung – Gruppe B".
    pub fn title_with_group(&self) -> String {
        if self.group.trim().is_empty() {
            self.title.clone()
        } else {
            format!("{} – Gruppe {}", self.title, self.group.trim())
        }
    }
    pub fn task_ids(&self) -> Vec<u64> {
        self.items
            .iter()
            .filter_map(|i| match i {
                SheetItem::Task(id) => Some(*id),
                _ => None,
            })
            .collect()
    }
    pub fn contains(&self, id: u64) -> bool {
        self.items.contains(&SheetItem::Task(id))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GradeRow {
    pub label: String,
    /// the grade is given from this percentage of the total points on
    pub min_percent: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GradeScheme {
    pub name: String,
    /// true when a higher label is a better grade (15-point scale)
    pub higher_is_better: bool,
    /// the first label that counts as "nicht bestanden"
    pub fail_from: String,
    pub rows: Vec<GradeRow>,
    /// smallest step of the point bounds: 0.5 (half points), 1 or 0.1
    #[serde(default = "default_step")]
    pub step: f32,
    /// heading above the printed key
    #[serde(default = "default_key_heading")]
    pub heading: String,
    /// print the percentages too
    #[serde(default)]
    pub show_percent: bool,
    /// print an empty row "Anzahl" to fill in by hand
    #[serde(default = "yes")]
    pub count_row: bool,
}

fn default_step() -> f32 {
    0.5
}
fn default_key_heading() -> String {
    "Notenschlüssel".into()
}
fn yes() -> bool {
    true
}

/// Brings grading keys of older versions up to date:
/// - the shipped keys of earlier preview versions are replaced by the current ones
///   (IHK keeps its steps; Gymnasium gets the published Zentralabitur steps);
/// - keys of the first previews are renamed and the sheets that use them relinked;
/// - keys written in points instead of percent are converted to percent.
pub fn migrate_schemes(db: &mut Database) {
    let fresh = GradeScheme::defaults();
    let old_gym = [96.0, 91.0, 86.0, 81.0, 76.0, 71.0, 66.0, 61.0, 56.0, 51.0, 46.0, 41.0, 34.0, 27.0, 20.0, 0.0];
    for (old, new_name) in [("Noten 1–6 (IHK)", "IHK"), ("15 Punkte (Oberstufe)", "Gymnasium")] {
        if db.settings.schemes.iter().any(|s| s.name == new_name) {
            continue;
        }
        if let Some(pos) = db.settings.schemes.iter().position(|s| s.name == old) {
            if let Some(n) = fresh.iter().find(|s| s.name == new_name) {
                db.settings.schemes[pos] = n.clone();
                for sh in db.sheets.iter_mut() {
                    if sh.scheme == old {
                        sh.scheme = new_name.to_string();
                    }
                }
            }
        }
    }
    for s in db.settings.schemes.iter_mut() {
        let p: Vec<f32> = s.rows.iter().map(|r| r.min_percent).collect();
        if s.name == "Gymnasium" && p == old_gym {
            if let Some(n) = fresh.iter().find(|f| f.name == "Gymnasium") {
                s.rows = n.rows.clone();
            }
        }
        let max = p.iter().cloned().fold(0.0f32, f32::max);
        if max > 100.0 {
            for r in s.rows.iter_mut() {
                r.min_percent = (r.min_percent * 100.0 / max * 10.0).round() / 10.0;
            }
        }
    }
}

/// Steps a key can use for its point bounds.
pub const KEY_STEPS: [(f32, &str); 3] = [(0.5, "halbe Punkte"), (1.0, "ganze Punkte"), (0.1, "Zehntelpunkte")];

impl GradeScheme {
    /// Keys that come with Klassenwerk. The steps are published scales:
    /// IHK: the nationwide key of the chambers of commerce (100-point key);
    /// Gymnasium: the assignment of raw points to 15 Notenpunkte of the NRW Zentralabitur.
    pub fn defaults() -> Vec<GradeScheme> {
        let key = |name: &str, hib: bool, fail: &str, rows: &[(&str, f32)]| GradeScheme {
            name: name.into(),
            higher_is_better: hib,
            fail_from: fail.into(),
            rows: rows.iter().map(|(l, p)| GradeRow { label: (*l).into(), min_percent: *p }).collect(),
            step: 0.5,
            heading: default_key_heading(),
            show_percent: false,
            count_row: true,
        };
        vec![
            key("IHK", false, "5", &[("1", 92.0), ("2", 81.0), ("3", 67.0), ("4", 50.0), ("5", 30.0), ("6", 0.0)]),
            key(
                "Gymnasium",
                true,
                "4",
                &[
                    ("15", 95.0),
                    ("14", 90.0),
                    ("13", 85.0),
                    ("12", 80.0),
                    ("11", 75.0),
                    ("10", 70.0),
                    ("9", 65.0),
                    ("8", 60.0),
                    ("7", 55.0),
                    ("6", 50.0),
                    ("5", 45.0),
                    ("4", 40.0),
                    ("3", 33.0),
                    ("2", 27.0),
                    ("1", 20.0),
                    ("0", 0.0),
                ],
            ),
            key(
                "Noten mit Tendenz (1+ bis 6)",
                false,
                "5+",
                &[
                    ("1+", 97.0),
                    ("1", 94.0),
                    ("1-", 91.0),
                    ("2+", 87.0),
                    ("2", 84.0),
                    ("2-", 81.0),
                    ("3+", 77.0),
                    ("3", 72.0),
                    ("3-", 67.0),
                    ("4+", 62.0),
                    ("4", 56.0),
                    ("4-", 50.0),
                    ("5+", 40.0),
                    ("5", 30.0),
                    ("5-", 20.0),
                    ("6", 0.0),
                ],
            ),
        ]
    }

    /// The step, limited to sensible values.
    pub fn step(&self) -> f64 {
        if self.step >= 0.05 {
            self.step as f64
        } else {
            0.5
        }
    }

    /// (label, from points, to points) for a total, best grade first.
    ///
    /// A grade needs at least its percentage of the total. The needed points are
    /// rounded **up** to the key's step (half points by default), so nobody gets a
    /// grade with less than its percentage. A grade ends one step below the start
    /// of the next better one; the best grade ends at the total. A grade whose
    /// range is empty (possible with few points) gets from > to; see `lost_grades`.
    pub fn rows_for(&self, total: f32) -> Vec<(String, f32, f32)> {
        let total = total.max(0.0) as f64;
        let step = self.step();
        let mut rows = self.rows.clone();
        rows.sort_by(|a, b| b.min_percent.partial_cmp(&a.min_percent).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = Vec::new();
        let mut upper = total;
        for r in rows {
            let exact = total * (r.min_percent.clamp(0.0, 100.0) as f64) / 100.0;
            // up to the next step; a tiny tolerance keeps 27.000000001 at 27
            let from = ((exact / step) - 1e-9).ceil().max(0.0) * step;
            let from = from.min(total);
            out.push((r.label.clone(), from as f32, upper as f32));
            upper = from - step;
        }
        out
    }

    /// Grades that cannot be reached with this total because their range is empty.
    pub fn lost_grades(&self, total: f32) -> Vec<String> {
        self.rows_for(total).into_iter().filter(|(_, from, to)| from > to).map(|(l, _, _)| l).collect()
    }

    /// Index (in `rows_for` order) of the grade for `pts`: the best grade whose
    /// lower bound is reached.
    pub fn grade_for(&self, total: f32, pts: f32) -> Option<usize> {
        self.rows_for(total).iter().position(|(_, from, to)| from <= to && pts as f64 + 1e-6 >= *from as f64)
    }

    /// numeric value of a label: "2-" -> 2.33, "13" -> 13
    pub fn value(label: &str) -> f32 {
        let base: String = label.chars().take_while(|c| c.is_ascii_digit()).collect();
        let v: f32 = base.parse().unwrap_or(0.0);
        if label.ends_with('+') {
            v - 0.33
        } else if label.ends_with('-') {
            v + 0.33
        } else {
            v
        }
    }
}

/// Print template with placeholders like {Titel}, {Fach}, {Klasse}, {Datum}.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Template {
    pub name: String,
    pub header: String,
    pub task_heading: String,
    pub show_points: bool,
    pub scale_table: bool,
    pub page_numbers: bool,
    pub closing: String,
}

impl Default for Template {
    fn default() -> Self {
        Template::defaults().remove(0)
    }
}

pub const PLACEHOLDERS: [(&str, &str); 15] = [
    ("{Logo}", "Schullogo"),
    ("{Schule}", "Name der Schule"),
    ("{Adresse}", "Adresse der Schule"),
    ("{Lehrkraft}", "Name der Lehrkraft"),
    ("{Titel}", "Titel der Klausur/Übung"),
    ("{Gruppe}", "Gruppe einer Parallelklausur, z. B. „Gruppe B“ (leer ohne Gruppen)"),
    ("{Typ}", "Art, z. B. Klausur"),
    ("{Fach}", "Fach"),
    ("{Klasse}", "Klasse/Kurs"),
    ("{Datum}", "Datum"),
    ("{Dauer}", "Bearbeitungszeit in Minuten"),
    ("{Gesamtpunkte}", "Summe der Punkte"),
    ("{Anzahl}", "Anzahl der Aufgaben"),
    ("{Nr}", "Nummer der Aufgabe (nur Aufgabenüberschrift)"),
    ("{Aufgabentitel}", "Unterthema der Aufgabe (nur Aufgabenüberschrift)"),
];

impl Template {
    pub fn defaults() -> Vec<Template> {
        vec![
            Template {
                name: "Klausur".into(),
                header: "{Logo}  **{Schule}**\n# {Typ}: {Titel}\nFach: {Fach}   ·   Klasse: {Klasse}   ·   Datum: {Datum}   ·   Bearbeitungszeit: {Dauer} Min.\n---\nName: ______________________________     Punkte: ______ / {Gesamtpunkte}     Note: ______\n".into(),
                task_heading: "Aufgabe {Nr}".into(),
                show_points: true,
                scale_table: true,
                page_numbers: true,
                closing: "Viel Erfolg!".into(),
            },
            Template {
                name: "Übung".into(),
                header: "{Logo}  **{Schule}**\n# {Titel}\n{Fach} · {Klasse} · {Datum}\n---\nName: ______________________________\n".into(),
                task_heading: "Übung {Nr}: {Aufgabentitel}".into(),
                show_points: false,
                scale_table: false,
                page_numbers: true,
                closing: String::new(),
            },
            Template {
                name: "Skript".into(),
                header: "# {Titel}\n{Fach} · {Lehrkraft} · {Datum}\n---\n".into(),
                task_heading: "{Nr}. {Aufgabentitel}".into(),
                show_points: false,
                scale_table: false,
                page_numbers: true,
                closing: String::new(),
            },
        ]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SheetType {
    pub name: String,
    /// counts as an exam (usage is recorded as exam use)
    pub exam: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub dark: bool,
    pub teacher: String,
    pub school: String,
    pub school_address: String,
    pub footer: String,
    /// school logo (PNG bytes), shown in the app and on printed sheets
    #[serde(with = "b64")]
    pub logo: Vec<u8>,
    pub side_by_side: bool,
    pub show_solution: bool,
    pub print_font: String,
    pub print_size: f32,
    pub view_size: f32,
    pub schemes: Vec<GradeScheme>,
    pub templates: Vec<Template>,
    pub types: Vec<SheetType>,
    /// spell checking in the task editor
    pub spell_check: bool,
    /// Hunspell dictionary (.dic); empty = look for an installed German one
    pub spell_dict: String,
    /// own words that always count as correct
    pub user_words: Vec<String>,
    /// synonyms in the editor's right-click menu
    pub thesaurus: bool,
    /// MyThes thesaurus (.dat); empty = look for an installed German one
    pub thesaurus_file: String,
    /// shared folder (network drive, cloud folder) for exchanging tasks with colleagues
    pub exchange_dir: String,
    /// look for a new version on GitHub when the program starts (at most once a day)
    pub update_check: bool,
    /// day of the last check (YYYY-MM-DD)
    pub update_last: String,
    /// version the teacher chose to skip, e.g. "v1.3.0"
    pub update_skip: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark: false,
            teacher: "Lehrer".into(),
            school: "Musterschule".into(),
            school_address: String::new(),
            footer: String::new(),
            logo: vec![],
            side_by_side: true,
            show_solution: true,
            print_font: "Arial".into(),
            print_size: 12.0,
            view_size: 15.0,
            schemes: GradeScheme::defaults(),
            templates: Template::defaults(),
            types: vec![
                SheetType { name: "Klausur".into(), exam: true },
                SheetType { name: "Klassenarbeit".into(), exam: true },
                SheetType { name: "Test".into(), exam: true },
                SheetType { name: "Übung".into(), exam: false },
                SheetType { name: "Arbeitsblatt".into(), exam: false },
                SheetType { name: "Skript".into(), exam: false },
                SheetType { name: "Hausaufgabe".into(), exam: false },
            ],
            spell_check: true,
            spell_dict: String::new(),
            user_words: vec![],
            thesaurus: true,
            thesaurus_file: String::new(),
            exchange_dir: String::new(),
            update_check: true,
            update_last: String::new(),
            update_skip: String::new(),
        }
    }
}

impl Settings {
    pub fn scheme(&self, name: &str) -> Option<&GradeScheme> {
        self.schemes.iter().find(|s| s.name == name)
    }
    pub fn template(&self, name: &str) -> Template {
        self.templates.iter().find(|t| t.name == name).or(self.templates.first()).cloned().unwrap_or_default()
    }
    pub fn is_exam(&self, kind: &str) -> bool {
        self.types.iter().find(|t| t.name == kind).map(|t| t.exam).unwrap_or(false)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Database {
    pub version: u32,
    pub next_id: u64,
    pub tasks: Vec<Task>,
    pub sheets: Vec<Sheet>,
    pub settings: Settings,
    /// classes with their students (Klassenlisten)
    pub courses: Vec<crate::classes::Course>,
}

impl Database {
    pub fn new_id(&mut self) -> u64 {
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
    pub fn task_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }
    pub fn sheet_total(&self, s: &Sheet) -> i32 {
        s.task_ids().iter().filter_map(|id| self.task(*id)).map(|t| t.points.max(0)).sum()
    }

    /// Columns for entering points per task: one per Teilaufgabe, or one per task
    /// without Teilaufgaben, numbered as on the printed sheet ("1", "2a", "2b").
    pub fn score_columns(&self, s: &Sheet) -> Vec<ScoreCol> {
        let mut out = Vec::new();
        let mut seen: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for (k, id) in s.task_ids().into_iter().enumerate() {
            let Some(t) = self.task(id) else { continue };
            let nr = k + 1;
            let n = seen.entry(id).or_insert(0);
            *n += 1;
            let base = if *n == 1 { id.to_string() } else { format!("{id}#{n}") };
            if t.parts.is_empty() {
                out.push(ScoreCol { key: base, label: nr.to_string(), max: t.points.max(0) as f32 });
            } else {
                for p in &t.parts {
                    out.push(ScoreCol { key: format!("{base}:{}", p.label), label: format!("{nr}{}", p.label), max: p.points.max(0) as f32 });
                }
            }
        }
        out
    }

    /// Windows: %APPDATA%; macOS: ~/Library/Application Support; Linux: $XDG_DATA_HOME or ~/.local/share
    fn base_dir() -> PathBuf {
        if let Ok(a) = std::env::var("APPDATA") {
            return PathBuf::from(a);
        }
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
        if cfg!(target_os = "macos") {
            return home.join("Library").join("Application Support");
        }
        match std::env::var("XDG_DATA_HOME") {
            Ok(x) if !x.is_empty() => PathBuf::from(x),
            _ => home.join(".local").join("share"),
        }
    }

    pub fn path() -> PathBuf {
        Self::base_dir().join(DATA_FOLDER).join("datenbank.json")
    }

    /// The program was called "KlDozent" until November 2026. On the first start
    /// under the new name the data (database and backups) is copied from the old
    /// folder; the old folder stays as it is, with a note where the data went.
    pub fn migrate_old_data() {
        Self::migrate_from(&Self::base_dir().join(OLD_DATA_FOLDER), &Self::base_dir().join(DATA_FOLDER));
    }

    pub fn migrate_from(old: &std::path::Path, new: &std::path::Path) -> bool {
        if new.join("datenbank.json").exists() || !old.join("datenbank.json").exists() {
            return false;
        }
        if std::fs::create_dir_all(new).is_err() {
            return false;
        }
        let Ok(rd) = std::fs::read_dir(old) else { return false };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("datenbank") && name.ends_with(".json") {
                let _ = std::fs::copy(e.path(), new.join(&name));
            }
        }
        let _ = std::fs::write(
            old.join("UMGEZOGEN.txt"),
            format!(
                "Das Programm heißt jetzt Klassenwerk.\r\nDie Daten wurden am {} kopiert nach:\r\n{}\r\n\r\nDieser Ordner wird nicht mehr benutzt und kann gelöscht werden, sobald Klassenwerk läuft.\r\n",
                chrono::Local::now().format("%d.%m.%Y %H:%M"),
                new.display()
            ),
        );
        new.join("datenbank.json").exists()
    }

    /// Reads a data file without touching the backups (for the MCP server and for
    /// picking up its changes in the running program).
    pub fn load_plain(p: &std::path::Path) -> Option<Database> {
        let s = std::fs::read_to_string(p).ok()?;
        serde_json::from_str(&s).ok()
    }

    /// Modification time of the data file (to notice changes by the MCP server).
    pub fn file_stamp() -> Option<std::time::SystemTime> {
        std::fs::metadata(Self::path()).and_then(|m| m.modified()).ok()
    }

    /// Id for entries created outside the program (MCP server). They come from a
    /// separate range, so they never clash with ids the running program hands out.
    pub fn new_external_id(&self) -> u64 {
        const BASE: u64 = 1_000_000_000;
        let max = self.tasks.iter().map(|t| t.id).chain(self.sheets.iter().map(|s| s.id)).filter(|id| *id >= BASE).max();
        max.map(|m| m + 1).unwrap_or(BASE)
    }

    /// Takes over entries the MCP server added to the data file (tasks and sheets
    /// marked with an origin that this database does not have yet). Each id is taken
    /// over only once per session, so an entry deleted by the teacher stays deleted.
    pub fn merge_external(&mut self, file: &Database, known: &mut HashSet<u64>) -> (usize, usize) {
        let mut n = (0, 0);
        for t in file.tasks.iter().filter(|t| !t.origin.is_empty()) {
            if self.task(t.id).is_none() && known.insert(t.id) {
                self.tasks.push(t.clone());
                n.0 += 1;
            }
        }
        for s in file.sheets.iter().filter(|s| !s.origin.is_empty()) {
            if !self.sheets.iter().any(|x| x.id == s.id) && known.insert(s.id) {
                self.sheets.push(s.clone());
                n.1 += 1;
            }
        }
        n
    }

    /// The automatic backups (newest first): path and time of the last change.
    pub fn backup_files() -> Vec<(std::path::PathBuf, Option<std::time::SystemTime>)> {
        let p = Self::path();
        (1..=9)
            .map(|i| p.with_file_name(format!("datenbank.{i}.json")))
            .filter(|f| f.exists())
            .map(|f| {
                let t = std::fs::metadata(&f).and_then(|m| m.modified()).ok();
                (f, t)
            })
            .collect()
    }

    /// Reads a backup (automatic generation or saved file).
    pub fn read_backup(p: &std::path::Path) -> Result<Database, String> {
        let s = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|_| "Die Datei ist keine gültige Sicherung.".to_string())
    }

    /// Takes over the chosen tasks, sheets and classes from a backup: entries that
    /// still exist are replaced, missing ones are added. The tasks of a chosen sheet
    /// come along when they are missing now. Returns how many of each were taken over.
    pub fn restore_part(&mut self, src: &Database, tasks: &HashSet<u64>, sheets: &HashSet<u64>, courses: &HashSet<u64>) -> (usize, usize, usize) {
        let mut task_ids: HashSet<u64> = tasks.clone();
        for s in src.sheets.iter().filter(|s| sheets.contains(&s.id)) {
            for id in s.task_ids() {
                if self.task(id).is_none() {
                    task_ids.insert(id);
                }
            }
        }
        let mut n = (0, 0, 0);
        for t in src.tasks.iter().filter(|t| task_ids.contains(&t.id)) {
            match self.tasks.iter_mut().find(|x| x.id == t.id) {
                Some(x) => *x = t.clone(),
                None => self.tasks.push(t.clone()),
            }
            n.0 += 1;
        }
        for s in src.sheets.iter().filter(|s| sheets.contains(&s.id)) {
            match self.sheets.iter_mut().find(|x| x.id == s.id) {
                Some(x) => *x = s.clone(),
                None => self.sheets.push(s.clone()),
            }
            n.1 += 1;
        }
        for c in src.courses.iter().filter(|c| courses.contains(&c.id)) {
            match self.courses.iter_mut().find(|x| x.id == c.id) {
                Some(x) => *x = c.clone(),
                None => self.courses.push(c.clone()),
            }
            n.2 += 1;
        }
        // ids of the backup must not be given out again
        let max = src
            .tasks
            .iter()
            .map(|t| t.id)
            .chain(src.sheets.iter().map(|s| s.id))
            .chain(src.courses.iter().map(|c| c.id))
            .filter(|id| *id < 1_000_000_000)
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(max + 1);
        n
    }

    pub fn load() -> Option<Database> {
        let s = std::fs::read_to_string(Self::path()).ok()?;
        let db: Database = serde_json::from_str(&s).ok()?;
        Self::rotate_backups(BACKUP_GENERATIONS);
        Some(db)
    }

    /// Keeps `n` generations of the data file from earlier sessions
    /// (datenbank.1.json is the newest).
    pub fn rotate_backups(n: u32) {
        let p = Self::path();
        let gen = |i: u32| p.with_file_name(format!("datenbank.{i}.json"));
        if !p.exists() {
            return;
        }
        let _ = std::fs::remove_file(gen(n));
        for i in (1..n).rev() {
            if gen(i).exists() {
                let _ = std::fs::rename(gen(i), gen(i + 1));
            }
        }
        let _ = std::fs::copy(&p, gen(1));
    }

    pub fn save_to(&self, p: &std::path::Path) -> Result<(), String> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let s = serde_json::to_string(self).map_err(|e| e.to_string())?;
        // own temp file per process: the program and the MCP server may save at the same time
        let tmp = p.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, s).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, p).map_err(|e| e.to_string())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    /// Database with the example tasks and one example exam.
    pub fn with_samples() -> Database {
        let mut db = Database { version: 2, next_id: 1, ..Default::default() };
        db.tasks = sample_tasks(&mut db);
        let ids: Vec<u64> = db.tasks.iter().filter(|t| t.topic == "Integral").take(3).map(|t| t.id).collect();
        let id = db.new_id();
        db.sheets.push(Sheet {
            id,
            title: "Integralrechnung".into(),
            kind: "Klausur".into(),
            subject: "Mathematik".into(),
            class_name: "12a".into(),
            date: chrono::Local::now().format("%d.%m.%Y").to_string(),
            minutes: 90,
            items: ids.into_iter().map(SheetItem::Task).collect(),
            created: Some(chrono::Local::now().naive_local()),
            ..Default::default()
        });
        db
    }

    /// Renames a subject/topic/subtopic/tag everywhere. Returns the number of changes.
    pub fn rename(&mut self, field: usize, from: &str, to: &str) -> usize {
        let mut n = 0;
        for t in &mut self.tasks {
            let f: Vec<&mut String> = match field {
                0 => vec![&mut t.subject],
                1 => vec![&mut t.topic],
                2 => vec![&mut t.subtopic],
                _ => t.tags.iter_mut().collect(),
            };
            for s in f {
                if s.as_str() == from {
                    *s = to.to_string();
                    n += 1;
                }
            }
            t.tags.dedup();
        }
        if field == 0 {
            for s in &mut self.sheets {
                if s.subject == from {
                    s.subject = to.to_string();
                }
            }
        }
        n
    }
}

/// Folder of the data file below %APPDATA%.
pub const DATA_FOLDER: &str = "Klassenwerk";
/// Folder used before the program was renamed (KlDozent).
pub const OLD_DATA_FOLDER: &str = "KlDozent";

/// Backup generations of the data file.
pub const BACKUP_GENERATIONS: u32 = 5;

/// Example tasks shipped with the program, written for Klassenwerk (format: as for the
/// MCP tool aufgabe_anlegen, see `task_from_spec`).
pub const SAMPLES: &str = include_str!("../samples/beispielaufgaben.json");

/// The example tasks (see `SAMPLES`), with new ids.
pub fn sample_tasks(db: &mut Database) -> Vec<Task> {
    let list: Vec<serde_json::Value> = serde_json::from_str(SAMPLES).unwrap_or_default();
    let now = chrono::Local::now().naive_local();
    list.iter()
        .filter_map(|v| task_from_spec(v).ok())
        .map(|(mut t, _)| {
            t.id = db.new_id();
            t.author_new = "Klassenwerk".into();
            t.author_upd = "Klassenwerk".into();
            t.created = Some(now);
            t.updated = Some(now);
            t.indent = "L".into();
            t
        })
        .collect()
}

/// A task from a description such as
/// `{"fach": "Mathematik", "schlagwort1": "Integral", "punkte": 4, "aufgabe": "Berechne $x^2$ …",
///   "loesung": "…", "teilaufgaben": [{"bezeichnung": "a", "punkte": 2, "erwartung": "…"}],
///   "zufallszahlen": [{"name": "a", "von": 1, "bis": 9, "schritt": 1}]}`.
/// Formulas are written as `$…$`. Used for the example tasks and by the MCP server.
/// Returns the task (without id) and the formula parts that were not understood.
pub fn task_from_spec(a: &serde_json::Value) -> Result<(Task, Vec<String>), String> {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    let n = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_f64());
    let fach = s(a, "fach").ok_or("fach fehlt.")?;
    let sw1 = s(a, "schlagwort1").ok_or("schlagwort1 fehlt.")?;
    let text = s(a, "aufgabe").ok_or("aufgabe (Text) fehlt.")?;
    let points = n(a, "punkte").ok_or("punkte fehlt.")?.round() as i32;
    if !(0..=1000).contains(&points) {
        return Err("punkte muss zwischen 0 und 1000 liegen.".into());
    }
    let (task_doc, mut problems) = crate::mcp::doc_from_assistant(&text);
    let sol_text = s(a, "loesung").unwrap_or_default();
    let (sol_doc, p2) = crate::mcp::doc_from_assistant(&sol_text);
    problems.extend(p2);
    let mut t = Task {
        subject: fach,
        topic: sw1,
        subtopic: s(a, "schlagwort2").unwrap_or_default(),
        points,
        difficulty: match s(a, "schwierigkeit").as_deref() {
            Some("leicht") => 1,
            Some("mittel") => 2,
            Some("schwer") => 3,
            _ => 0,
        },
        minutes: n(a, "minuten").map(|m| m.max(0.0).round() as u32).unwrap_or(0),
        answer_lines: n(a, "leerzeilen").map(|m| m.clamp(0.0, 60.0).round() as u32).unwrap_or(0),
        tags: a
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|v| v.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default(),
        task: task_doc,
        solution: sol_doc,
        changed: true,
        ..Default::default()
    };
    if let Some(vars) = a.get("zufallszahlen").and_then(|v| v.as_array()) {
        for (k, x) in vars.iter().enumerate() {
            let name: String = s(x, "name").unwrap_or_default().chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
            if name.is_empty() || !name.chars().next().unwrap().is_alphabetic() {
                return Err(format!("zufallszahlen[{k}]: name muss mit einem Buchstaben beginnen."));
            }
            let (min, max) = (n(x, "von").ok_or(format!("zufallszahlen[{k}]: von fehlt."))?, n(x, "bis").ok_or(format!("zufallszahlen[{k}]: bis fehlt."))?);
            let step = n(x, "schritt").unwrap_or(1.0);
            if max < min || step <= 0.0 {
                return Err(format!("zufallszahlen[{k}]: bis muss ≥ von und schritt > 0 sein."));
            }
            t.vars.push(crate::vary::Var { name, min, max, step });
        }
        let all = format!("{text}\n{sol_text}");
        problems.extend(crate::vary::problems(&all, &crate::vary::values(&t, 0)).into_iter().map(|p| format!("Zufallszahl: {p}")));
    }
    if let Some(parts) = a.get("teilaufgaben").and_then(|v| v.as_array()) {
        for (k, p) in parts.iter().enumerate() {
            let label = s(p, "bezeichnung").map(|l| l.trim().trim_end_matches(')').to_string()).filter(|l| !l.is_empty());
            let label = label.unwrap_or_else(|| t.next_part_label());
            let points = n(p, "punkte").ok_or(format!("teilaufgaben[{k}]: punkte fehlt."))?.round() as i32;
            if !(0..=1000).contains(&points) {
                return Err(format!("teilaufgaben[{k}]: punkte muss zwischen 0 und 1000 liegen."));
            }
            t.parts.push(Part { label, points, expectation: s(p, "erwartung").unwrap_or_default() });
        }
        t.sync_points();
    }
    Ok((t, problems))
}

/// Parses the old date format, e.g. 20080714T17:08:45000; the zero date means "never".
pub fn parse_date(s: &str) -> Option<NaiveDateTime> {
    if s.len() < 8 || s.starts_with("18991230") {
        return None;
    }
    let d = NaiveDate::parse_from_str(&s[..8], "%Y%m%d").ok()?;
    let (mut h, mut m, mut sec) = (0, 0, 0);
    if s.len() >= 17 && &s[8..9] == "T" {
        h = s[9..11].parse().unwrap_or(0);
        m = s[12..14].parse().unwrap_or(0);
        sec = s[15..17].parse().unwrap_or(0);
    }
    d.and_hms_opt(h, m, sec)
}

fn write_date(d: &Option<NaiveDateTime>) -> String {
    match d {
        Some(d) => d.format("%Y%m%dT%H:%M:%S000").to_string(),
        None => "18991230".into(),
    }
}

pub fn format_date(d: &Option<NaiveDateTime>) -> String {
    match d {
        Some(d) => d.format("%d.%m.%Y").to_string(),
        None => "–".into(),
    }
}

/// The old program stores text fields UTF-8 encoded twice ("Ã¤" for "ä").
/// Repairs such strings; leaves normal text unchanged.
pub fn fix_double_utf8(s: &str) -> String {
    if s.is_ascii() || s.chars().any(|c| c as u32 > 0xFF) {
        return s.to_string();
    }
    let bytes: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
    match String::from_utf8(bytes) {
        Ok(fixed) => fixed,
        Err(_) => s.to_string(),
    }
}

/// Encodes text the way the old program expects it in its XML files.
fn double_utf8(s: &str) -> String {
    s.as_bytes().iter().map(|&b| b as char).collect()
}

/// Imports a task export (XML data packet) into tasks.
pub fn import_xml(xml: &str, db: &mut Database) -> Result<Vec<Task>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("Keine gültige XML-Datei: {e}"))?;
    let mut out = Vec::new();
    for row in doc.descendants().filter(|n| n.has_tag_name("ROW")) {
        let a = |k: &str| fix_double_utf8(row.attribute(k).unwrap_or(""));
        let i = |k: &str| row.attribute(k).and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
        let raw = |k: &str| -> Vec<u8> {
            let raw = row.attribute(k).unwrap_or("");
            let cleaned: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
            base64::engine::general_purpose::STANDARD.decode(cleaned).unwrap_or_default()
        };
        let (b1, b2) = (raw("Body1"), raw("Body2"));
        let doc_of = |b: &[u8]| if b.is_empty() { Doc::default() } else { crate::rvf::to_doc(b) };
        let id = db.new_id();
        let t = Task {
            id,
            subject: a("Desc1"),
            topic: a("Desc2"),
            subtopic: a("Desc3"),
            desc4: a("Desc4"),
            desc5: a("Desc5"),
            subject_line: a("Subject"),
            value1: i("Int1"),
            answer_lines: i("Int1").unsigned_abs(),
            underline_answer: i("Int1") < 0,
            points: i("Int2"),
            int3: i("Int3"),
            int4: i("Int4"),
            int5: i("Int5"),
            author_new: a("Autornew"),
            author_upd: a("Autorupd"),
            created: parse_date(&a("Datetimenew")),
            updated: parse_date(&a("Datetimeupd")),
            lesson: parse_date(&a("Lesson")),
            exam: parse_date(&a("Clausur")),
            first_used: parse_date(&a("Datefirstused")),
            last_used: parse_date(&a("Datelastused")),
            exam_first: parse_date(&a("Clausurfirst")),
            exam_last: parse_date(&a("Clausurlast")),
            indent: a("Indent"),
            position: i("Position"),
            task: doc_of(&b1),
            solution: doc_of(&b2),
            orig_task: b1,
            orig_solution: b2,
            ..Default::default()
        };
        out.push(t);
    }
    if out.is_empty() {
        return Err("Die Datei enthält keine Aufgaben.".into());
    }
    Ok(out)
}

fn xml_attr(s: &str) -> String {
    double_utf8(s).replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\n', "&#10;")
}

/// Exports tasks in the same XML data packet format the old program uses.
/// Start of an XML file in the format of KlasseDozent: the field list of the data packet.
const XML_HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>  <DATAPACKET Version="2.0"><METADATA><FIELDS><FIELD attrname="Clausur" fieldtype="dateTime"><PARAM Name="PROVFLAGS" Value="7" Type="i4" Roundtrip="True"/></FIELD><FIELD attrname="Lesson" fieldtype="dateTime"><PARAM Name="PROVFLAGS" Value="7" Type="i4" Roundtrip="True"/></FIELD><FIELD attrname="Indent" fieldtype="string.uni" WIDTH="2"><PARAM Name="PROVFLAGS" Value="7" Type="i4" Roundtrip="True"/></FIELD><FIELD attrname="Position" fieldtype="i4"/><FIELD attrname="Autornew" fieldtype="string.uni" WIDTH="50"/><FIELD attrname="Autorupd" fieldtype="string.uni" WIDTH="50"/><FIELD attrname="Datetimenew" fieldtype="dateTime"/><FIELD attrname="Datetimeupd" fieldtype="dateTime"/><FIELD attrname="Datefirstused" fieldtype="dateTime"/><FIELD attrname="Datelastused" fieldtype="dateTime"/><FIELD attrname="Clausurfirst" fieldtype="dateTime"/><FIELD attrname="Clausurlast" fieldtype="dateTime"/><FIELD attrname="Subject" fieldtype="string.uni" WIDTH="100"/><FIELD attrname="Desc1" fieldtype="string.uni" WIDTH="60"/><FIELD attrname="Desc2" fieldtype="string.uni" WIDTH="60"/><FIELD attrname="Desc3" fieldtype="string.uni" WIDTH="60"/><FIELD attrname="Desc4" fieldtype="string.uni" WIDTH="60"/><FIELD attrname="Desc5" fieldtype="string.uni" WIDTH="60"/><FIELD attrname="Int1" fieldtype="i4"/><FIELD attrname="Int2" fieldtype="i4"/><FIELD attrname="Int3" fieldtype="i4"/><FIELD attrname="Int4" fieldtype="i4"/><FIELD attrname="Int5" fieldtype="i4"/><FIELD attrname="Body1" fieldtype="bin.hex" SUBTYPE="Binary"/><FIELD attrname="Body2" fieldtype="bin.hex" SUBTYPE="Binary"/></FIELDS><PARAMS DEFAULT_ORDER="1 2 3" PRIMARY_KEY="1 2 3" UNIQUE_KEY="1 3 4 2"/></METADATA><ROWDATA>"#;

pub fn export_xml(tasks: &[&Task]) -> String {
    let mut s = XML_HEAD.to_string();
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    for t in tasks {
        let body1 = if !t.changed && !t.orig_task.is_empty() { t.orig_task.clone() } else { crate::rvf::write(&t.task) };
        let body2 = if !t.changed && !t.orig_solution.is_empty() { t.orig_solution.clone() } else { crate::rvf::write(&t.solution) };
        s += &format!(
            "<ROW Clausur=\"{}\" Lesson=\"{}\" Indent=\"{}\" Position=\"{}\" Autornew=\"{}\" Autorupd=\"{}\" Datetimenew=\"{}\" Datetimeupd=\"{}\" Datefirstused=\"{}\" Datelastused=\"{}\" Clausurfirst=\"{}\" Clausurlast=\"{}\" Desc1=\"{}\" Desc2=\"{}\" Desc3=\"{}\" Int1=\"{}\" Int2=\"{}\" Int3=\"{}\" Int4=\"{}\" Int5=\"{}\" Body1=\"{}\" Body2=\"{}\"/>",
            write_date(&t.exam),
            write_date(&t.lesson.or(t.created)),
            if t.indent.is_empty() { "L" } else { &t.indent },
            t.position,
            xml_attr(&t.author_new),
            xml_attr(&t.author_upd),
            write_date(&t.created),
            write_date(&t.updated),
            write_date(&t.first_used),
            write_date(&t.last_used),
            write_date(&t.exam_first),
            write_date(&t.exam_last),
            xml_attr(&t.subject),
            xml_attr(&t.topic),
            xml_attr(&t.subtopic),
            if t.underline_answer { -(t.answer_lines as i32) } else { t.answer_lines as i32 },
            t.points,
            t.int3,
            t.int4,
            t.int5,
            b64(&body1),
            b64(&body2)
        );
    }
    s += "</ROWDATA></DATAPACKET>";
    s
}

/// A bundle of tasks (and optionally exams) for sharing with colleagues.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Bundle {
    pub format: String,
    /// who shared it (teacher's name from the settings) and when
    pub author: String,
    pub created: String,
    pub tasks: Vec<Task>,
    pub sheets: Vec<Sheet>,
}

/// Sort key of a replacement task: points difference, other keyword 2, other difficulty,
/// used in an exam, last exam use, random, task id.
type Candidate = (i32, bool, bool, bool, Option<NaiveDateTime>, u64, u64);

/// How a parallel version (Gruppe A/B) of a sheet is made.
#[derive(Clone, Copy, Debug)]
pub struct ParallelOptions {
    /// replace every task by another one with the same subject, keyword 1 and points
    pub replace: bool,
    /// put the tasks in a different order (additional texts keep their places)
    pub shuffle: bool,
    /// how many points a replacement may differ (0 = same points only)
    pub tolerance: i32,
}

pub struct ParallelResult {
    pub sheet: Sheet,
    /// tasks swapped for an equivalent one
    pub replaced: usize,
    /// tasks kept because no equivalent task exists
    pub kept: usize,
    /// total points of the new version minus those of the original
    pub points_diff: i32,
}

fn xorshift(x: &mut u64) -> u64 {
    if *x == 0 {
        *x = 0x9E37_79B9_7F4A_7C15;
    }
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

impl Database {
    /// All sheets of one parallel family: the original and every version made from it.
    pub fn parallel_family(&self, sheet: &Sheet) -> Vec<u64> {
        let root = sheet.parallel_of.unwrap_or(sheet.id);
        self.sheets.iter().filter(|s| s.id == root || s.parallel_of == Some(root)).map(|s| s.id).collect()
    }

    /// The first group letter not used in the sheet's family ("B" after "A").
    pub fn next_group(&self, sheet: &Sheet) -> String {
        let fam = self.parallel_family(sheet);
        let used: Vec<String> = self.sheets.iter().filter(|s| fam.contains(&s.id)).map(|s| s.group.trim().to_uppercase()).collect();
        let first = if sheet.group.trim().is_empty() { "A".to_string() } else { sheet.group.trim().to_uppercase() };
        ('A'..='Z').map(|c| c.to_string()).find(|g| *g != first && !used.contains(g)).unwrap_or_else(|| "Z".into())
    }

    /// Makes a parallel version of `src` (a new sheet with id `new_id`).
    /// Replacement tasks have the same subject and "Schlagwort 1" and at most
    /// `tolerance` points more or less. Preferred, in this order: the closest points,
    /// the same "Schlagwort 2", the same difficulty, never or longest ago used in an exam.
    /// Tasks of any sheet of the same family are never used twice.
    pub fn make_parallel(&self, src: &Sheet, opt: ParallelOptions, seed: u64, new_id: u64) -> ParallelResult {
        let mut rng = seed | 1;
        let mut out = src.clone();
        out.id = new_id;
        out.parallel_of = Some(src.parallel_of.unwrap_or(src.id));
        out.group = self.next_group(src);
        out.results.clear();
        out.used_marked = false;
        out.archived = false;
        let fam = self.parallel_family(src);
        let mut taken: HashSet<u64> = self.sheets.iter().filter(|s| fam.contains(&s.id)).flat_map(|s| s.task_ids()).collect();
        taken.extend(src.task_ids());
        let (mut replaced, mut kept) = (0, 0);
        if opt.replace {
            for item in out.items.iter_mut() {
                let SheetItem::Task(tid) = item else { continue };
                let Some(orig) = self.task(*tid) else { continue };
                let tol = opt.tolerance.max(0);
                let mut cands: Vec<Candidate> = self
                    .tasks
                    .iter()
                    .filter(|t| !taken.contains(&t.id))
                    .filter(|t| t.subject == orig.subject && t.topic == orig.topic && (t.points - orig.points).abs() <= tol)
                    .map(|t| {
                        let same_sub = t.subtopic.trim().eq_ignore_ascii_case(orig.subtopic.trim());
                        (
                            (t.points - orig.points).abs(),
                            !same_sub,
                            t.difficulty != orig.difficulty,
                            t.exam_last.is_some(),
                            t.exam_last,
                            xorshift(&mut rng),
                            t.id,
                        )
                    })
                    .collect();
                cands.sort();
                match cands.first() {
                    Some(c) => {
                        *tid = c.6;
                        taken.insert(c.6);
                        replaced += 1;
                    }
                    None => kept += 1,
                }
            }
        }
        if opt.shuffle {
            let pos: Vec<usize> = out.items.iter().enumerate().filter(|(_, i)| matches!(i, SheetItem::Task(_))).map(|(n, _)| n).collect();
            let mut ids: Vec<SheetItem> = pos.iter().map(|&p| out.items[p].clone()).collect();
            if ids.len() > 1 {
                let before = ids.clone();
                for _ in 0..8 {
                    for i in (1..ids.len()).rev() {
                        let j = (xorshift(&mut rng) % (i as u64 + 1)) as usize;
                        ids.swap(i, j);
                    }
                    if ids != before {
                        break;
                    }
                }
            }
            for (p, it) in pos.into_iter().zip(ids) {
                out.items[p] = it;
            }
        }
        // other random numbers for tasks with variables
        out.seed = xorshift(&mut rng);
        if out.seed == src.seed {
            out.seed = out.seed.wrapping_add(1);
        }
        let points_diff = self.sheet_total(&out) - self.sheet_total(src);
        ParallelResult { sheet: out, replaced, kept, points_diff }
    }
}
