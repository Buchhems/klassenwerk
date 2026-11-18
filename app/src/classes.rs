//! Classes (Klassen/Kurse) with their students, and the grade book (Notenbuch)
//! that collects the results of all exams written by a class.
//! Not part of the original program.

use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Student {
    pub id: u64,
    pub name: String,
    /// private remark of the teacher (never printed)
    pub remark: String,
}

/// A grade of one student (oral or other assessment, or the report grade).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct StudentGrade {
    pub student: u64,
    /// as entered, e.g. "2", "2-", "11", "2,5"
    pub grade: String,
    /// for report grades: 0 = whole school year, 1 / 2 = Halbjahr
    pub term: u8,
}

/// Oral or other assessment (mündliche / sonstige Leistung) with a grade per student.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Assessment {
    pub id: u64,
    /// e.g. "Mündlich 1. Hj.", "Referat", "Heftführung"
    pub title: String,
    pub date: String,
    /// weight among the other assessments (1 = normal, 2 = counts double)
    pub weight: f32,
    pub grades: Vec<StudentGrade>,
    /// Halbjahr (1, 2); 0 = from the date, or the whole year without a date
    pub term: u8,
}

impl Default for Assessment {
    fn default() -> Self {
        Assessment { id: 0, title: String::new(), date: String::new(), weight: 1.0, grades: vec![], term: 0 }
    }
}

impl Assessment {
    /// The Halbjahr it counts in: the chosen one, otherwise from the date.
    pub fn term(&self) -> Option<u8> {
        if self.term > 0 {
            Some(self.term)
        } else {
            term_of_date(&self.date)
        }
    }
}

/// Halbjahr of a date "TT.MM.JJJJ": August to January = 1, February to July = 2.
pub fn term_of_date(date: &str) -> Option<u8> {
    use chrono::Datelike;
    let d = chrono::NaiveDate::parse_from_str(date.trim(), "%d.%m.%Y").ok()?;
    Some(if d.month() >= 8 || d.month() == 1 { 1 } else { 2 })
}

/// "Schuljahr", "1. Halbjahr", "2. Halbjahr"
pub fn term_name(term: u8) -> &'static str {
    match term {
        1 => "1. Halbjahr",
        2 => "2. Halbjahr",
        _ => "Schuljahr",
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Course {
    pub id: u64,
    /// e.g. "12a" or "BWL Kurs 2"
    pub name: String,
    pub subject: String,
    /// e.g. "2026/27"
    pub school_year: String,
    pub students: Vec<Student>,
    pub archived: bool,
    pub remarks: String,
    /// oral and other assessments (mündliche / sonstige Leistungen)
    pub assessments: Vec<Assessment>,
    /// share of the written exams in the overall grade, in percent (the rest: other assessments)
    pub written_percent: f32,
    /// report grades set by the teacher (Zeugnisnote); a missing entry means: use the proposal
    pub report: Vec<StudentGrade>,
}

impl Default for Course {
    fn default() -> Self {
        Course {
            id: 0,
            name: String::new(),
            subject: String::new(),
            school_year: String::new(),
            students: vec![],
            archived: false,
            remarks: String::new(),
            assessments: vec![],
            written_percent: 50.0,
            report: vec![],
        }
    }
}

/// The grade a student has in a list (empty = none).
pub fn grade_in(list: &[StudentGrade], student: u64) -> &str {
    grade_in_term(list, student, 0)
}

/// Sets or (with an empty text) removes the grade of a student.
pub fn set_grade(list: &mut Vec<StudentGrade>, student: u64, grade: &str) {
    set_grade_term(list, student, 0, grade)
}

/// The report grade of a student for a Halbjahr (0 = school year).
pub fn grade_in_term(list: &[StudentGrade], student: u64, term: u8) -> &str {
    list.iter().find(|g| g.student == student && g.term == term).map(|g| g.grade.as_str()).unwrap_or("")
}

pub fn set_grade_term(list: &mut Vec<StudentGrade>, student: u64, term: u8, grade: &str) {
    let g = grade.trim();
    match list.iter_mut().position(|x| x.student == student && x.term == term) {
        Some(i) if g.is_empty() => {
            list.remove(i);
        }
        Some(i) => list[i].grade = g.to_string(),
        None if !g.is_empty() => list.push(StudentGrade { student, grade: g.to_string(), term }),
        None => {}
    }
}

/// Numeric value of a grade as typed: "2" → 2, "2-" → 2,33, "2,5" → 2,5, "13" → 13.
pub fn grade_value(text: &str) -> Option<f32> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(v) = t.replace(',', ".").parse::<f32>() {
        return Some(v);
    }
    if t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return Some(GradeScheme::value(t));
    }
    None
}

