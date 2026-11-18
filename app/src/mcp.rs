//! MCP server (Model Context Protocol) for AI assistants such as Claude.
//! Started with `Klassenwerk.exe --mcp`; talks JSON-RPC over stdin/stdout.
//!
//! What an assistant may do (decision of the teacher, Nov 2026):
//! * read and search tasks, exams/worksheets, subjects, grading keys
//! * create new tasks and exam drafts (marked "KI-Assistent")
//! * nothing is changed or deleted
//! * no student data: classes, student lists, results and the grade book are
//!   never sent (privacy / GDPR)
//!
//! Every call reads the data file fresh and writes it back right away, so the
//! running program picks the new entries up (see `Database::merge_external`).

use crate::model::*;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub const ORIGIN: &str = "KI-Assistent";
const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Klassenwerk ist die Aufgabendatenbank und der Klausurplaner einer Lehrkraft. \
Über diese Werkzeuge können Aufgaben (mit Musterlösung) und Klausuren/Übungen gesucht und gelesen sowie neue Aufgaben \
und Klausur-Entwürfe angelegt werden. Bestehende Einträge können nicht geändert oder gelöscht werden; neue Einträge \
sind als „KI-Assistent“ gekennzeichnet und werden von der Lehrkraft in Klassenwerk geprüft. Schülerdaten (Klassen, \
Namen, Ergebnisse, Noten) sind aus Datenschutzgründen nicht zugänglich. Aufgabentexte nutzen eine einfache \
Auszeichnung (siehe Werkzeug aufgabe_anlegen); Formeln stehen als $LaTeX$ im Text. Antworten auf Deutsch.";

const MARKUP_HELP: &str = "Auszeichnung im Text: **fett**, *kursiv*, ++unterstrichen++, x^{2} hoch, x_{1} tief, \
Zeile mit '# ' = Überschrift, Zeile mit '• ' oder '- ' = Aufzählung, '---' = Trennlinie, \
'| a) | Text | 2 P. |' = Tabelle ohne Rahmen (z. B. Teilaufgaben), '+| A | B |' = Tabelle mit Rahmen, \
'[ ]' Ankreuzkästchen, '[[Linien:5]]' = 5 Antwortlinien, {blau:Text} farbig. \
Formeln in $...$ mit LaTeX-Schreibweise: $\\frac{a}{b}$, $x^2$, $\\sqrt{x}$, $\\int_{0}^{1} x\\,dx$, $\\sum_{i=1}^{n}$, \
$\\lim_{x \\to 0}$, $\\vec{v}$, $\\alpha$, $\\cdot$, $\\le$, $\\Rightarrow$, $\\mathbb{R}$, \\left( \\right). \
Ein Dollarzeichen als Text: \\$.";

/// Where the data file is: `--mcp-db PATH` or the program's normal file.
pub struct Server {
    path: PathBuf,
}

fn ok(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

fn tool_err(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": true })
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}

fn n(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.replace(',', ".").parse().ok())))
}

fn difficulty_label(d: u8) -> &'static str {
    match d {
        1 => "leicht",
        2 => "mittel",
        3 => "schwer",
        _ => "",
    }
}

fn date(d: &Option<chrono::NaiveDateTime>) -> Value {
    d.map(|d| Value::String(d.format("%d.%m.%Y").to_string())).unwrap_or(Value::Null)
}

