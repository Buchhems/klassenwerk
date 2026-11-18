#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod classes;
mod delphi;
mod docx;
mod export;
mod fonts;
mod formula;
mod markup;
mod math;
mod mcp;
mod model;
mod moodle;
mod rvf;
mod spell;
mod table;
#[cfg(test)]
mod tests;
mod thesaurus;
mod update;
mod vary;
mod wmf;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut shot: Option<PathBuf> = None;
    let mut i = 1;
    // data of the time the program was called KlDozent
    model::Database::migrate_old_data();
    // MCP server for AI assistants: no window, JSON-RPC on stdin/stdout
    if let Some(p) = args.iter().position(|a| a == "--mcp") {
        let db = args.iter().position(|a| a == "--mcp-db").and_then(|i| args.get(i + 1)).map(PathBuf::from);
        let _ = p;
        mcp::run(db);
        return Ok(());
    }
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" if i + 1 < args.len() => {
                shot = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--export-test" if i + 1 < args.len() => {
                let dir = PathBuf::from(&args[i + 1]);
                let _ = std::fs::create_dir_all(&dir);
                let mut db = model::Database::with_samples();
                let mut s = db.sheets[0].clone();
                s.results = [34.0, 27.5, 40.0, 12.0, 22.0, 30.5].iter().map(|p| model::ResultRow { points: *p, ..Default::default() }).collect();
                s.print_spiegel = true;
                db.sheets[0] = s.clone();
                let c = export::compose(&db, &s, false);
                let _ = std::fs::write(dir.join("klausur.html"), export::to_html(&db, &c, true));
                let _ = std::fs::write(dir.join("klausur.rtf"), export::to_rtf(&db, &c, true));
                let c = export::compose(&db, &s, true);
                let _ = std::fs::write(dir.join("loesung.html"), export::to_html(&db, &c, true));
                let c = export::compose_spiegel(&db, &s);
                let _ = std::fs::write(dir.join("notenspiegel.html"), export::to_html(&db, &c, true));
                let _ = std::fs::write(dir.join("notenspiegel.docx"), docx::to_docx(&db, &c, true));
                let _ = std::fs::write(dir.join("notenspiegel.rtf"), export::to_rtf(&db, &c, true));
                // Teilaufgaben with Erwartungshorizont, points entered per Teilaufgabe
                {
                    let mut db = db.clone();
                    let mut s = s.clone();
                    for id in s.task_ids() {
                        if let Some(t) = db.task_mut(id) {
                            let a = t.points / 2;
                            t.parts = vec![
                                model::Part { label: "a".into(), points: a, expectation: "Ansatz und Rechenweg".into() },
                                model::Part { label: "b".into(), points: t.points - a, expectation: String::new() },
                            ];
                        }
                    }
                    let cols = db.score_columns(&s);
                    for (i, r) in s.results.iter_mut().enumerate() {
                        for (k, c) in cols.iter().enumerate() {
                            r.scores
                                .insert(c.key.clone(), (c.max * [0.9, 0.6, 1.0, 0.3, 0.7, 0.8][i] * if k % 2 == 0 { 1.0 } else { 0.8 } * 2.0).round() / 2.0);
                        }
                        r.sum_scores(&cols);
                    }
                    let c = export::compose_expectation(&db, &s);
                    let _ = std::fs::write(dir.join("erwartungshorizont.html"), export::to_html(&db, &c, false));
                    let _ = std::fs::write(dir.join("erwartungshorizont.docx"), docx::to_docx(&db, &c, false));
                    let c = export::compose_spiegel(&db, &s);
                    let _ = std::fs::write(dir.join("notenspiegel_teilaufgaben.html"), export::to_html(&db, &c, true));
                    let _ = std::fs::write(dir.join("ergebnisse_vorlage.csv"), classes::results_template(&cols, &["Becker, Anna".to_string()]));
                }
                let tasks: Vec<&model::Task> = db.tasks.iter().collect();
                let xml = model::export_xml(&tasks);
                let _ = std::fs::write(dir.join("aufgaben.xml"), &xml);
                let mut db2 = model::Database::default();
                let back = model::import_xml(&xml, &mut db2).map(|t| t.len()).unwrap_or(0);
                println!("XML round trip: {} -> {} tasks", db.tasks.len(), back);
                // parallel version (Gruppe B) and a class with a grade book
                let mut a = db.sheets[0].clone();
                a.group = "A".into();
                db.sheets[0] = a.clone();
                let nid = db.new_id();
                let par = db.make_parallel(&a, model::ParallelOptions { replace: true, shuffle: true, tolerance: 2 }, 11, nid);
                println!("Gruppe {}: {} ersetzt, {} behalten, Punktdifferenz {}", par.sheet.group, par.replaced, par.kept, par.points_diff);
                let b = par.sheet;
                let c = export::compose(&db, &b, false);
                let _ = std::fs::write(dir.join("gruppe_b.html"), export::to_html(&db, &c, true));
                db.sheets.push(b.clone());
                let mut course =
                    classes::Course { id: 9000, name: "12a".into(), subject: "Mathematik".into(), school_year: "2026/27".into(), ..Default::default() };
                let names: Vec<String> =
                    ["Becker, Anna", "Dorn, Ben", "Ernst, Clara", "Fischer, David", "Graf, Emma", "Huber, Finn"].iter().map(|s| s.to_string()).collect();
                course.add_students(&names, &mut db.next_id);
                let total = db.sheet_total(&a) as f32;
                let pts = [0.95, 0.62, 0.78, 0.41, 0.88, 0.55];
                for (i, st) in course.students.iter().enumerate() {
                    let sheet = if i % 2 == 0 { 0 } else { db.sheets.len() - 1 };
                    db.sheets[sheet].course_id = Some(course.id);
                    db.sheets[sheet].results.push(model::ResultRow {
                        name: st.name.clone(),
                        student: Some(st.id),
                        points: (total * pts[i] * 2.0).round() / 2.0,
                        absent: i == 5,
                        ..Default::default()
                    });
                }
                let mut oral = classes::Assessment { id: 9001, title: "Mündlich".into(), ..Default::default() };
                for (st, g) in course.students.iter().zip(["2", "3-", "1", "4", "2+", "3"]) {
                    classes::set_grade(&mut oral.grades, st.id, g);
                }
                course.assessments.push(oral);
                db.courses.push(course.clone());
                let gb = classes::gradebook(&db, &course);
                let _ = std::fs::write(dir.join("notenbuch.csv"), classes::to_csv(&gb, &course));
                let c = export::Composed {
                    title: "Notenbuch 12a".into(),
                    sections: vec![export::Section::text(&classes::to_markup(&gb, &course))],
                    page_numbers: true,
                    footer: String::new(),
                };
                let _ = std::fs::write(dir.join("notenbuch.html"), export::to_html(&db, &c, false));
                // new formulas: one task using several, as HTML and RTF
                let formulas = [
                    r"\frac{a}{b} + \frac{1}{2}",
                    r"x_{1,2} = -\frac{p}{2} \pm \sqrt{\left(\frac{p}{2}\right)^2 - q}",
                    r"\int_{0}^{1} x^2 \, dx = \left[ \frac{1}{3} x^3 \right]_{0}^{1} = \frac{1}{3}",
                    r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}",
                    r"\lim_{x \to \infty} \left( 1 + \frac{1}{x} \right)^x = e",
                    r"\vec{v} = \frac{\Delta s}{\Delta t} \quad \sqrt[3]{27} = 3",
                    r"f'(x) = 3x^2 - \sin(\alpha) \cdot \cos \beta, \; x \in \mathbb{R}",
                    r"K_n = K_0 \cdot \left( 1 + \frac{p}{100} \right)^n",
                    r"A = \begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}, \; \det \begin{vmatrix} a & b \\ c & d \end{vmatrix} = ad - bc",
                    r"|x| = \begin{cases} x & \text{für } x \ge 0 \\ -x & \text{für } x < 0 \end{cases}",
                    r"(a+b)^2 &= (a+b)(a+b) \\ &= a^2 + 2ab + b^2",
                    r"\vec{v} = \begin{bmatrix} 1 \\ 0 \\ -1 \end{bmatrix}",
                ];
                let mut t = db.tasks[0].clone();
                t.id = db.new_id();
                t.subtopic = "Formeltest".into();
                t.task.objects.clear();
                t.task.text = String::from("Neue Formeln aus dem Formeleditor:\n");
                for (k, f) in formulas.iter().enumerate() {
                    let d = math::layout(f);
                    let (w, h) = math::size_px(&d);
                    t.task.objects.push(model::DocObject { id: k as u32 + 1, kind: model::ObjKind::Math, data: f.as_bytes().to_vec(), width: w, height: h });
                    t.task.text += &format!("{}) Im Text {{{{{}}}}} eingebettet.\n", k + 1, k + 1);
                }
                t.answer_lines = 0;
                db.tasks.push(t.clone());
                let mut s = db.sheets[0].clone();
                s.items = vec![model::SheetItem::Task(t.id)];
                s.title = "Formeltest".into();
                let c = export::compose(&db, &s, false);
                let _ = std::fs::write(dir.join("formeln.html"), export::to_html(&db, &c, false));
                let _ = std::fs::write(dir.join("formeln.rtf"), export::to_rtf(&db, &c, false));
                let _ = std::fs::write(dir.join("formeln.docx"), docx::to_docx(&db, &c, false));
                // the sample exam with old formulas, pictures and tables as DOCX
                let s0 = db.sheets[0].clone();
                let c = export::compose(&db, &s0, false);
                let _ = std::fs::write(dir.join("klausur.docx"), docx::to_docx(&db, &c, true));
                let c = export::compose(&db, &s0, true);
                let _ = std::fs::write(dir.join("loesung.docx"), docx::to_docx(&db, &c, true));
                let all: Vec<model::SheetItem> = db.tasks.iter().map(|t| model::SheetItem::Task(t.id)).collect();
                let mut s_all = s0.clone();
                s_all.items = all;
                s_all.title = "Alle Beispielaufgaben".into();
                let c = export::compose(&db, &s_all, true);
                let _ = std::fs::write(dir.join("alle.docx"), docx::to_docx(&db, &c, true));
                let _ = std::fs::write(dir.join("alle.html"), export::to_html(&db, &c, true));
                return Ok(());
            }
            // screenshot helpers: --demo (class with results), --open settings:N | edit:ID | formula:ID
            "--demo" => std::env::set_var("KLASSENWERK_DEMO", "1"),
            "--dark" => std::env::set_var("KLASSENWERK_DARK", "1"),
            "--open" if i + 1 < args.len() => {
                std::env::set_var("KLASSENWERK_OPEN", &args[i + 1]);
                i += 1;
            }
            "--tab" if i + 1 < args.len() => {
                std::env::set_var("KLASSENWERK_TAB", &args[i + 1]);
                i += 1;
            }
            "--task" if i + 1 < args.len() => {
                std::env::set_var("KLASSENWERK_TASK", &args[i + 1]);
                i += 1;
            }
            "--dump" => {
                let db = model::Database::with_samples();
                for t in &db.tasks {
                    println!("=== #{} {} / {} / {}  ({} P.)", t.id, t.subject, t.topic, t.subtopic, t.points);
                    println!("--- Aufgabe ({} Objekte)\n{}", t.task.objects.len(), t.task.text);
                    println!("--- Lösung ({} Objekte)\n{}", t.solution.objects.len(), t.solution.text);
                    for o in t.task.objects.iter().chain(t.solution.objects.iter()) {
                        let ok = match o.kind {
                            model::ObjKind::Formula => {
                                formula::parse_wmf(&o.data).map(|d| format!("{} prims ext {:?}", d.prims.len(), d.ext)).unwrap_or("FAIL".into())
                            }
                            model::ObjKind::Math => format!("Formel {}", String::from_utf8_lossy(&o.data)),
                            model::ObjKind::Image => {
                                image::load_from_memory(&o.data).map(|i| format!("{}x{}", i.width(), i.height())).unwrap_or_else(|e| format!("FAIL {e}"))
                            }
                        };
                        println!("    obj {} {:?} {}x{} -> {}", o.id, o.kind, o.width, o.height, ok);
                        if std::env::var("KLASSENWERK_PRIMS").ok().and_then(|v| v.parse::<u64>().ok()) == Some(t.id) {
                            if let Some(d) = formula::parse_wmf(&o.data) {
                                println!("      org {:?} ext {:?}", d.org, d.ext);
                                for p in &d.prims {
                                    println!("      {:?}", p);
                                }
                            }
                        }
                    }
                }
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Klassenwerk – Aufgaben, Klausuren & Noten")
        // KLASSENWERK_HEIGHT: a taller window for screenshots of long views
        .with_inner_size([1360.0, std::env::var("KLASSENWERK_HEIGHT").ok().and_then(|v| v.parse().ok()).unwrap_or(840.0)])
        .with_min_inner_size([900.0, 560.0])
        .with_app_id("klassenwerk");
    // window / taskbar icon: on Windows the small form (only the K) like the .exe at
    // 16–32 px, elsewhere the full symbol
    let icon: &[u8] = if cfg!(windows) { include_bytes!("../assets/logo-64.png") } else { include_bytes!("../assets/icon-256.png") };
    if let Ok(img) = image::load_from_memory(icon) {
        let img = img.to_rgba8();
        let (width, height) = img.dimensions();
        viewport = viewport.with_icon(std::sync::Arc::new(egui::IconData { rgba: img.into_raw(), width, height }));
    }
    if shot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot.is_some() {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    let _ = &mut options;
    eframe::run_native(
        "Klassenwerk",
        options,
        Box::new(move |cc| {
            fonts::setup(&cc.egui_ctx);
            Ok(Box::new(app::App::new(&cc.egui_ctx, shot)))
        }),
    )
}