/// Turns pasted text (one student per line, e.g. copied from a spreadsheet or a
/// class list) into names. Leading numbers ("1.", "12)") are removed; with
/// tab/semicolon separated columns the first two columns become "Nachname, Vorname".
pub fn parse_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(['\t', ';']).map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
        let cols: Vec<&str> = cols.into_iter().filter(|c| !c.chars().all(|ch| ch.is_ascii_digit() || ".)".contains(ch))).collect();
        let name = match cols.len() {
            0 => continue,
            1 => cols[0].to_string(),
            _ => format!("{}, {}", cols[0], cols[1]),
        };
        let name = name.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')' || c.is_whitespace()).trim().to_string();
        let low = name.to_lowercase();
        let header = ["name", "nachname", "vorname", "schüler", "schülerin", "schüler/in", "schülerinnen und schüler"];
        if !name.is_empty() && !header.iter().any(|h| low == *h || low.starts_with(&format!("{h},"))) {
            out.push(name);
        }
    }
    out
}

impl Course {
    /// Adds students (skipping names already in the list). Returns how many were added.
    pub fn add_students(&mut self, names: &[String], db_next_id: &mut u64) -> usize {
        let mut n = 0;
        for name in names {
            if self.students.iter().any(|s| s.name.trim().eq_ignore_ascii_case(name.trim())) {
                continue;
            }
            *db_next_id = (*db_next_id).max(1);
            let id = *db_next_id;
            *db_next_id += 1;
            self.students.push(Student { id, name: name.trim().to_string(), remark: String::new() });
            n += 1;
        }
        n
    }
}

impl ResultRow {
    pub fn is_for(&self, s: &Student) -> bool {
        match self.student {
            Some(id) => id == s.id,
            None => !self.name.trim().is_empty() && self.name.trim().eq_ignore_ascii_case(s.name.trim()),
        }
    }
}

/// One result in the grade book.
#[derive(Clone, Debug)]
pub struct Cell {
    /// the sheet the result comes from (0 for an oral/other assessment)
    pub sheet_id: u64,
    pub points: f32,
    pub total: f32,
    /// grade label from the sheet's grading key ("2", "13", "3-") or as entered
    pub grade: Option<String>,
    /// numeric grade value used for averages
    pub value: Option<f32>,
    /// the group (A/B) the student wrote
    pub group: String,
}

/// One exam (or one family of parallel exams, Gruppe A/B), or one oral/other assessment.
#[derive(Clone, Debug)]
pub struct Column {
    pub sheet_ids: Vec<u64>,
    /// id of the assessment for an oral/other column
    pub assessment: Option<u64>,
    pub title: String,
    pub date: String,
    pub kind: String,
    pub weight: f32,
    pub average: Option<f32>,
}