/// The task text for an assistant: `{{n}}` objects become `$formula$`, [Bild] or [Formel (Bild)].
pub fn doc_for_assistant(doc: &Doc) -> String {
    let mut out = String::new();
    let mut rest = doc.text.as_str();
    while let Some(p) = rest.find("{{") {
        out += &rest[..p];
        let after = &rest[p + 2..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && after[digits.len()..].starts_with("}}") {
            let id: u32 = digits.parse().unwrap_or(0);
            out += &match doc.object(id) {
                Some(o) if o.kind == ObjKind::Math => format!("${}$", String::from_utf8_lossy(&o.data)),
                Some(o) if o.kind == ObjKind::Formula => "[Formel (Bild aus altem Formeleditor)]".to_string(),
                Some(_) => "[Bild]".to_string(),
                None => String::new(),
            };
            rest = &after[digits.len() + 2..];
        } else {
            out += "{{";
            rest = after;
        }
    }
    out += rest;
    out
}

/// Turns `$…$` formulas in text from an assistant into formula objects (`{{n}}`).
/// Returns the document and the formula parts that were not understood.
pub fn doc_from_assistant(text: &str) -> (Doc, Vec<String>) {
    let mut doc = Doc::default();
    let mut out = String::new();
    let mut problems = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1) == Some(&'$') {
            out.push('$');
            i += 2;
            continue;
        }
        if c == '$' {
            // find the closing $
            let mut j = i + 1;
            while j < chars.len() && !(chars[j] == '$' && chars[j - 1] != '\\') {
                j += 1;
            }
            if j < chars.len() && j > i + 1 {
                let src: String = chars[i + 1..j].iter().collect();
                let src = src.trim().to_string();
                for e in crate::math::errors(&crate::math::parse(&src)) {
                    problems.push(format!("{e} in ${src}$"));
                }
                let d = crate::math::layout(&src);
                let (w, h) = crate::math::size_px(&d);
                let id = doc.next_obj_id();
                doc.objects.push(DocObject { id, kind: ObjKind::Math, data: src.into_bytes(), width: w, height: h });
                out += &format!("{{{{{id}}}}}");
                i = j + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    // "- " at the start of a line is a bullet, as in the app's markup help
    doc.text = out.lines().map(|l| if let Some(r) = l.strip_prefix("- ") { format!("• {r}") } else { l.to_string() }).collect::<Vec<_>>().join("\n");
    (doc, problems)
}

fn task_summary(db: &Database, t: &Task) -> Value {
    let preview: String = crate::markup::plain(&doc_for_assistant(&t.task)).chars().take(160).collect();
    json!({
        "id": t.id,
        "fach": t.subject,
        "schlagwort1": t.topic,
        "schlagwort2": t.subtopic,
        "punkte": t.points,
        "schwierigkeit": difficulty_label(t.difficulty),
        "minuten": if t.minutes > 0 { json!(t.minutes) } else { Value::Null },
        "tags": t.tags,
        "favorit": t.favorite,
        "zuletzt_in_klausur": date(&t.exam_last),
        "zuletzt_verwendet": date(&t.last_any_use()),
        "in_mappen": db.sheets.iter().filter(|s| s.contains(t.id)).count(),
        "angelegt_von": if t.origin.is_empty() { Value::Null } else { json!(t.origin) },
        "textanfang": preview.trim(),
    })
}

fn sheet_summary(db: &Database, s: &Sheet) -> Value {
    json!({
        "id": s.id,
        "titel": s.title,
        "gruppe": if s.group.trim().is_empty() { Value::Null } else { json!(s.group) },
        "typ": s.kind,
        "fach": s.subject,
        "klasse": s.class_name,
        "datum": s.date,
        "dauer_min": s.minutes,
        "aufgaben": s.task_ids().len(),
        "gesamtpunkte": db.sheet_total(s),
        "notenschluessel": s.scheme,
        "durchgefuehrt": s.used_marked,
        "archiviert": s.archived,
        "angelegt_von": if s.origin.is_empty() { Value::Null } else { json!(s.origin) },
    })
}

fn grade_table(db: &Database, scheme: &str, total: f32) -> Option<Value> {
    let sc = db.settings.scheme(scheme)?;
    let rows: Vec<Value> = sc
        .rows_for(total)
        .into_iter()
        .filter(|(_, a, b)| a <= b)
        .map(|(l, a, b)| json!({ "note": l, "von_punkte": crate::export::fmt_bound(a), "bis_punkte": crate::export::fmt_bound(b) }))
        .collect();
    Some(json!({ "notenschluessel": sc.name, "gesamtpunkte": total, "stufen": rows }))
}

impl Server {
    pub fn new(path: Option<PathBuf>) -> Server {
        Server { path: path.unwrap_or_else(Database::path) }
    }

    fn load(&self) -> Result<Database, String> {
        if !self.path.exists() {
            // like the program's first start
            return Ok(Database::with_samples());
        }
        Database::load_plain(&self.path).ok_or_else(|| format!("Die Datenbank {} konnte nicht gelesen werden.", self.path.display()))
    }

    fn save(&self, db: &Database) -> Result<(), String> {
        db.save_to(&self.path)
    }

    pub fn tools() -> Value {
        json!([
            {
                "name": "uebersicht",
                "title": "Überblick über die Sammlung",
                "description": "Fächer mit ihren Schlagwörtern (Themen) und Anzahl der Aufgaben, vorhandene Tags, Notenschlüssel, Prüfungstypen und Druckvorlagen. Gut als erster Schritt.",
                "inputSchema": { "type": "object", "properties": {} },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "aufgaben_suchen",
                "title": "Aufgaben suchen",
                "description": "Sucht Aufgaben im Pool. Liefert Kurzinfos (id, Fach, Schlagwörter, Punkte, letzte Verwendung, Textanfang). Den vollen Text liefert aufgabe_lesen.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "suche": { "type": "string", "description": "Suchwörter (in Text, Lösung, Schlagwörtern, Tags; alle müssen vorkommen)" },
                        "fach": { "type": "string" },
                        "schlagwort": { "type": "string", "description": "Schlagwort 1 oder 2 (Thema)" },
                        "tag": { "type": "string" },
                        "min_punkte": { "type": "number" },
                        "max_punkte": { "type": "number" },
                        "schwierigkeit": { "type": "string", "enum": ["leicht", "mittel", "schwer"] },
                        "nie_in_klausur": { "type": "boolean", "description": "nur Aufgaben, die noch in keiner Klausur waren" },
                        "limit": { "type": "integer", "description": "höchstens so viele Treffer (Standard 30, max. 200)" }
                    }
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "aufgabe_lesen",
                "title": "Aufgabe lesen",
                "description": "Voller Aufgabentext und Musterlösung einer oder mehrerer Aufgaben (Formeln als $LaTeX$, Bilder als [Bild]).",
                "inputSchema": {
                    "type": "object",
                    "properties": { "ids": { "type": "array", "items": { "type": "integer" }, "description": "Aufgaben-ids (max. 20)" } },
                    "required": ["ids"]
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "klausuren_auflisten",
                "title": "Klausuren und Übungen auflisten",
                "description": "Listet Klausuren, Übungen, Arbeitsblätter usw. (ohne Ergebnisse oder Schülerdaten).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "suche": { "type": "string" },
                        "fach": { "type": "string" },
                        "typ": { "type": "string", "description": "z. B. Klausur, Übung" },
                        "archivierte": { "type": "boolean", "description": "auch archivierte zeigen (Standard: nein)" },
                        "limit": { "type": "integer" }
                    }
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "klausur_lesen",
                "title": "Klausur lesen",
                "description": "Aufbau einer Klausur/Übung: Kopfdaten, Aufgaben mit Punkten in Reihenfolge, Gesamtpunkte und Notenschlüssel-Tabelle. Mit mit_texten=true auch die vollen Aufgabentexte.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "mit_texten": { "type": "boolean" },
                        "mit_loesungen": { "type": "boolean" }
                    },
                    "required": ["id"]
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "notenschluessel_berechnen",
                "title": "Notenschlüssel berechnen",
                "description": "Punktegrenzen je Note für eine Gesamtpunktzahl (genau wie Klassenwerk druckt). Optional die Note für eine erreichte Punktzahl.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "gesamtpunkte": { "type": "number" },
                        "notenschluessel": { "type": "string", "description": "Name, z. B. IHK oder Gymnasium (Standard: IHK)" },
                        "erreicht": { "type": "number", "description": "erreichte Punkte, für die die Note bestimmt wird" }
                    },
                    "required": ["gesamtpunkte"]
                },
                "annotations": { "readOnlyHint": true }
            },
            {
                "name": "aufgabe_anlegen",
                "title": "Neue Aufgabe anlegen",
                "description": format!("Legt eine neue Aufgabe mit Musterlösung im Pool an (gekennzeichnet als „{ORIGIN}“). Vorhandene Aufgaben werden nie geändert. {MARKUP_HELP}"),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "fach": { "type": "string", "description": "Fach, möglichst wie in uebersicht geschrieben" },
                        "schlagwort1": { "type": "string", "description": "Thema, z. B. Integral" },
                        "schlagwort2": { "type": "string", "description": "Unterthema / Titel der Aufgabe" },
                        "punkte": { "type": "integer" },
                        "aufgabe": { "type": "string", "description": "Aufgabentext (Auszeichnung siehe oben)" },
                        "loesung": { "type": "string", "description": "Musterlösung / Erwartungshorizont" },
                        "schwierigkeit": { "type": "string", "enum": ["leicht", "mittel", "schwer"] },
                        "minuten": { "type": "integer", "description": "geschätzte Bearbeitungszeit" },
                        "leerzeilen": { "type": "integer", "description": "Antwortlinien unter der Aufgabe beim Druck" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "zufallszahlen": {
                            "type": "array",
                            "description": "Optional: Variablen für Aufgabenvarianten. Im Text, in der Lösung und in Formeln steht dann {=a} für den Wert oder {=2*a+b} für einen berechneten Wert ({=a/3;2} rundet auf 2 Stellen). Jede Klausur und jede Gruppe bekommt eigene Zahlen.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "von": { "type": "number" },
                                    "bis": { "type": "number" },
                                    "schritt": { "type": "number", "description": "Standard 1" }
                                },
                                "required": ["name", "von", "bis"]
                            }
                        },
                        "teilaufgaben": {
                            "type": "array",
                            "description": "Optional: Teilaufgaben mit eigenen Punkten und Erwartungshorizont. Die Summe ihrer Punkte ersetzt punkte. Den Text der Teilaufgaben (a) …, b) …) schreibt man trotzdem in aufgabe.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "bezeichnung": { "type": "string", "description": "a, b, c …" },
                                    "punkte": { "type": "integer" },
                                    "erwartung": { "type": "string", "description": "erwartete Leistung für die volle Punktzahl" }
                                },
                                "required": ["punkte"]
                            }
                        }
                    },
                    "required": ["fach", "schlagwort1", "punkte", "aufgabe"]
                },
                "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false }
            },
            {
                "name": "klausur_entwurf_anlegen",
                "title": "Klausur-Entwurf anlegen",
                "description": format!("Legt eine neue Klausur/Übung aus vorhandenen Aufgaben an (Entwurf, gekennzeichnet als „{ORIGIN}“; nicht als durchgeführt markiert). Die Lehrkraft prüft und druckt sie in Klassenwerk."),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "titel": { "type": "string" },
                        "aufgaben": { "type": "array", "items": { "type": "integer" }, "description": "Aufgaben-ids in der gewünschten Reihenfolge" },
                        "typ": { "type": "string", "description": "Prüfungstyp, z. B. Klausur, Übung, Arbeitsblatt (siehe uebersicht)" },
                        "fach": { "type": "string" },
                        "klasse": { "type": "string", "description": "Bezeichnung der Klasse/des Kurses als Text, z. B. 12a" },
                        "datum": { "type": "string", "description": "TT.MM.JJJJ" },
                        "dauer_min": { "type": "integer" },
                        "notenschluessel": { "type": "string" },
                        "vorlage": { "type": "string", "description": "Druckvorlage (siehe uebersicht)" },
                        "hinweis": { "type": "string", "description": "Text vor der ersten Aufgabe, z. B. Hilfsmittel" }
                    },
                    "required": ["titel", "aufgaben"]
                },
                "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false }
            }
        ])
    }

    pub fn call(&self, name: &str, a: &Value) -> Value {
        let r = match name {
            "uebersicht" => self.overview(),
            "aufgaben_suchen" => self.search(a),
            "aufgabe_lesen" => self.read_tasks(a),
            "klausuren_auflisten" => self.list_sheets(a),
            "klausur_lesen" => self.read_sheet(a),
            "notenschluessel_berechnen" => self.grades(a),
            "aufgabe_anlegen" => self.create_task(a),
            "klausur_entwurf_anlegen" => self.create_sheet(a),
            _ => Err(format!("Unbekanntes Werkzeug: {name}")),
        };
        match r {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_default()),
            Err(e) => tool_err(e),
        }
    }

    fn overview(&self) -> Result<Value, String> {
        let db = self.load()?;
        let mut subjects: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> = Default::default();
        for t in &db.tasks {
            *subjects.entry(t.subject.clone()).or_default().entry(t.topic.clone()).or_default() += 1;
        }
        let tags: std::collections::BTreeSet<&String> = db.tasks.iter().flat_map(|t| t.tags.iter()).collect();
        Ok(json!({
            "aufgaben": db.tasks.len(),
            "mappen": db.sheets.len(),
            "faecher": subjects.iter().map(|(f, m)| json!({ "fach": f, "aufgaben": m.values().sum::<usize>(), "schlagwoerter": m })).collect::<Vec<_>>(),
            "tags": tags,
            "notenschluessel": db.settings.schemes.iter().map(|s| &s.name).collect::<Vec<_>>(),
            "pruefungstypen": db.settings.types.iter().map(|t| json!({ "name": t.name, "zaehlt_als_pruefung": t.exam })).collect::<Vec<_>>(),
            "vorlagen": db.settings.templates.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "hinweis": "Schülerdaten (Klassen, Ergebnisse, Noten) sind über diese Schnittstelle nicht zugänglich."
        }))
    }

    fn search(&self, a: &Value) -> Result<Value, String> {
        let db = self.load()?;
        let words: Vec<String> = s(a, "suche").map(|q| q.to_lowercase().split_whitespace().map(String::from).collect()).unwrap_or_default();
        let fach = s(a, "fach").map(|x| x.to_lowercase());
        let sw = s(a, "schlagwort").map(|x| x.to_lowercase());
        let tag = s(a, "tag").map(|x| x.to_lowercase());
        let (minp, maxp) = (n(a, "min_punkte"), n(a, "max_punkte"));
        let diff = s(a, "schwierigkeit");
        let never = a.get("nie_in_klausur").and_then(|v| v.as_bool()).unwrap_or(false);
        let limit = a.get("limit").and_then(|v| v.as_u64()).unwrap_or(30).clamp(1, 200) as usize;
        let hits: Vec<&Task> = db
            .tasks
            .iter()
            .filter(|t| fach.as_ref().is_none_or(|f| t.subject.to_lowercase() == *f))
            .filter(|t| sw.as_ref().is_none_or(|w| t.topic.to_lowercase().contains(w.as_str()) || t.subtopic.to_lowercase().contains(w.as_str())))
            .filter(|t| tag.as_ref().is_none_or(|g| t.tags.iter().any(|x| x.to_lowercase() == *g)))
            .filter(|t| minp.is_none_or(|m| t.points as f64 >= m) && maxp.is_none_or(|m| t.points as f64 <= m))
            .filter(|t| diff.as_deref().is_none_or(|d| difficulty_label(t.difficulty) == d))
            .filter(|t| !never || t.exam_last.is_none())
            .filter(|t| {
                if words.is_empty() {
                    return true;
                }
                let hay = format!(
                    "{} {} {} {} {} {} {}",
                    t.subject,
                    t.topic,
                    t.subtopic,
                    t.subject_line,
                    t.tags.join(" "),
                    doc_for_assistant(&t.task),
                    doc_for_assistant(&t.solution)
                )
                .to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect();
        let total = hits.len();
        Ok(json!({
            "treffer": total,
            "angezeigt": total.min(limit),
            "aufgaben": hits.iter().take(limit).map(|t| task_summary(&db, t)).collect::<Vec<_>>()
        }))
    }

    fn read_tasks(&self, a: &Value) -> Result<Value, String> {
        let db = self.load()?;
        let ids: Vec<u64> = match a.get("ids") {
            Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_u64()).collect(),
            Some(v) => v.as_u64().into_iter().collect(),
            None => a.get("id").and_then(|v| v.as_u64()).into_iter().collect(),
        };
        if ids.is_empty() {
            return Err("Bitte ids angeben, z. B. {\"ids\": [3]}.".into());
        }
        let mut out = Vec::new();
        for id in ids.into_iter().take(20) {
            match db.task(id) {
                Some(t) => {
                    let mut v = task_summary(&db, t);
                    v["aufgabe"] = json!(doc_for_assistant(&t.task));
                    v["loesung"] = json!(doc_for_assistant(&t.solution));
                    v["leerzeilen"] = json!(t.answer_lines);
                    if !t.vars.is_empty() {
                        v["zufallszahlen"] =
                            json!(t.vars.iter().map(|x| json!({ "name": x.name, "von": x.min, "bis": x.max, "schritt": x.step })).collect::<Vec<_>>());
                    }
                    if !t.parts.is_empty() {
                        v["teilaufgaben"] = json!(t
                            .parts
                            .iter()
                            .map(|p| json!({ "bezeichnung": p.label, "punkte": p.points, "erwartung": p.expectation }))
                            .collect::<Vec<_>>());
                    }
                    v.as_object_mut().unwrap().remove("textanfang");
                    out.push(v);
                }
                None => out.push(json!({ "id": id, "fehler": "keine Aufgabe mit dieser id" })),
            }
        }
        Ok(json!(out))
    }

    fn list_sheets(&self, a: &Value) -> Result<Value, String> {
        let db = self.load()?;
        let q = s(a, "suche").map(|x| x.to_lowercase());
        let fach = s(a, "fach").map(|x| x.to_lowercase());
        let typ = s(a, "typ").map(|x| x.to_lowercase());
        let arch = a.get("archivierte").and_then(|v| v.as_bool()).unwrap_or(false);
        let limit = a.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).clamp(1, 500) as usize;
        let list: Vec<Value> = db
            .sheets
            .iter()
            .rev()
            .filter(|s| arch || !s.archived)
            .filter(|s| fach.as_ref().is_none_or(|f| s.subject.to_lowercase() == *f))
            .filter(|s| typ.as_ref().is_none_or(|t| s.kind.to_lowercase() == *t))
            .filter(|s| q.as_ref().is_none_or(|q| format!("{} {} {}", s.title, s.class_name, s.remarks).to_lowercase().contains(q.as_str())))
            .take(limit)
            .map(|s| sheet_summary(&db, s))
            .collect();
        Ok(json!({ "mappen": list }))
    }

    fn read_sheet(&self, a: &Value) -> Result<Value, String> {
        let db = self.load()?;
        let id = a.get("id").and_then(|v| v.as_u64()).ok_or("Bitte id angeben.")?;
        let sh = db.sheets.iter().find(|s| s.id == id).ok_or_else(|| format!("Keine Klausur/Übung mit id {id}."))?;
        let texts = a.get("mit_texten").and_then(|v| v.as_bool()).unwrap_or(false);
        let sols = a.get("mit_loesungen").and_then(|v| v.as_bool()).unwrap_or(false);
        let mut items = Vec::new();
        let mut nr = 0;
        for it in &sh.items {
            match it {
                SheetItem::Task(tid) => {
                    nr += 1;
                    match db.task(*tid) {
                        Some(t) => {
                            let mut v =
                                json!({ "nr": nr, "aufgabe_id": t.id, "titel": t.title(), "fach": t.subject, "schlagwort1": t.topic, "punkte": t.points });
                            if texts {
                                v["aufgabe"] = json!(doc_for_assistant(&t.task));
                            }
                            if sols {
                                v["loesung"] = json!(doc_for_assistant(&t.solution));
                            }
                            items.push(v);
                        }
                        None => items.push(json!({ "nr": nr, "aufgabe_id": tid, "fehler": "Aufgabe gelöscht" })),
                    }
                }
                SheetItem::Text(t) => items.push(json!({ "zusatztext": t })),
            }
        }
        let mut v = sheet_summary(&db, sh);
        v["inhalt"] = json!(items);
        v["vorlage"] = json!(sh.template);
        if let Some(g) = grade_table(&db, &sh.scheme, db.sheet_total(sh) as f32) {
            v["notenschluessel_tabelle"] = g;
        }
        Ok(v)
    }

    fn grades(&self, a: &Value) -> Result<Value, String> {
        let db = self.load()?;
        let total = n(a, "gesamtpunkte").ok_or("Bitte gesamtpunkte angeben.")? as f32;
        if total.is_nan() || total <= 0.0 {
            return Err("gesamtpunkte muss größer als 0 sein.".into());
        }
        let name = s(a, "notenschluessel").unwrap_or_else(|| "IHK".into());
        let sc = db.settings.schemes.iter().find(|s| s.name.eq_ignore_ascii_case(&name)).ok_or_else(|| {
            format!("Notenschlüssel „{name}“ gibt es nicht. Vorhanden: {}", db.settings.schemes.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", "))
        })?;
        let mut v = grade_table(&db, &sc.name, total).unwrap();
        if let Some(p) = n(a, "erreicht") {
            let rows = sc.rows_for(total);
            v["erreicht"] = json!(p);
            v["note"] = json!(sc.grade_for(total, p as f32).map(|i| rows[i].0.clone()));
        }
        Ok(v)
    }

    fn create_task(&self, a: &Value) -> Result<Value, String> {
        let (mut t, problems) = task_from_spec(a)?;
        let mut db = self.load()?;
        // use the spelling already in the collection
        if let Some(f) = db.tasks.iter().map(|x| &x.subject).find(|x| x.eq_ignore_ascii_case(&t.subject)) {
            t.subject = f.clone();
        }
        if let Some(f) = db.tasks.iter().filter(|x| x.subject == t.subject).map(|x| &x.topic).find(|x| x.eq_ignore_ascii_case(&t.topic)) {
            t.topic = f.clone();
        }
        t.id = db.new_external_id();
        t.author_new = ORIGIN.into();
        t.origin = ORIGIN.into();
        t.created = Some(chrono::Local::now().naive_local());
        t.updated = t.created;
        if let Some(dup) = db.tasks.iter().find(|x| x.subject == t.subject && x.topic == t.topic && x.task.text.trim() == t.task.text.trim()) {
            return Err(format!("Diese Aufgabe gibt es schon (id {}). Es wurde nichts angelegt.", dup.id));
        }
        let id = t.id;
        db.tasks.push(t);
        self.save(&db)?;
        let mut v = json!({ "angelegt": true, "id": id, "hinweis": "Die Aufgabe erscheint in Klassenwerk im Pool (gekennzeichnet als KI-Assistent)." });
        if !problems.is_empty() {
            v["formel_warnungen"] = json!(problems);
        }
        Ok(v)
    }

    fn create_sheet(&self, a: &Value) -> Result<Value, String> {
        let title = s(a, "titel").ok_or("titel fehlt.")?;
        let ids: Vec<u64> = a.get("aufgaben").and_then(|v| v.as_array()).map(|v| v.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
        if ids.is_empty() {
            return Err("aufgaben (Liste von Aufgaben-ids) fehlt.".into());
        }
        let mut db = self.load()?;
        let missing: Vec<u64> = ids.iter().copied().filter(|id| db.task(*id).is_none()).collect();
        if !missing.is_empty() {
            return Err(format!("Diese Aufgaben gibt es nicht: {missing:?}. Es wurde nichts angelegt."));
        }
        let mut seen = std::collections::HashSet::new();
        if ids.iter().any(|id| !seen.insert(*id)) {
            return Err("Eine Aufgabe kommt doppelt vor. Es wurde nichts angelegt.".into());
        }
        let kind = s(a, "typ").unwrap_or_else(|| "Klausur".into());
        let kind = db.settings.types.iter().find(|t| t.name.eq_ignore_ascii_case(&kind)).map(|t| t.name.clone()).ok_or_else(|| {
            format!("Typ „{kind}“ gibt es nicht. Vorhanden: {}", db.settings.types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", "))
        })?;
        let mut sh = Sheet { title, kind: kind.clone(), ..Default::default() };
        if let Some(sc) = s(a, "notenschluessel") {
            sh.scheme = db
                .settings
                .schemes
                .iter()
                .find(|x| x.name.eq_ignore_ascii_case(&sc))
                .map(|x| x.name.clone())
                .ok_or_else(|| format!("Notenschlüssel „{sc}“ gibt es nicht."))?;
        } else if !db.settings.is_exam(&kind) {
            sh.scheme.clear();
        }
        let tpl = s(a, "vorlage").unwrap_or_else(|| if db.settings.is_exam(&kind) { "Klausur".into() } else { kind.clone() });
        sh.template = db
            .settings
            .templates
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&tpl))
            .or(db.settings.templates.first())
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let first = db.task(ids[0]).unwrap();
        sh.subject = s(a, "fach").unwrap_or_else(|| first.subject.clone());
        sh.class_name = s(a, "klasse").unwrap_or_default();
        sh.date = s(a, "datum").unwrap_or_default();
        if let Some(m) = n(a, "dauer_min") {
            sh.minutes = m.clamp(0.0, 600.0).round() as u32;
        }
        if let Some(h) = s(a, "hinweis") {
            sh.items.push(SheetItem::Text(h));
        }
        sh.items.extend(ids.iter().map(|id| SheetItem::Task(*id)));
        sh.created = Some(chrono::Local::now().naive_local());
        sh.remarks = format!("Entwurf, angelegt vom {ORIGIN}.");
        sh.origin = ORIGIN.into();
        sh.id = db.new_external_id();
        let id = sh.id;
        let total = db.sheet_total(&sh);
        let scheme = sh.scheme.clone();
        db.sheets.push(sh);
        self.save(&db)?;
        let mut v =
            json!({ "angelegt": true, "id": id, "gesamtpunkte": total, "hinweis": "Der Entwurf erscheint in Klassenwerk unter „Klausuren & Übungen“." });
        if let Some(g) = grade_table(&db, &scheme, total as f32) {
            v["notenschluessel_tabelle"] = g;
        }
        Ok(v)
    }

    /// Handles one JSON-RPC message; returns the response (None for notifications).
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let id = id?; // notifications (initialized, cancelled …) need no answer
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOLS[0]);
                let version = if PROTOCOLS.contains(&asked) { asked } else { PROTOCOLS[0] };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "klassenwerk", "title": "Klassenwerk – Aufgaben & Klausuren", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": Self::tools() })),
            "tools/call" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(self.call(name, &args))
            }
            "resources/list" => Ok(json!({ "resources": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            _ => Err((-32601, format!("Methode nicht unterstützt: {method}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, m)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": m } }),
        })
    }
}

