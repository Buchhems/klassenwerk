//! Export of tasks as Moodle XML questions (Fragensammlung → Importieren →
//! „Moodle-XML-Format“). Every task becomes a Freitext question (essay) with its
//! points as the default grade, the model solution as general feedback and the
//! Erwartungshorizont as information for the graders. Pictures and old formula
//! pictures travel inside the file; new formulas become TeX (\( … \)), which
//! Moodle shows with its MathJax filter. Random numbers get the example values.

use crate::markup::{html_escape, to_html_with};
use crate::model::{Doc, DocObject, ObjKind, Task};
use base64::Engine;

/// Category path below the course: "Klassenwerk/Fach/Thema".
fn category(t: &Task) -> String {
    let mut c = vec!["Klassenwerk".to_string()];
    for p in [&t.subject, &t.topic] {
        let p = p.trim().replace('/', "-");
        if !p.is_empty() {
            c.push(p);
        }
    }
    c.join("/")
}

/// Formula source as TeX for MathJax: a multi-line formula without its own
/// environment becomes `aligned`.
pub fn tex(src: &str) -> String {
    let s = src.trim().replace(['\n', '\r'], " ");
    if s.contains("\\\\") && !s.contains("\\begin") {
        format!("\\begin{{aligned}}{s}\\end{{aligned}}")
    } else {
        s
    }
}

/// HTML of a document for Moodle plus the files it uses (name, bytes).
fn html(doc: &Doc, prefix: &str) -> (String, Vec<(String, Vec<u8>)>) {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut obj = |o: &DocObject| -> String {
        match o.kind {
            ObjKind::Math => format!("\\({}\\)", html_escape(&tex(&String::from_utf8_lossy(&o.data)))),
            ObjKind::Image => {
                let ext = if o.data.starts_with(b"\x89PNG") {
                    "png"
                } else if o.data.starts_with(b"GIF") {
                    "gif"
                } else if o.data.starts_with(b"BM") {
                    "bmp"
                } else {
                    "jpg"
                };
                let name = format!("{prefix}bild{}.{ext}", o.id);
                let size = if o.width > 0.0 { format!(" width=\"{:.0}\" height=\"{:.0}\"", o.width, o.height) } else { String::new() };
                files.push((name.clone(), o.data.clone()));
                format!("<img src=\"@@PLUGINFILE@@/{name}\" alt=\"\"{size} style=\"vertical-align:middle;max-width:100%\">")
            }
            ObjKind::Formula => {
                // the old formula pictures as SVG files
                let svg = crate::markup::obj_html(o);
                if svg.is_empty() {
                    return String::new();
                }
                let svg = if svg.contains("xmlns=") { svg } else { svg.replacen("<svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"", 1) };
                let name = format!("{prefix}formel{}.svg", o.id);
                files.push((name.clone(), svg.into_bytes()));
                format!("<img src=\"@@PLUGINFILE@@/{name}\" alt=\"Formel\" style=\"vertical-align:middle\">")
            }
        }
    };
    let h = to_html_with(doc, false, false, &mut obj)
        // Moodle has no style sheet of ours: tables with lines, boxes as characters
        .replace("<table class=\"grid\">", "<table border=\"1\" cellpadding=\"4\" style=\"border-collapse:collapse\">")
        .replace("<table class=\"plain\">", "<table cellpadding=\"4\">")
        .replace("<span class=\"box on\">✗</span>", "☒ ")
        .replace("<span class=\"box\"></span>", "☐ ")
        .replace("<div class=\"gap\"></div>", "<p></p>")
        .replace("<p class=\"li\">", "<p>");
    (h, files)
}

fn cdata(s: &str) -> String {
    format!("<![CDATA[{}]]>", s.replace("]]>", "]]]]><![CDATA[>"))
}

fn text_el(tag: &str, html: &str, files: &[(String, Vec<u8>)]) -> String {
    let mut s = format!("    <{tag} format=\"html\">\n      <text>{}</text>\n", cdata(html));
    for (name, data) in files {
        s += &format!(
            "      <file name=\"{}\" path=\"/\" encoding=\"base64\">{}</file>\n",
            html_escape(name),
            base64::engine::general_purpose::STANDARD.encode(data)
        );
    }
    s += &format!("    </{tag}>\n");
    s
}

/// Moodle XML with one question per task, sorted into categories by subject and topic.
pub fn export(tasks: &[&Task]) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<quiz>\n");
    let mut last_cat = String::new();
    for t in tasks {
        let t = t.varied(0);
        let cat = category(&t);
        if cat != last_cat {
            s += &format!(
                "  <question type=\"category\">\n    <category>\n      <text>$course$/top/{}</text>\n    </category>\n  </question>\n",
                html_escape(&cat)
            );
            last_cat = cat;
        }
        let (q, qf) = html(&t.task, "a");
        let (sol, sf) = html(&t.solution, "l");
        // Erwartungshorizont for the graders: parts with points, otherwise the model solution
        let mut grader = String::new();
        if !t.parts.is_empty() {
            grader +=
                "<table border=\"1\" cellpadding=\"4\" style=\"border-collapse:collapse\"><tr><th>Teil</th><th>Erwartete Leistung</th><th>Punkte</th></tr>";
            for p in &t.parts {
                let e = if p.expectation.trim().is_empty() { "–".to_string() } else { html_escape(p.expectation.trim()).replace('\n', "<br>") };
                grader += &format!("<tr><td>{}</td><td>{e}</td><td>{}</td></tr>", html_escape(&p.label), p.points);
            }
            grader += &format!("<tr><td></td><td><b>Summe</b></td><td><b>{}</b></td></tr></table>", t.points);
        }
        let mut gfiles = Vec::new();
        if !sol.trim().is_empty() {
            let (gs, gf) = html(&t.solution, "e");
            grader += &format!("<p><b>Musterlösung</b></p>{gs}");
            gfiles = gf;
        }
        let lines = if t.answer_lines > 0 { t.answer_lines.clamp(5, 40) } else { 15 };
        s += "  <question type=\"essay\">\n";
        s += &format!("    <name>\n      <text>{}</text>\n    </name>\n", html_escape(&t.title()));
        s += &text_el("questiontext", &q, &qf);
        s += &text_el("generalfeedback", &sol, &sf);
        s += &format!("    <defaultgrade>{}</defaultgrade>\n    <penalty>0</penalty>\n    <hidden>0</hidden>\n    <idnumber></idnumber>\n", t.points.max(0));
        s += &format!(
            "    <responseformat>editor</responseformat>\n    <responserequired>1</responserequired>\n    <responsefieldlines>{lines}</responsefieldlines>\n    <minwordlimit></minwordlimit>\n    <maxwordlimit></maxwordlimit>\n    <attachments>0</attachments>\n    <attachmentsrequired>0</attachmentsrequired>\n    <maxbytes>0</maxbytes>\n    <filetypeslist></filetypeslist>\n"
        );
        s += &text_el("graderinfo", &grader, &gfiles);
        s += "    <responsetemplate format=\"html\">\n      <text></text>\n    </responsetemplate>\n";
        if !t.tags.is_empty() {
            s += "    <tags>\n";
            for tag in &t.tags {
                s += &format!("      <tag><text>{}</text></tag>\n", html_escape(tag));
            }
            s += "    </tags>\n";
        }
        s += "  </question>\n";
    }
    s += "</quiz>\n";
    s
}