impl Column {
    pub fn is_written(&self) -> bool {
        self.assessment.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub student: u64,
    pub name: String,
    pub cells: Vec<Option<Cell>>,
    /// weighted average of the written exams
    pub written: Option<f32>,
    /// weighted average of the oral/other assessments
    pub other: Option<f32>,
    /// overall average (written and other combined by the course's share)
    pub average: Option<f32>,
    /// proposed report grade (rounded overall average)
    pub proposal: Option<String>,
    /// report grade set by the teacher
    pub report: Option<String>,
}

impl Row {
    /// The report grade: the teacher's entry, otherwise the proposal.
    pub fn final_grade(&self) -> Option<&str> {
        self.report.as_deref().or(self.proposal.as_deref())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Gradebook {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// mean of the students' averages
    pub average: Option<f32>,
    /// true when there are oral/other assessments (the averages are then split)
    pub has_other: bool,
    pub written_percent: f32,
    /// 0 = school year, 1 / 2 = Halbjahr
    pub term: u8,
}

fn sheet_date(s: &Sheet) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s.date.trim(), "%d.%m.%Y").ok()
}

fn weighted(cells: &[Option<Cell>], cols: &[Column], written: bool) -> Option<f32> {
    let (mut sum, mut w) = (0.0f32, 0.0f32);
    for (c, col) in cells.iter().zip(cols) {
        if col.is_written() != written {
            continue;
        }
        if let Some(v) = c.as_ref().and_then(|c| c.value) {
            sum += v * col.weight;
            w += col.weight;
        }
    }
    if w > 0.0 {
        Some(sum / w)
    } else {
        None
    }
}

/// The grading key used for the class: the one of its first exam.
pub fn course_scheme<'a>(db: &'a Database, course: &Course) -> Option<&'a GradeScheme> {
    let mut sheets: Vec<&Sheet> = db.sheets.iter().filter(|s| s.course_id == Some(course.id)).collect();
    sheets.sort_by_key(|s| (sheet_date(s), s.created, s.id));
    sheets.iter().find_map(|s| db.settings.scheme(&s.scheme))
}

/// Proposed report grade for an average: rounded to a whole grade (x,5 rounds up
/// to the next number), limited to the grades of the key.
pub fn proposal(avg: f32, scheme: Option<&GradeScheme>) -> String {
    let mut v = (avg + 0.0001).round();
    if let Some(sc) = scheme {
        let vals: Vec<f32> = sc.rows.iter().map(|r| GradeScheme::value(&r.label).round()).collect();
        if let (Some(lo), Some(hi)) = (vals.iter().copied().reduce(f32::min), vals.iter().copied().reduce(f32::max)) {
            v = v.clamp(lo, hi);
        }
    }
    format!("{}", v as i32)
}

/// Collects the results of every sheet assigned to the course. Parallel versions
/// (Gruppe A/B) share one column; a student's result is taken from whichever
/// version they wrote. Results without a student link count when the name matches.
/// Oral/other assessments follow as further columns.
pub fn gradebook(db: &Database, course: &Course) -> Gradebook {
    gradebook_term(db, course, 0)
}