/// Runs the server on stdin/stdout until stdin closes. Messages are one JSON object per line.
pub fn run(db_path: Option<PathBuf>) {
    let server = Server::new(db_path);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) => {
                let v: Vec<Value> = batch.iter().filter_map(|m| server.handle(m)).collect();
                if v.is_empty() {
                    None
                } else {
                    Some(Value::Array(v))
                }
            }
            Ok(m) => server.handle(&m),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Ungültiges JSON: {e}") } })),
        };
        if let Some(r) = reply {
            let _ = writeln!(out, "{r}");
            let _ = out.flush();
        }
    }
}

// ------------------------------------------------------------------ setup in Claude Desktop

/// The server entry for a client configuration.
pub fn server_entry(exe: &str) -> Value {
    json!({ "command": exe, "args": ["--mcp"] })
}

/// Configuration snippet to paste into claude_desktop_config.json.
pub fn config_snippet(exe: &str) -> String {
    serde_json::to_string_pretty(&json!({ "mcpServers": { "klassenwerk": server_entry(exe) } })).unwrap_or_default()
}

/// Command line that adds the server to Claude Code.
pub fn claude_code_command(exe: &str) -> String {
    format!("claude mcp add klassenwerk -- \"{exe}\" --mcp")
}

/// Claude Desktop configuration files: the normal one, plus the ones of a
/// Microsoft Store (MSIX) installation, which reads from its own folder.
pub fn claude_desktop_configs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(a) = std::env::var("APPDATA") {
        out.push(PathBuf::from(a).join("Claude").join("claude_desktop_config.json"));
    } else if let Ok(h) = std::env::var("HOME") {
        let h = PathBuf::from(h);
        if cfg!(target_os = "macos") {
            out.push(h.join("Library/Application Support/Claude/claude_desktop_config.json"));
        } else {
            out.push(h.join(".config/Claude/claude_desktop_config.json"));
        }
    }
    if let Ok(l) = std::env::var("LOCALAPPDATA") {
        if let Ok(rd) = std::fs::read_dir(PathBuf::from(l).join("Packages")) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with("Anthropic.Claude") {
                    for sub in ["LocalCache/Roaming/Claude", "LocalCache/Local/Claude"] {
                        let dir = e.path().join(sub);
                        if dir.is_dir() {
                            out.push(dir.join("claude_desktop_config.json"));
                        }
                    }
                }
            }
        }
    }
    out
}

/// Adds (or updates) the "klassenwerk" entry (and removes the old "kldozent" one) in a client configuration file and keeps
/// all other settings. A backup of the old file is written next to it.
pub fn register_in(path: &std::path::Path, exe: &str) -> Result<(), String> {
    let mut root = if path.exists() {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        if text.trim().is_empty() {
            json!({})
        } else {
            let v: Value =
                serde_json::from_str(&text).map_err(|_| format!("{} ist kein gültiges JSON. Bitte den Eintrag von Hand ergänzen.", path.display()))?;
            if !v.is_object() {
                return Err(format!("{} hat einen unerwarteten Aufbau.", path.display()));
            }
            let _ = std::fs::write(path.with_extension("json.bak-klassenwerk"), &text);
            v
        }
    } else {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        json!({})
    };
    let servers = root.as_object_mut().unwrap().entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    // the entry of the time the program was called KlDozent would start the old program file
    if let Some(m) = servers.as_object_mut() {
        m.remove("kldozent");
    }
    servers["klassenwerk"] = server_entry(exe);
    std::fs::write(path, serde_json::to_string_pretty(&root).unwrap()).map_err(|e| e.to_string())
}