/// The grade book of one Halbjahr (1, 2) or of the school year (0): exams count
/// by their date, oral/other assessments by their Halbjahr (or date). Without a
/// date they only count for the school year.
pub fn gradebook_term(db: &Database, course: &Course, term: u8) -> Gradebook {
    let in_term = |t: Option<u8>| term == 0 || t == Some(term);
    let mut sheets: Vec<&Sheet> = db.sheets.iter().filter(|s| s.course_id == Some(course.id)).filter(|s| in_term(term_of_date(&s.date))).collect();
    sheets.sort_by_key(|s| (sheet_date(s), s.created, s.id));
    // group the parallel versions
    let mut fams: Vec<(u64, Vec<&Sheet>)> = Vec::new();
    for s in sheets {
        let root = s.parallel_of.unwrap_or(s.id);
        match fams.iter_mut().find(|f| f.0 == root) {
            Some(f) => f.1.push(s),
            None => fams.push((root, vec![s])),
        }
    }
    let mut columns: Vec<Column> = fams
        .iter()
        .map(|(root, v)| {
            let head = v.iter().find(|s| s.id == *root).copied().unwrap_or(v[0]);
            Column {
                sheet_ids: v.iter().map(|s| s.id).collect(),
                assessment: None,
                title: head.title.clone(),
                date: head.date.clone(),
                kind: head.kind.clone(),
                weight: head.weight.max(0.0),
                average: None,
            }
        })
        .collect();
    let assessments: Vec<&Assessment> = course.assessments.iter().filter(|a| in_term(a.term())).collect();
    for a in &assessments {
        columns.push(Column {
            sheet_ids: vec![],
            assessment: Some(a.id),
            title: a.title.clone(),
            date: a.date.clone(),
            kind: "sonstige Leistung".into(),
            weight: a.weight.max(0.0),
            average: None,
        });
    }
    let has_other = !assessments.is_empty();
    let share = (course.written_percent / 100.0).clamp(0.0, 1.0);
    let scheme = course_scheme(db, course);
    let mut rows = Vec::new();
    for st in &course.students {
        let mut cells = Vec::new();
        for (_, fam) in &fams {
            let mut cell = None;
            for s in fam {
                let Some(r) = s.results.iter().find(|r| !r.absent && r.is_for(st)) else { continue };
                let total = db.sheet_total(s) as f32;
                let (grade, value) = match db.settings.scheme(&s.scheme) {
                    Some(sc) => {
                        let rows = sc.rows_for(total);
                        match sc.grade_for(total, r.points) {
                            Some(i) => (Some(rows[i].0.clone()), Some(GradeScheme::value(&rows[i].0))),
                            None => (None, None),
                        }
                    }
                    None => (None, None),
                };
                cell = Some(Cell { sheet_id: s.id, points: r.points, total, grade, value, group: s.group.clone() });
                break;
            }
            cells.push(cell);
        }
        for a in &assessments {
            let g = grade_in(&a.grades, st.id);
            cells.push(if g.is_empty() {
                None
            } else {
                Some(Cell { sheet_id: 0, points: 0.0, total: 0.0, grade: Some(g.to_string()), value: grade_value(g), group: String::new() })
            });
        }
        let written = weighted(&cells, &columns, true);
        let other = weighted(&cells, &columns, false);
        let average = match (written, other) {
            (Some(w), Some(o)) => Some(w * share + o * (1.0 - share)),
            (w, o) => w.or(o),
        };
        let report = Some(grade_in_term(&course.report, st.id, term).to_string()).filter(|g| !g.is_empty());
        rows.push(Row { student: st.id, name: st.name.clone(), cells, written, other, average, proposal: average.map(|a| proposal(a, scheme)), report });
    }
    for (i, col) in columns.iter_mut().enumerate() {
        let v: Vec<f32> = rows.iter().filter_map(|r| r.cells[i].as_ref().and_then(|c| c.value)).collect();
        col.average = if v.is_empty() { None } else { Some(v.iter().sum::<f32>() / v.len() as f32) };
    }
    let avgs: Vec<f32> = rows.iter().filter_map(|r| r.average).collect();
    let average = if avgs.is_empty() { None } else { Some(avgs.iter().sum::<f32>() / avgs.len() as f32) };
    Gradebook { columns, rows, average, has_other, written_percent: course.written_percent, term }
}

/// Grades of one student in date order, for the chart of their development:
/// (label of the column, grade value, written exam?).
pub fn development(gb: &Gradebook, row: &Row) -> Vec<(String, f32, bool)> {
    let mut v: Vec<(Option<chrono::NaiveDate>, usize, String, f32, bool)> = Vec::new();
    for (k, (c, col)) in row.cells.iter().zip(&gb.columns).enumerate() {
        if let Some(val) = c.as_ref().and_then(|c| c.value) {
            let d = chrono::NaiveDate::parse_from_str(col.date.trim(), "%d.%m.%Y").ok();
            v.push((d, k, col.title.clone(), val, col.is_written()));
        }
    }
    // undated columns keep their place after the dated ones
    v.sort_by_key(|x| (x.0.is_none(), x.0, x.1));
    v.into_iter().map(|x| (x.2, x.3, x.4)).collect()
}

pub fn fmt2(v: f32) -> String {
    format!("{v:.2}").replace('.', ",")
}

fn cell_text(c: &Option<Cell>) -> String {
    match c {
        Some(c) if c.sheet_id == 0 => c.grade.clone().unwrap_or_default(),
        Some(c) => match &c.grade {
            Some(g) => format!("{g} ({} P.)", crate::export::fmt_pts(c.points)),
            None => format!("{} P.", crate::export::fmt_pts(c.points)),
        },
        None => String::new(),
    }
}

/// Grade book as CSV for Excel/LibreOffice (semicolons, decimal comma, UTF-8 with BOM).
pub fn to_csv(gb: &Gradebook, course: &Course) -> String {
    let q = |s: &str| {
        if s.contains([';', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = String::from("\u{feff}");
    out += &format!("{};{};{};{}\n", q(&course.name), q(&course.subject), q(&course.school_year), term_name(gb.term));
    let mut head = vec!["Name".to_string()];
    let mut weights = vec!["Gewichtung".to_string()];
    for c in &gb.columns {
        let t = if c.date.is_empty() { c.title.clone() } else { format!("{} ({})", c.title, c.date) };
        if c.is_written() {
            head.push(q(&format!("{t} – Note")));
            head.push(q(&format!("{t} – Punkte")));
            weights.push(fmt2(c.weight));
            weights.push(String::new());
        } else {
            head.push(q(&format!("{t} – Note")));
            weights.push(fmt2(c.weight));
        }
    }
    let tail: Vec<&str> = if gb.has_other { vec!["Ø schriftlich", "Ø sonstige", "Ø gesamt", "Zeugnisnote"] } else { vec!["Durchschnitt", "Zeugnisnote"] };
    for t in &tail {
        head.push(t.to_string());
        weights.push(String::new());
    }
    out += &head.join(";");
    out += "\n";
    out += &weights.join(";");
    out += "\n";
    for r in &gb.rows {
        let mut line = vec![q(&r.name)];
        for (c, col) in r.cells.iter().zip(&gb.columns) {
            match (c, col.is_written()) {
                (Some(c), true) => {
                    line.push(q(c.grade.as_deref().unwrap_or("")));
                    line.push(crate::export::fmt_pts(c.points));
                }
                (None, true) => {
                    line.push(String::new());
                    line.push(String::new());
                }
                (c, false) => line.push(q(c.as_ref().and_then(|c| c.grade.as_deref()).unwrap_or(""))),
            }
        }
        if gb.has_other {
            line.push(r.written.map(fmt2).unwrap_or_default());
            line.push(r.other.map(fmt2).unwrap_or_default());
        }
        line.push(r.average.map(fmt2).unwrap_or_default());
        line.push(q(r.final_grade().unwrap_or("")));
        out += &line.join(";");
        out += "\n";
    }
    let mut avg = vec!["Klassendurchschnitt".to_string()];
    for c in &gb.columns {
        avg.push(c.average.map(fmt2).unwrap_or_default());
        if c.is_written() {
            avg.push(String::new());
        }
    }
    if gb.has_other {
        avg.push(String::new());
        avg.push(String::new());
    }
    avg.push(gb.average.map(fmt2).unwrap_or_default());
    avg.push(String::new());
    out += &avg.join(";");
    out += "\n";
    out
}

/// Grade book as a printable markup page.
pub fn to_markup(gb: &Gradebook, course: &Course) -> String {
    let e = markup_escape;
    let mut s = format!("# Notenbuch {}\n", e(&course.name));
    let term = if gb.term > 0 { term_name(gb.term) } else { "" };
    let info: Vec<&str> = [course.subject.as_str(), course.school_year.as_str(), term].into_iter().filter(|x| !x.trim().is_empty()).collect();
    if !info.is_empty() {
        s += &format!("{}\n", e(&info.join(" · ")));
    }
    let n_written = gb.columns.iter().filter(|c| c.is_written()).count();
    s += &format!("{} Schülerinnen und Schüler · {} Leistungsnachweise", gb.rows.len(), n_written);
    if gb.has_other {
        s += &format!(" · {} sonstige Leistungen", gb.columns.len() - n_written);
    }
    s += "\n\n";
    let mut head = String::from("+| **Name** |");
    for c in &gb.columns {
        head += &if c.date.trim().is_empty() { format!(" **{}** |", e(&c.title)) } else { format!(" **{}** ({}) |", e(&c.title), e(&c.date)) };
    }
    if gb.has_other {
        head += " **Ø schriftl.** | **Ø sonst.** |";
    }
    head += " **Ø** | **Zeugnis** |";
    s += &head;
    s += "\n";
    for r in &gb.rows {
        let mut line = format!("+| {} |", e(&r.name));
        for c in &r.cells {
            line += &format!(" {} |", e(&cell_text(c)));
        }
        if gb.has_other {
            line += &format!(" {} | {} |", r.written.map(fmt2).unwrap_or_default(), r.other.map(fmt2).unwrap_or_default());
        }
        line += &format!(" **{}** | **{}** |", r.average.map(fmt2).unwrap_or_default(), e(r.final_grade().unwrap_or("")));
        s += &line;
        s += "\n";
    }
    let mut line = String::from("+| *Klassendurchschnitt* |");
    for c in &gb.columns {
        line += &format!(" *{}* |", c.average.map(fmt2).unwrap_or_default());
    }
    if gb.has_other {
        line += "  |  |";
    }
    line += &format!(" *{}* |  |", gb.average.map(fmt2).unwrap_or_default());
    s += &line;
    s += "\n";
    let w: Vec<String> = gb.columns.iter().filter(|c| (c.weight - 1.0).abs() > 0.001).map(|c| format!("{} ×{}", e(&c.title), fmt2(c.weight))).collect();
    if !w.is_empty() {
        s += &format!("\nGewichtung: {}\n", w.join(", "));
    }
    if gb.has_other {
        s += &format!(
            "\nGesamtnote: schriftlich {} %, sonstige Leistungen {} %. Der Zeugnisvorschlag ist der gerundete Gesamtdurchschnitt.\n",
            fmt_pct(gb.written_percent),
            fmt_pct(100.0 - gb.written_percent)
        );
    }
    s
}

fn fmt_pct(v: f32) -> String {
    crate::export::fmt_pts(v.clamp(0.0, 100.0))
}

// ---------------------------------------------------------------- CSV import

/// Text of an imported file: UTF-8 (with or without BOM), otherwise Windows-1252 (old Excel).
pub fn decode_text(bytes: &[u8]) -> String {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => crate::delphi::cp1252(b),
    }
}

/// Splits CSV text into rows and cells. The separator is detected: semicolon
/// (German Excel), tab, or comma. Quotes ("…") are handled.
pub fn read_csv(text: &str) -> Vec<Vec<String>> {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = if first.contains(';') {
        ';'
    } else if first.contains('\t') {
        '\t'
    } else {
        ','
    };
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => row.push(std::mem::take(&mut cell).trim().to_string()),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut cell).trim().to_string());
                if row.iter().any(|c| !c.is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            c => cell.push(c),
        }
    }
    row.push(cell.trim().to_string());
    if row.iter().any(|c| !c.is_empty()) {
        rows.push(row);
    }
    rows
}

/// Name in a comparable form: "Müller, Anna" and "Anna Müller" are the same.
pub fn name_key(name: &str) -> String {
    let mut w: Vec<String> = name.to_lowercase().replace([',', ';'], " ").split_whitespace().map(|s| s.to_string()).collect();
    w.sort();
    w.join(" ")
}

fn num(s: &str) -> Option<f32> {
    let t = s.trim().trim_end_matches("P.").trim_end_matches('P').trim();
    if t.is_empty() {
        return None;
    }
    t.replace(',', ".").parse::<f32>().ok()
}

/// What an import of results did.
#[derive(Debug, Default, PartialEq)]
pub struct ImportReport {
    /// rows that updated an existing result or a student of the class
    pub matched: usize,
    /// new result rows without a matching student
    pub added: usize,
    /// columns of the file that were not understood
    pub ignored_columns: Vec<String>,
}

/// Reads results from CSV text: one row per student, a name column and either a
/// total ("Punkte", "Summe", "Gesamt") or one column per task / Teilaufgabe with
/// the labels of `cols` ("1", "2a" …; "Aufgabe 2a" works too). "fehlt" marks an absent student.
/// Existing rows are updated by name; students of the class get linked.
pub fn import_results(text: &str, cols: &[ScoreCol], students: &[Student], results: &mut Vec<ResultRow>) -> Result<ImportReport, String> {
    let rows = read_csv(text);
    let Some(head) = rows.first() else { return Err("Die Datei ist leer.".into()) };
    let norm = |h: &str| h.to_lowercase().replace("aufgabe", "").replace("teil", "").replace(['.', ' '], "").trim().to_string();
    let mut name_col = None;
    let mut total_col = None;
    let mut map: Vec<(usize, usize)> = Vec::new(); // (csv column, score column)
    let mut report = ImportReport::default();
    for (i, h) in head.iter().enumerate() {
        let l = h.trim().to_lowercase();
        if name_col.is_none() && (l.contains("name") || l == "schüler" || l == "schülerin") {
            name_col = Some(i);
        } else if ["punkte", "summe", "gesamt", "gesamtpunkte", "punktzahl"].contains(&l.as_str()) {
            total_col = Some(i);
        } else if let Some(k) = cols.iter().position(|c| norm(&c.label) == norm(h) && !norm(h).is_empty()) {
            map.push((i, k));
        } else if !l.is_empty() && !["note", "nr", "nr.", "fehlt"].contains(&l.as_str()) {
            report.ignored_columns.push(h.trim().to_string());
        }
    }
    // without a header: "Name;Punkte" rows
    let has_header = name_col.is_some() || total_col.is_some() || !map.is_empty();
    let (name_col, data) = if has_header {
        (name_col.unwrap_or(0), &rows[1..])
    } else {
        report.ignored_columns.clear();
        if head.len() >= 2 && num(&head[1]).is_some() {
            total_col = Some(1);
            (0, &rows[..])
        } else {
            return Err(
                "Keine Spalte für Punkte gefunden. Erwartet wird eine Kopfzeile mit „Name“ und „Punkte“ oder den Aufgabennummern (z. B. 1, 2a, 2b).".into()
            );
        }
    };
    if total_col.is_none() && map.is_empty() {
        return Err("Keine Spalte für Punkte gefunden. Erwartet wird eine Kopfzeile mit „Name“ und „Punkte“ oder den Aufgabennummern (z. B. 1, 2a, 2b).".into());
    }
    for r in data {
        let name = r.get(name_col).map(|s| s.trim().to_string()).unwrap_or_default();
        // the "max." row of the template, or a row with the maximum points
        if name.is_empty() || ["max", "max.", "maximal", "höchstpunktzahl"].contains(&name.to_lowercase().as_str()) {
            continue;
        }
        let absent = r.iter().any(|c| c.trim().eq_ignore_ascii_case("fehlt"));
        let key = name_key(&name);
        let student = students.iter().find(|s| name_key(&s.name) == key);
        let pos = results.iter().position(|x| match (x.student, student) {
            (Some(id), Some(st)) => id == st.id,
            _ => name_key(&x.name) == key,
        });
        let idx = match pos {
            Some(i) => {
                report.matched += 1;
                i
            }
            None => {
                if student.is_some() {
                    report.matched += 1;
                } else {
                    report.added += 1;
                }
                results.push(ResultRow { name: student.map(|s| s.name.clone()).unwrap_or(name.clone()), student: student.map(|s| s.id), ..Default::default() });
                results.len() - 1
            }
        };
        let row = &mut results[idx];
        if row.student.is_none() {
            row.student = student.map(|s| s.id);
        }
        row.absent = absent;
        if !map.is_empty() {
            row.scores.clear();
            for (ci, k) in &map {
                if let Some(v) = r.get(*ci).and_then(|c| num(c)) {
                    row.scores.insert(cols[*k].key.clone(), v.max(0.0));
                }
            }
            row.sum_scores(cols);
        }
        if let Some(v) = total_col.and_then(|i| r.get(i)).and_then(|c| num(c)) {
            if map.is_empty() || row.scores.is_empty() {
                row.scores.clear();
                row.points = v.max(0.0);
            }
        }
    }
    Ok(report)
}

/// An empty CSV table to fill in (e.g. in Excel) and import again: one column per
/// task / Teilaufgabe, one row per student.
pub fn results_template(cols: &[ScoreCol], names: &[String]) -> String {
    let mut out = String::from("\u{feff}Name");
    for c in cols {
        out += &format!(";{}", c.label);
    }
    out += ";Punkte\n";
    out += "max.";
    for c in cols {
        out += &format!(";{}", crate::export::fmt_pts(c.max));
    }
    out += &format!(";{}\n", crate::export::fmt_pts(cols.iter().map(|c| c.max).sum()));
    for n in names {
        let n = if n.contains([';', '"']) { format!("\"{}\"", n.replace('"', "\"\"")) } else { n.clone() };
        out += &n;
        out += &";".repeat(cols.len() + 1);
        out += "\n";
    }
    out
}

fn markup_escape(s: &str) -> String {
    crate::rvf::escape(s)
}
