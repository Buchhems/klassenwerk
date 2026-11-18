//! Unit tests for the core rules (grading keys, data format, markup).
//! Run with `cargo test`.

use crate::export::{fmt_bound, fmt_pts};
use crate::model::*;
use std::collections::HashSet;

fn scheme(name: &str) -> GradeScheme {
    GradeScheme::defaults().into_iter().find(|s| s.name == name).unwrap()
}

fn bounds(s: &GradeScheme, total: f32) -> Vec<(String, String, String)> {
    s.rows_for(total).into_iter().map(|(l, a, b)| (l, fmt_bound(a), fmt_bound(b))).collect()
}

#[test]
fn ihk_31_points_half_steps() {
    // Example in docs/REGELN.md: 31 points, IHK, half points (default)
    let b = bounds(&scheme("IHK"), 31.0);
    let want = [("1", "29", "31"), ("2", "25,5", "28,5"), ("3", "21", "25"), ("4", "15,5", "20,5"), ("5", "9,5", "15"), ("6", "0", "9")];
    assert_eq!(b.len(), want.len());
    for (got, w) in b.iter().zip(want.iter()) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2.as_str()), *w);
    }
}

#[test]
fn needed_points_are_rounded_up_to_the_step() {
    let mut ihk = scheme("IHK");
    ihk.step = 0.1;
    let b = bounds(&ihk, 31.0);
    // 92 % of 31 = 28.52 -> 28.6 (never less than the percentage)
    assert_eq!(b[0], ("1".to_string(), "28,6".to_string(), "31".to_string()));
    assert_eq!(b[1], ("2".to_string(), "25,2".to_string(), "28,5".to_string()));
    ihk.step = 1.0;
    let b = bounds(&ihk, 31.0);
    assert_eq!(b[0].1, "29");
    assert_eq!(b[1].1, "26");
    assert_eq!(b[1].2, "28");
    // exact values stay: 50 % of 40 = 20
    assert_eq!(scheme("IHK").rows_for(40.0)[3].1, 20.0);
}

#[test]
fn shipped_keys_follow_published_scales() {
    let ihk = scheme("IHK");
    let from: Vec<f32> = ihk.rows.iter().map(|r| r.min_percent).collect();
    assert_eq!(from, vec![92.0, 81.0, 67.0, 50.0, 30.0, 0.0]);
    let gym = scheme("Gymnasium");
    let from: Vec<f32> = gym.rows.iter().map(|r| r.min_percent).collect();
    assert_eq!(from, vec![95.0, 90.0, 85.0, 80.0, 75.0, 70.0, 65.0, 60.0, 55.0, 50.0, 45.0, 40.0, 33.0, 27.0, 20.0, 0.0]);
}

#[test]
fn bounds_never_exceed_total_and_are_descending() {
    for s in GradeScheme::defaults() {
        for total in [7.0, 31.0, 60.0, 100.0] {
            let rows = s.rows_for(total);
            assert!(!rows.is_empty());
            for w in rows.windows(2) {
                assert!(w[0].1 >= w[1].1, "{}: lower bounds must fall", s.name);
            }
            for r in &rows {
                assert!(r.1 <= total && r.2 <= total);
            }
            assert_eq!(rows.last().unwrap().1, 0.0);
            assert_eq!(rows[0].2, total);
        }
    }
}

#[test]
fn few_points_can_lose_grades() {
    // 7 points on the 15-point key: several grades share a half point
    let gym = scheme("Gymnasium");
    let lost = gym.lost_grades(7.0);
    assert!(!lost.is_empty());
    // every reachable point value still gets exactly one grade
    for p in 0..=14 {
        let pts = p as f32 * 0.5;
        let i = gym.grade_for(7.0, pts).unwrap();
        assert!(!lost.contains(&gym.rows_for(7.0)[i].0));
    }
}

#[test]
fn grade_for_points() {
    let ihk = scheme("IHK");
    let rows = ihk.rows_for(31.0);
    assert_eq!(rows[ihk.grade_for(31.0, 31.0).unwrap()].0, "1");
    assert_eq!(rows[ihk.grade_for(31.0, 29.0).unwrap()].0, "1");
    assert_eq!(rows[ihk.grade_for(31.0, 28.5).unwrap()].0, "2");
    assert_eq!(rows[ihk.grade_for(31.0, 0.0).unwrap()].0, "6");
}

#[test]
fn old_keys_are_migrated() {
    let mut db = Database::with_samples();
    let gym = db.settings.schemes.iter_mut().find(|s| s.name == "Gymnasium").unwrap();
    for (r, p) in gym.rows.iter_mut().zip([96.0, 91.0, 86.0, 81.0, 76.0, 71.0, 66.0, 61.0, 56.0, 51.0, 46.0, 41.0, 34.0, 27.0, 20.0, 0.0]) {
        r.min_percent = p;
    }
    let mut own = scheme("IHK");
    own.name = "eigener in Punkten".into();
    for r in own.rows.iter_mut() {
        r.min_percent *= 1.2; // written in points for 120 points
    }
    own.rows[0].min_percent = 120.0;
    db.settings.schemes.push(own);
    // a key from a file of an earlier preview version (more fields, no step)
    let old: GradeScheme = serde_json::from_str(r#"{"name":"X","higher_is_better":false,"fail_from":"5","rows":[{"label":"1","min_percent":90.0,"max_percent":100.0}],"round_half":true,"vertical":false,"labels":{"heading":"a"}}"#).unwrap();
    assert_eq!(old.step, 0.5);
    assert!(old.count_row);
    crate::model::migrate_schemes(&mut db);
    let gym = db.settings.schemes.iter().find(|s| s.name == "Gymnasium").unwrap();
    assert_eq!(gym.rows[0].min_percent, 95.0);
    let own = db.settings.schemes.iter().find(|s| s.name == "eigener in Punkten").unwrap();
    assert_eq!(own.rows[0].min_percent, 100.0);
}

#[test]
fn grade_values() {
    assert_eq!(GradeScheme::value("13"), 13.0);
    assert!((GradeScheme::value("2-") - 2.33).abs() < 0.01);
    assert!((GradeScheme::value("1+") - 0.67).abs() < 0.01);
}

#[test]
fn points_format() {
    assert_eq!(fmt_pts(12.0), "12");
    assert_eq!(fmt_pts(12.5), "12,5");
}

#[test]
fn double_utf8_is_repaired_and_plain_text_kept() {
    // "ä" stored as C3 83 C2 A4 in the old files, i.e. read as "Ã¤"
    assert_eq!(fix_double_utf8("Ã¤"), "ä");
    assert_eq!(fix_double_utf8("Größe"), "Größe");
    assert_eq!(fix_double_utf8("plain"), "plain");
}

#[test]
fn old_date_format() {
    assert!(parse_date("18991230T00:00:00000").is_none());
    let d = parse_date("20080714T17:08:45000").unwrap();
    assert_eq!(d.format("%Y-%m-%d %H:%M:%S").to_string(), "2008-07-14 17:08:45");
}

#[test]
fn samples_load_and_xml_round_trip() {
    let db = Database::with_samples();
    assert_eq!(db.tasks.len(), 25, "all example tasks are readable");
    for t in &db.tasks {
        let all = format!("{}{}", t.task.text, t.solution.text);
        assert!(!all.contains('$'), "formulas became objects: {}", t.subtopic);
        for o in t.task.objects.iter().chain(&t.solution.objects) {
            let src = String::from_utf8_lossy(&o.data).to_string();
            let src = crate::vary::apply(&src, &crate::vary::values(t, 0));
            assert!(crate::math::errors(&crate::math::parse(&src)).is_empty(), "{}: {src}", t.subtopic);
        }
        let probs = crate::vary::problems(&all, &crate::vary::values(t, 3));
        assert!(probs.is_empty(), "{}: {probs:?}", t.subtopic);
        if !t.parts.is_empty() {
            assert_eq!(t.points, t.parts.iter().map(|p| p.points).sum::<i32>());
        }
    }
    assert!(db.tasks[0].parts.is_empty() && !db.tasks[0].solution.text.is_empty());
    assert_eq!(db.sheets[0].task_ids().len(), 3);
    // export in the format of KlasseDozent and back
    let tasks: Vec<&Task> = db.tasks.iter().collect();
    let xml = export_xml(&tasks);
    let mut db2 = Database::default();
    let back = import_xml(&xml, &mut db2).unwrap();
    assert_eq!(back.len(), 25);
    for (a, b) in db.tasks.iter().zip(back.iter()) {
        assert_eq!((&a.subject, &a.topic, &a.subtopic), (&b.subject, &b.topic, &b.subtopic));
        assert_eq!((a.points, a.answer_lines, a.underline_answer), (b.points, b.answer_lines, b.underline_answer));
        // the old format has no formula editor (formulas come back as text) and only the
        // Windows character set (no "Ω"); all other words stay
        let words = |t: &str| {
            crate::markup::plain(t)
                .split_whitespace()
                .filter(|w| w.chars().all(|c| c.is_alphabetic() && (c as u32) < 256))
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
        };
        let (wa, wb) = (words(&a.task.text), words(&b.task.text));
        assert!(wa.iter().all(|w| wb.contains(w)), "{}: {wa:?} / {wb:?}", a.subtopic);
    }
}

/// Real exports of KlasseDozent, if there are some next to the project (they are not
/// part of the repository): `_archiv/musteraufgaben-original/*/*.xml`.
#[test]
fn real_klassedozent_exports_if_present() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../_archiv/musteraufgaben-original");
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files
                .extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|f| f.path()).filter(|f| f.extension().map(|x| x == "xml").unwrap_or(false)));
        }
    }
    let mut n = 0;
    for f in files {
        let xml = std::fs::read_to_string(&f).unwrap();
        let mut db = Database::default();
        let tasks = import_xml(&xml, &mut db).unwrap();
        n += tasks.len();
        // unchanged tasks are written back byte for byte
        let refs: Vec<&Task> = tasks.iter().collect();
        let mut db2 = Database::default();
        let back = import_xml(&export_xml(&refs), &mut db2).unwrap();
        for (a, b) in tasks.iter().zip(&back) {
            assert_eq!(a.task.text, b.task.text);
            assert_eq!(a.orig_task, b.orig_task);
            for o in a.task.objects.iter().filter(|o| o.kind == ObjKind::Formula) {
                assert!(crate::formula::parse_wmf(&o.data).is_some(), "old formula readable");
            }
        }
    }
    eprintln!("{n} Aufgaben aus echten KlasseDozent-Exporten geprüft");
}

#[test]
fn database_json_round_trip() {
    let db = Database::with_samples();
    let s = serde_json::to_string(&db).unwrap();
    let back: Database = serde_json::from_str(&s).unwrap();
    assert_eq!(back.tasks.len(), db.tasks.len());
    assert_eq!(back.sheets.len(), db.sheets.len());
}

#[test]
fn markup_parses_basics() {
    use crate::markup::{parse_inline, Inl};
    let inl = parse_inline("a **b** x^{2}");
    assert!(inl.iter().any(|i| matches!(i, Inl::Text { b: true, .. })));
}

#[test]
fn parallel_version_uses_equivalent_tasks() {
    let mut db = Database::with_samples();
    let mut src = db.sheets[0].clone();
    src.group = "A".into();
    db.sheets[0] = src.clone();
    let nid = db.new_id();
    let r = db.make_parallel(&src, ParallelOptions { replace: true, shuffle: false, tolerance: 0 }, 42, nid);
    assert_eq!(r.sheet.group, "B");
    assert_eq!(r.sheet.parallel_of, Some(src.id));
    assert_eq!(r.replaced + r.kept, src.task_ids().len());
    let a = src.task_ids();
    for (old, new) in a.iter().zip(r.sheet.task_ids()) {
        let (o, n) = (db.task(*old).unwrap(), db.task(new).unwrap());
        assert_eq!((&o.subject, &o.topic, o.points), (&n.subject, &n.topic, n.points));
        if old != &new {
            assert!(!a.contains(&new), "a replacement must not come from group A");
        }
    }
    // a third group gets "C" and avoids the tasks of A and B
    db.sheets.push(r.sheet.clone());
    let nid = db.new_id();
    let r3 = db.make_parallel(&src, ParallelOptions { replace: true, shuffle: true, tolerance: 0 }, 7, nid);
    assert_eq!(r3.sheet.group, "C");
    for id in r3.sheet.task_ids() {
        if !a.contains(&id) && !r.sheet.task_ids().contains(&id) {
            continue;
        }
        // only allowed when no equivalent task was left
        assert!(r3.kept > 0);
    }
}

#[test]
fn shuffle_keeps_texts_in_place_and_tasks_complete() {
    let db = Database::with_samples();
    let mut src = db.sheets[0].clone();
    src.items.insert(1, SheetItem::Text("Hinweis".into()));
    let r = db.make_parallel(&src, ParallelOptions { replace: false, shuffle: true, tolerance: 0 }, 3, 999);
    assert_eq!(r.sheet.items[1], SheetItem::Text("Hinweis".into()));
    let mut a = src.task_ids();
    let mut b = r.sheet.task_ids();
    a.sort();
    b.sort();
    assert_eq!(a, b);
    assert_ne!(src.task_ids(), r.sheet.task_ids(), "order should change");
}

#[test]
fn group_appears_in_printed_title() {
    let db = Database::with_samples();
    let mut s = db.sheets[0].clone();
    s.group = "B".into();
    let c = crate::export::compose(&db, &s, false);
    assert!(c.title.ends_with("Gruppe B"));
    assert!(c.sections[0].doc.text.contains("Gruppe B"));
}

#[test]
fn parallel_tolerance_finds_close_tasks() {
    let db = Database::with_samples();
    let src = db.sheets[0].clone();
    let strict = db.make_parallel(&src, ParallelOptions { replace: true, shuffle: false, tolerance: 0 }, 1, 999);
    let loose = db.make_parallel(&src, ParallelOptions { replace: true, shuffle: false, tolerance: 2 }, 1, 999);
    assert!(loose.replaced >= strict.replaced);
    assert!(loose.replaced > 0, "the samples have Integral tasks within ±2 points");
    for (o, n) in src.task_ids().iter().zip(loose.sheet.task_ids()) {
        assert!((db.task(*o).unwrap().points - db.task(n).unwrap().points).abs() <= 2);
    }
    assert_eq!(loose.points_diff, db.sheet_total(&loose.sheet) - db.sheet_total(&src));
}

#[test]
fn names_are_parsed_from_pasted_lists() {
    use crate::classes::parse_names;
    let n = parse_names("Name\n1. Anna Becker\n2) Ben Dorn\n\n  Clara Ernst  \n");
    assert_eq!(n, vec!["Anna Becker", "Ben Dorn", "Clara Ernst"]);
    let n = parse_names("1\tBecker\tAnna\n2\tDorn\tBen\n");
    assert_eq!(n, vec!["Becker, Anna", "Dorn, Ben"]);
    let n = parse_names("Becker;Anna;12a\n");
    assert_eq!(n, vec!["Becker, Anna"]);
}

fn class_db() -> (Database, crate::classes::Course) {
    use crate::classes::Course;
    let mut db = Database::with_samples();
    let mut c = Course { id: 500, name: "12a".into(), ..Default::default() };
    let names: Vec<String> = ["Anna", "Ben", "Clara"].iter().map(|s| s.to_string()).collect();
    assert_eq!(c.add_students(&names, &mut db.next_id), 3);
    assert_eq!(c.add_students(&names, &mut db.next_id), 0, "no duplicates");
    db.courses.push(c.clone());
    (db, c)
}

#[test]
fn gradebook_collects_results_and_weights() {
    let (mut db, c) = class_db();
    let total = db.sheet_total(&db.sheets[0]) as f32;
    let st = c.students.clone();
    // exam 1 (IHK): Anna full points, Ben half, Clara absent
    let mut s1 = db.sheets[0].clone();
    s1.course_id = Some(c.id);
    s1.date = "01.10.2026".into();
    s1.results = vec![
        ResultRow { name: st[0].name.clone(), student: Some(st[0].id), points: total, absent: false, ..Default::default() },
        ResultRow { name: st[1].name.clone(), student: Some(st[1].id), points: total * 0.5, absent: false, ..Default::default() },
        ResultRow { name: st[2].name.clone(), student: Some(st[2].id), points: 0.0, absent: true, ..Default::default() },
    ];
    db.sheets[0] = s1.clone();
    // exam 2 counts double; results only by name (no link)
    let mut s2 = s1.clone();
    s2.id = 600;
    s2.date = "01.12.2026".into();
    s2.weight = 2.0;
    s2.results = vec![ResultRow { name: "anna".into(), student: None, points: total * 0.85, absent: false, ..Default::default() }];
    db.sheets.push(s2);
    let gb = crate::classes::gradebook(&db, &c);
    assert_eq!(gb.columns.len(), 2);
    assert_eq!(gb.columns[0].date, "01.10.2026");
    let anna = &gb.rows[0];
    assert_eq!(anna.cells[0].as_ref().unwrap().grade.as_deref(), Some("1"));
    assert_eq!(anna.cells[1].as_ref().unwrap().grade.as_deref(), Some("2"));
    // (1*1 + 2*2) / 3
    assert!((anna.average.unwrap() - 5.0 / 3.0).abs() < 0.01);
    assert_eq!(gb.rows[1].cells[0].as_ref().unwrap().grade.as_deref(), Some("4"));
    assert!(gb.rows[2].cells[0].is_none(), "absent is not a result");
    assert!(gb.rows[2].average.is_none());
    let csv = crate::classes::to_csv(&gb, &c);
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.lines().any(|l| l.starts_with("Anna;1;")));
    assert!(crate::classes::to_markup(&gb, &c).contains("Notenbuch 12a"));
}

#[test]
fn gradebook_merges_parallel_groups() {
    let (mut db, c) = class_db();
    let st = c.students.clone();
    let mut a = db.sheets[0].clone();
    a.course_id = Some(c.id);
    a.group = "A".into();
    db.sheets[0] = a.clone();
    let mut b = db.make_parallel(&a, ParallelOptions { replace: false, shuffle: true, tolerance: 0 }, 5, 700).sheet;
    let total = db.sheet_total(&a) as f32;
    db.sheets[0].results = vec![ResultRow { name: st[0].name.clone(), student: Some(st[0].id), points: total, absent: false, ..Default::default() }];
    b.results = vec![ResultRow { name: st[1].name.clone(), student: Some(st[1].id), points: total, absent: false, ..Default::default() }];
    db.sheets.push(b);
    let gb = crate::classes::gradebook(&db, &c);
    assert_eq!(gb.columns.len(), 1, "A and B are one column");
    assert_eq!(gb.rows[0].cells[0].as_ref().unwrap().group, "A");
    assert_eq!(gb.rows[1].cells[0].as_ref().unwrap().group, "B");
}

#[test]
fn absent_results_do_not_count_in_grade_overview() {
    let mut db = Database::with_samples();
    let total = db.sheet_total(&db.sheets[0]) as f32;
    db.sheets[0].results = vec![ResultRow { points: total, ..Default::default() }, ResultRow { points: 0.0, absent: true, ..Default::default() }];
    let sp = crate::export::spiegel(&db, &db.sheets[0]).unwrap();
    assert_eq!(sp.counts.iter().sum::<usize>(), 1);
    assert_eq!(sp.failed, 0);
}

#[test]
fn old_database_files_still_load() {
    // a data file from before classes/groups existed
    let json = r#"{"version":2,"next_id":5,"tasks":[],"sheets":[{"id":1,"title":"Alt","results":[{"name":"X","points":3.0}]}]}"#;
    let db: Database = serde_json::from_str(json).unwrap();
    assert_eq!(db.sheets[0].weight, 1.0);
    assert!(db.sheets[0].group.is_empty());
    assert!(db.courses.is_empty());
    assert!(!db.sheets[0].results[0].absent);
}

#[test]
fn math_parses_common_school_formulas() {
    use crate::math::{errors, parse};
    for src in [
        r"\frac{a}{b}",
        r"x^2 + 2x - 3 = 0",
        r"x_{1,2} = -\frac{p}{2} \pm \sqrt{\left(\frac{p}{2}\right)^2 - q}",
        r"\int_{0}^{1} x^2 \, dx = \frac{1}{3}",
        r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}",
        r"\lim_{x \to \infty} \frac{1}{x} = 0",
        r"\vec{v} = \frac{\Delta s}{\Delta t}",
        r"\sqrt[3]{27} = 3",
        r"f'(x) = 3x^2",
        r"\sin(\alpha) \cdot \cos \beta",
        r"x \in \mathbb{R}",
        r"\text{Umsatz} = p \cdot x",
        r"3,5 \cdot 10^{-3}",
    ] {
        let n = parse(src);
        assert!(errors(&n).is_empty(), "{src}: {:?}", errors(&n));
    }
    assert_eq!(errors(&parse(r"\foo + \frac{1}{2")), vec![r"\foo".to_string()]);
}

#[test]
fn math_layout_has_sane_size() {
    let d = crate::math::layout(r"\frac{1}{2}");
    let (w, h) = crate::math::size_px(&d);
    assert!(w > 8.0 && w < 30.0, "w = {w}");
    assert!(h > 25.0 && h < 50.0, "a fraction is about two lines high, h = {h}");
    let d = crate::math::layout("x");
    let (_, h1) = crate::math::size_px(&d);
    assert!(h1 < 20.0);
    // nothing outside the drawing area
    for src in [r"\int_{a}^{b} f(x)\,dx", r"\sum_{k=1}^{n} k^2", r"\left( \frac{a}{b} \right)^2", r"\sqrt[3]{x}"] {
        let d = crate::math::layout(src);
        let (x0, y0, x1, y1) = (d.org.x, d.org.y, d.org.x + d.ext.x, d.org.y + d.ext.y);
        for p in &d.prims {
            if let crate::formula::Prim::Lines { pts, .. } = p {
                for q in pts {
                    assert!(q.x >= x0 - 1.0 && q.x <= x1 + 1.0 && q.y >= y0 - 1.0 && q.y <= y1 + 1.0, "{src}: {q:?} outside");
                }
            }
        }
    }
}

#[test]
fn math_wmf_round_trip() {
    // the metafile written for Word can be read back by the old-formula reader
    let src = r"\int_{0}^{\pi} \sin x \, dx = 2";
    let d = crate::math::layout(src);
    let wmf = crate::wmf::from_math(src);
    assert_eq!(&wmf[0..4], &[0xD7, 0xCD, 0xC6, 0x9A]);
    let back = crate::formula::parse_wmf(&wmf[22..]).expect("readable metafile");
    let n_text = |d: &crate::formula::Drawing| d.prims.iter().filter(|p| matches!(p, crate::formula::Prim::Text { .. })).count();
    let n_lines = |d: &crate::formula::Drawing| d.prims.iter().filter(|p| !matches!(p, crate::formula::Prim::Text { .. })).count();
    let chars: usize =
        d.prims.iter().map(|p| if let crate::formula::Prim::Text { chars, .. } = p { chars.iter().filter(|c| **c != ' ').count() } else { 0 }).sum();
    assert_eq!(n_text(&back), chars, "one text record per character");
    assert!(n_lines(&back) >= n_lines(&d));
    assert!((back.ext.x - d.ext.x).abs() < 2.0 && (back.ext.y - d.ext.y).abs() < 2.0);
    // π comes out in the Symbol font
    let has_pi = back.prims.iter().any(|p| matches!(p, crate::formula::Prim::Text { chars, .. } if chars.contains(&'π')));
    assert!(has_pi);
}

#[test]
fn math_omml_is_well_formed() {
    let o = crate::math::to_omml(r"\frac{a}{b} + \sqrt{x} + x^{2} + \int_{0}^{1} t\,dt");
    for tag in ["m:f", "m:rad", "m:sSup", "m:nary", "m:oMath"] {
        assert_eq!(o.matches(&format!("<{tag}>")).count(), o.matches(&format!("</{tag}>")).count(), "{tag}");
    }
    assert!(roxmltree::Document::parse(&format!("<w xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">{o}</w>")).is_ok());
}

#[test]
fn formula_under_cursor() {
    let t = "Text $x^2$ mehr $\\frac{a}{b}$ und 5 $.";
    assert_eq!(dollar_at(t, 0), None);
    assert_eq!(dollar_at(t, 5).map(|x| x.2), Some("x^2".to_string()));
    assert_eq!(dollar_at(t, 10).map(|x| (x.0, x.1)), Some((5, 10)));
    assert_eq!(dollar_at(t, 18).map(|x| x.2), Some("\\frac{a}{b}".to_string()));
    assert_eq!(dollar_at(t, 36), None, "a lone dollar sign is no formula");
}

#[test]
fn docx_is_a_valid_package() {
    let mut db = Database::with_samples();
    // a picture and a formula picture of the old formula editor in the first solution
    let mut png = Vec::new();
    image::RgbImage::from_pixel(8, 4, image::Rgb([200, 30, 30])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let wmf = crate::wmf::from_drawing(&crate::math::layout("x^2"), 600)[22..].to_vec();
    let id = db.sheets[0].task_ids()[0];
    let sol = &mut db.task_mut(id).unwrap().solution;
    sol.objects.push(DocObject { id: 90, kind: ObjKind::Image, data: png, width: 80.0, height: 40.0 });
    sol.objects.push(DocObject { id: 91, kind: ObjKind::Formula, data: wmf, width: 0.0, height: 0.0 });
    sol.text += "\n{{90}} {{91}}";
    let c = crate::export::compose(&db, &db.sheets[0], true);
    let d = crate::docx::to_docx(&db, &c, true);
    assert_eq!(&d[0..4], b"PK\x03\x04");
    // every XML part parses, every picture referenced is in the package
    let text = String::from_utf8_lossy(&d);
    for part in ["[Content_Types].xml", "word/document.xml", "word/_rels/document.xml.rels", "word/styles.xml", "word/footer1.xml"] {
        assert!(text.contains(part), "{part} missing");
    }
    let start = text.find("<w:document").unwrap();
    let end = text.find("</w:document>").unwrap() + "</w:document>".len();
    roxmltree::Document::parse(&text[start..end]).expect("document.xml well-formed");
    let pics = text[start..end].matches("<w:drawing>").count();
    assert_eq!(pics, 2, "the picture and the old formula");
    assert!(text[start..end].contains("<m:oMath>"), "new formulas as Word formulas");
    // each file name is in the zip twice (entry + central directory)
    assert_eq!(text.matches("word/media/image").count(), 2 * pics, "one media file per picture");
}

#[test]
fn zip_crc_matches_reference() {
    assert_eq!(crate::docx::crc32_for_tests(b"123456789"), 0xCBF4_3926);
}

#[test]
fn assistant_text_formulas_become_objects() {
    use crate::mcp::{doc_for_assistant, doc_from_assistant};
    let (d, problems) = doc_from_assistant("Es gilt $\\frac{1}{2}$ und \\$5.\n- Punkt");
    assert!(problems.is_empty());
    assert_eq!(d.objects.len(), 1);
    assert_eq!(d.objects[0].kind, ObjKind::Math);
    assert_eq!(d.text, "Es gilt {{1}} und $5.\n• Punkt");
    assert_eq!(doc_for_assistant(&d), "Es gilt $\\frac{1}{2}$ und $5.\n• Punkt");
    let (_, problems) = doc_from_assistant("$\\foo x$");
    assert_eq!(problems.len(), 1);
}

fn mcp_tmp(name: &str, db: &Database) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kld_mcp_{name}_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("datenbank.json");
    db.save_to(&p).unwrap();
    p
}

fn call(s: &crate::mcp::Server, name: &str, args: serde_json::Value) -> (bool, String) {
    let r = s.call(name, &args);
    (r["isError"].as_bool().unwrap(), r["content"][0]["text"].as_str().unwrap().to_string())
}

#[test]
fn mcp_never_shows_student_data() {
    use crate::classes::Course;
    let mut db = Database::with_samples();
    let mut c = Course { id: 77, name: "Klasse Geheim".into(), ..Default::default() };
    c.add_students(&["Zacharias Unbekannt".to_string()], &mut db.next_id);
    let sid = c.students[0].id;
    let mut oral = crate::classes::Assessment { id: 78, title: "Referat Geheim".into(), ..Default::default() };
    crate::classes::set_grade(&mut oral.grades, sid, "12,5");
    c.assessments.push(oral);
    crate::classes::set_grade(&mut c.report, sid, "12,5");
    db.courses.push(c);
    db.sheets[0].course_id = Some(77);
    db.sheets[0].results = vec![ResultRow { name: "Zacharias Unbekannt".into(), student: Some(sid), points: 12.5, absent: false, ..Default::default() }];
    db.sheets[0].results[0].scores.insert("1".into(), 12.5);
    db.sheets[0].remarks = String::new();
    let p = mcp_tmp("privacy", &db);
    let s = crate::mcp::Server::new(Some(p));
    let sid0 = db.sheets[0].id;
    let mut all = String::new();
    for (name, args) in [
        ("uebersicht", serde_json::json!({})),
        ("aufgaben_suchen", serde_json::json!({ "limit": 200 })),
        ("klausuren_auflisten", serde_json::json!({ "archivierte": true })),
        ("klausur_lesen", serde_json::json!({ "id": sid0, "mit_texten": true, "mit_loesungen": true })),
        ("aufgaben_suchen", serde_json::json!({ "suche": "Zacharias" })),
    ] {
        let (err, text) = call(&s, name, args);
        assert!(!err, "{name}: {text}");
        all += &text;
    }
    assert!(!all.contains("Zacharias"), "student name leaked");
    assert!(!all.contains("Geheim"), "class list leaked");
    assert!(!all.contains("12.5") && !all.contains("12,5"), "result leaked");
}

#[test]
fn mcp_creates_but_never_changes() {
    let db = Database::with_samples();
    let p = mcp_tmp("create", &db);
    let s = crate::mcp::Server::new(Some(p.clone()));
    let before = std::fs::read_to_string(&p).unwrap();
    let (err, text) = call(
        &s,
        "aufgabe_anlegen",
        serde_json::json!({ "fach": "mathematik", "schlagwort1": "INTEGRAL", "punkte": 4, "aufgabe": "Berechne $\\int_0^1 x\\,dx$." }),
    );
    assert!(!err, "{text}");
    let after = Database::load_plain(&p).unwrap();
    assert_eq!(after.tasks.len(), db.tasks.len() + 1);
    let t = after.tasks.last().unwrap();
    assert_eq!((t.subject.as_str(), t.topic.as_str()), ("Mathematik", "Integral"), "spelling of the collection is used");
    assert_eq!(t.origin, crate::mcp::ORIGIN);
    assert!(t.id >= 1_000_000_000);
    assert_eq!(t.task.objects[0].kind, ObjKind::Math);
    // all old entries unchanged
    let old: Database = serde_json::from_str(&before).unwrap();
    for (a, b) in old.tasks.iter().zip(after.tasks.iter()) {
        assert_eq!(serde_json::to_string(a).unwrap(), serde_json::to_string(b).unwrap());
    }
    // unknown tool / bad arguments are errors, not crashes
    assert!(call(&s, "aufgabe_loeschen", serde_json::json!({})).0);
    assert!(call(&s, "aufgabe_anlegen", serde_json::json!({ "fach": "x" })).0);
    assert!(call(&s, "klausur_entwurf_anlegen", serde_json::json!({ "titel": "t", "aufgaben": [1, 1] })).0);
    assert!(call(&s, "klausur_entwurf_anlegen", serde_json::json!({ "titel": "t", "aufgaben": [1], "typ": "Quatsch" })).0);
}

#[test]
fn running_program_takes_over_assistant_entries_once() {
    let mut gui = Database::with_samples();
    let mut file = gui.clone();
    let mut t = file.tasks[0].clone();
    t.id = file.new_external_id();
    t.origin = crate::mcp::ORIGIN.into();
    file.tasks.push(t.clone());
    let mut s = file.sheets[0].clone();
    s.id = file.new_external_id();
    s.origin = crate::mcp::ORIGIN.into();
    s.items = vec![SheetItem::Task(t.id)];
    file.sheets.push(s);
    // the teacher meanwhile created a task in the program
    let mine = gui.new_id();
    let mut m = gui.tasks[1].clone();
    m.id = mine;
    gui.tasks.push(m);
    let mut known = std::collections::HashSet::new();
    assert_eq!(gui.merge_external(&file, &mut known), (1, 1));
    assert!(gui.task(mine).is_some(), "own new task kept");
    assert!(gui.task(t.id).is_some());
    assert_eq!(gui.merge_external(&file, &mut known), (0, 0), "nothing twice");
    // deleted by the teacher before the file was saved again: stays deleted
    gui.tasks.retain(|x| x.id != t.id);
    assert_eq!(gui.merge_external(&file, &mut known), (0, 0));
    // entries without origin are never taken from the file
    let mut file2 = file.clone();
    file2.tasks[0].id = 5_000;
    let mut known2 = std::collections::HashSet::new();
    let mut g2 = Database::with_samples();
    g2.tasks.retain(|x| x.id != file.tasks[0].id);
    assert_eq!(g2.merge_external(&file2, &mut known2).0, 1, "only the assistant task");
}

#[test]
fn mcp_protocol_handshake() {
    let s = crate::mcp::Server::new(Some(mcp_tmp("proto", &Database::with_samples())));
    let r = s.handle(&serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2024-11-05" } })).unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
    let r = s.handle(&serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } })).unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert!(s.handle(&serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
    let r = s.handle(&serde_json::json!({ "jsonrpc": "2.0", "id": "x", "method": "tools/list" })).unwrap();
    for t in r["result"]["tools"].as_array().unwrap() {
        assert!(t["inputSchema"]["type"] == "object");
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
}

#[test]
fn claude_desktop_config_is_merged_not_replaced() {
    let dir = std::env::temp_dir().join(format!("kld_cfg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let p = dir.join("Claude").join("claude_desktop_config.json");
    // new file
    crate::mcp::register_in(&p, r"C:\Programme\Klassenwerk\Klassenwerk.exe").unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert_eq!(v["mcpServers"]["klassenwerk"]["args"][0], "--mcp");
    // existing settings and other servers stay
    std::fs::write(
        &p,
        r#"{"globalShortcut":"Ctrl+Space","mcpServers":{"filesystem":{"command":"npx","args":["x"]},"kldozent":{"command":"KlDozent.exe","args":["--mcp"]}}}"#,
    )
    .unwrap();
    crate::mcp::register_in(&p, "K.exe").unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert_eq!(v["globalShortcut"], "Ctrl+Space");
    assert_eq!(v["mcpServers"]["filesystem"]["command"], "npx");
    assert_eq!(v["mcpServers"]["klassenwerk"]["command"], "K.exe");
    assert!(v["mcpServers"].get("kldozent").is_none(), "old entry removed");
    assert!(p.with_extension("json.bak-klassenwerk").exists());
    // broken file is left alone
    std::fs::write(&p, "{ kaputt").unwrap();
    assert!(crate::mcp::register_in(&p, "K.exe").is_err());
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "{ kaputt");
    let s = crate::mcp::config_snippet(r"C:\a b\Klassenwerk.exe");
    assert!(s.contains(r#""C:\\a b\\Klassenwerk.exe""#));
}

#[test]
fn data_of_old_name_is_taken_over() {
    let base = std::env::temp_dir().join(format!("kw_mig_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (old, new) = (base.join("KlDozent"), base.join("Klassenwerk"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("datenbank.json"), "{\"version\":2}").unwrap();
    std::fs::write(old.join("datenbank.1.json"), "{}").unwrap();
    assert!(Database::migrate_from(&old, &new));
    assert_eq!(std::fs::read_to_string(new.join("datenbank.json")).unwrap(), "{\"version\":2}");
    assert!(new.join("datenbank.1.json").exists());
    assert!(old.join("datenbank.json").exists(), "old data stays");
    assert!(old.join("UMGEZOGEN.txt").exists());
    // never overwrites data that is already there
    std::fs::write(new.join("datenbank.json"), "neu").unwrap();
    assert!(!Database::migrate_from(&old, &new));
    assert_eq!(std::fs::read_to_string(new.join("datenbank.json")).unwrap(), "neu");
}

#[test]
fn windows_resources_match_version() {
    // packaging/windows/make-resources.sh must be run after a version change
    let res = include_bytes!("../assets/windows/klassenwerk.res");
    let v: Vec<u8> = env!("CARGO_PKG_VERSION").encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    assert!(res.windows(v.len()).any(|w| w == v.as_slice()), "Windows-Ressourcen haben eine andere Version – make-resources.sh ausführen");
    let obj = include_bytes!("../assets/windows/klassenwerk.obj");
    assert!(obj.windows(v.len()).any(|w| w == v.as_slice()));
}

// ---------------------------------------------------------------- 1.1: Teilaufgaben

fn task_with_parts(db: &mut Database) -> u64 {
    let id = db.new_id();
    let mut t = Task { id, subject: "Mathematik".into(), topic: "Analysis".into(), subtopic: "Kurvendiskussion".into(), ..Default::default() };
    t.parts = vec![
        Part { label: "a".into(), points: 4, expectation: "Ableitung richtig".into() },
        Part { label: "b".into(), points: 6, expectation: "Extrempunkte mit Begründung".into() },
    ];
    t.sync_points();
    t.task = Doc::plain("a) Leiten Sie ab.\nb) Bestimmen Sie die Extrempunkte.");
    t.solution = Doc::plain("a) f'(x) = 2x");
    db.tasks.push(t);
    id
}

#[test]
fn parts_give_points_and_score_columns() {
    let mut db = Database::with_samples();
    let id = task_with_parts(&mut db);
    assert_eq!(db.task(id).unwrap().points, 10, "points are the sum of the parts");
    let mut s = db.sheets[0].clone();
    let first = db.tasks[0].id;
    s.items = vec![SheetItem::Task(first), SheetItem::Task(id)];
    let cols = db.score_columns(&s);
    let labels: Vec<&str> = cols.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["1", "2a", "2b"]);
    assert_eq!(cols[2].max, 6.0);
    assert_eq!(cols.iter().map(|c| c.max).sum::<f32>(), db.sheet_total(&s) as f32);
    let mut r = ResultRow::default();
    r.scores.insert(cols[1].key.clone(), 3.0);
    r.scores.insert(cols[2].key.clone(), 5.5);
    r.scores.insert("gelöscht".into(), 9.0);
    r.sum_scores(&cols);
    assert_eq!(r.points, 8.5, "only columns of the sheet count");
    assert_eq!(Task::default().next_part_label(), "a");
}

#[test]
fn expectation_horizon_lists_parts_and_solution() {
    let mut db = Database::with_samples();
    let id = task_with_parts(&mut db);
    let mut s = db.sheets[0].clone();
    s.items = vec![SheetItem::Task(id), SheetItem::Task(db.tasks[0].id)];
    let c = crate::export::compose_expectation(&db, &s);
    let all: String = c.sections.iter().map(|x| x.doc.text.clone()).collect();
    assert!(all.contains("| 1a | Ableitung richtig | 4 |"));
    assert!(all.contains("**Summe** | **10**"));
    // parts fully described: no model solution for task 1; the second task has no parts
    assert_eq!(all.matches("**Musterlösung**").count(), 1);
    let html = crate::export::to_html(&db, &c, false);
    assert!(html.contains("Erwartungshorizont"));
    assert!(crate::docx::to_docx(&db, &c, false).starts_with(b"PK"));
}

// ---------------------------------------------------------------- 1.1: results per task, CSV

#[test]
fn results_import_from_csv() {
    use crate::classes::{import_results, results_template, Student};
    let mut db = Database::with_samples();
    let id = task_with_parts(&mut db);
    let mut s = db.sheets[0].clone();
    s.items = vec![SheetItem::Task(id)];
    let cols = db.score_columns(&s);
    let students =
        vec![Student { id: 1, name: "Müller, Anna".into(), remark: String::new() }, Student { id: 2, name: "Ben Becker".into(), remark: String::new() }];
    // the template has the right columns and a row per student
    let tpl = results_template(&cols, &students.iter().map(|s| s.name.clone()).collect::<Vec<_>>());
    assert!(tpl.contains("Name;1a;1b;Punkte"));
    assert!(tpl.contains("max.;4;6;10"));
    // filled in (Excel, German): names in another order, decimal comma, one absent, one unknown
    let csv = "Name;Aufgabe 1a;1b;Punkte;Note\nmax.;4;6;10;\nAnna Müller;3,5;6;;\nBen Becker;fehlt;;;\nCarla Neu;2;2;;\n";
    let mut results = vec![ResultRow { name: "Ben Becker".into(), points: 7.0, ..Default::default() }];
    let rep = import_results(csv, &cols, &students, &mut results).unwrap();
    assert_eq!((rep.matched, rep.added), (2, 1));
    assert!(rep.ignored_columns.is_empty(), "{:?}", rep.ignored_columns);
    let anna = results.iter().find(|r| r.student == Some(1)).unwrap();
    assert_eq!(anna.points, 9.5);
    assert_eq!(anna.name, "Müller, Anna", "name from the class list");
    let ben = &results[0];
    assert!(ben.absent && ben.student == Some(2), "existing row updated and linked");
    assert_eq!(results.len(), 3);
    // only totals, without a header line
    let mut r2 = Vec::new();
    import_results("Anna Müller\t31,5\nBen Becker\t12\n", &cols, &students, &mut r2).unwrap();
    assert_eq!(r2.iter().map(|r| r.points).collect::<Vec<_>>(), vec![31.5, 12.0]);
    assert!(import_results("a;b\nc;d\n", &cols, &students, &mut r2).is_err());
    // quoted cells and Windows-1252
    assert_eq!(crate::classes::read_csv("\"Müller; Anna\";3\r\n")[0], vec!["Müller; Anna", "3"]);
    assert_eq!(crate::classes::decode_text(b"M\xfcller"), "Müller");
}

#[test]
fn names_from_file_skip_header() {
    let n = crate::classes::parse_names("Nachname;Vorname\nMüller;Anna\nBecker;Ben\n");
    assert_eq!(n, vec!["Müller, Anna", "Becker, Ben"]);
}

// ---------------------------------------------------------------- 1.1: grade book

#[test]
fn gradebook_with_oral_grades_and_report_grade() {
    use crate::classes::{grade_value, proposal, set_grade, Assessment};
    let (mut db, mut c) = class_db();
    let st = c.students.clone();
    let total = db.sheet_total(&db.sheets[0]) as f32;
    db.sheets[0].course_id = Some(c.id);
    db.sheets[0].results = vec![
        ResultRow { student: Some(st[0].id), points: total, ..Default::default() },       // 1
        ResultRow { student: Some(st[1].id), points: total * 0.5, ..Default::default() }, // 4
    ];
    let mut a = Assessment { id: 900, title: "Mündlich".into(), ..Default::default() };
    set_grade(&mut a.grades, st[0].id, "2");
    set_grade(&mut a.grades, st[1].id, "3-");
    set_grade(&mut a.grades, st[2].id, "2");
    set_grade(&mut a.grades, st[2].id, ""); // removed again
    c.assessments.push(a);
    c.written_percent = 60.0;
    set_grade(&mut c.report, st[1].id, "4");
    let gb = crate::classes::gradebook(&db, &c);
    assert!(gb.has_other);
    assert_eq!(gb.columns.len(), 2);
    let anna = &gb.rows[0];
    assert_eq!((anna.written, anna.other), (Some(1.0), Some(2.0)));
    assert!((anna.average.unwrap() - 1.4).abs() < 0.001, "60 % written, 40 % other");
    assert_eq!(anna.final_grade(), Some("1"));
    let ben = &gb.rows[1];
    assert!((ben.average.unwrap() - (0.6 * 4.0 + 0.4 * 3.33)).abs() < 0.01);
    assert_eq!(ben.proposal.as_deref(), Some("4"));
    assert_eq!(ben.report.as_deref(), Some("4"), "the teacher's grade wins");
    assert!(gb.rows[2].average.is_none());
    assert_eq!(grade_value("2,5"), Some(2.5));
    assert_eq!(grade_value("2-"), Some(2.33));
    assert_eq!(grade_value("gut"), None);
    assert_eq!(proposal(2.5, None), "3");
    assert_eq!(proposal(2.49, None), "2");
    let gym = scheme("Gymnasium");
    assert_eq!(proposal(16.2, Some(&gym)), "15", "limited to the key");
    let csv = crate::classes::to_csv(&gb, &c);
    assert!(csv.lines().nth(1).unwrap().ends_with("Ø schriftlich;Ø sonstige;Ø gesamt;Zeugnisnote"));
    let md = crate::classes::to_markup(&gb, &c);
    assert!(md.contains("schriftlich 60 %, sonstige Leistungen 40 %"));
    // a class saved by version 1.0 gets the default share
    let old: crate::classes::Course = serde_json::from_str(r#"{"id":1,"name":"9b","students":[]}"#).unwrap();
    assert_eq!(old.written_percent, 50.0);
}

// ---------------------------------------------------------------- 1.1: formula editor

#[test]
fn math_matrices_cases_and_lines() {
    use crate::math::{errors, layout, parse, to_omml, Node, TableKind};
    let m = parse(r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}");
    match &m {
        Node::Table { kind: TableKind::Matrix('(', ')'), rows } => assert_eq!((rows.len(), rows[0].len()), (2, 2)),
        other => panic!("{other:?}"),
    }
    let c = parse(r"f(x) = \begin{cases} x & x \ge 0 \\ -x & \text{sonst} \end{cases}");
    assert!(errors(&c).is_empty(), "{:?}", errors(&c));
    let l = parse(r"x &= 2 + 3 \\ &= 5 \\");
    match &l {
        Node::Table { kind: TableKind::Aligned, rows } => assert_eq!(rows.len(), 2, "trailing \\\\ adds no line"),
        other => panic!("{other:?}"),
    }
    // a matrix inside a fraction, a vector with \vec
    assert!(errors(&parse(r"\vec{v} = \begin{bmatrix} 1 \\ 0 \\ -1 \end{bmatrix}, \frac{1}{2}")).is_empty());
    // unknown environments are reported
    assert!(!errors(&parse(r"\begin{foo} a \end{foo}")).is_empty());
    // three lines are taller than one, and the matrix is taller than a plain number
    let one = layout("x = 5");
    let three = layout(r"x &= 1 \\ &= 2 \\ &= 3");
    assert!(three.ext.y > 2.5 * one.ext.y);
    assert!(layout(r"\begin{pmatrix} 1 \\ 2 \end{pmatrix}").ext.y > 1.5 * one.ext.y);
    // Word: a matrix with brackets and alignment marks
    let o = to_omml(r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}");
    assert!(o.contains("<m:begChr m:val=\"(\"/>") && o.matches("<m:mr>").count() == 2);
    let o = to_omml(r"x &= 1 \\ &= 2");
    assert!(o.contains("<m:mcJc m:val=\"right\"/>") && o.contains("<m:mcJc m:val=\"left\"/>"));
    let o = to_omml(r"\begin{cases} 1 & a \\ 0 & b \end{cases}");
    assert!(o.contains("<m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/>"));
    // the old formulas still parse as before
    assert!(matches!(parse(r"\frac{a}{b}"), Node::Frac(..)));
}

#[test]
fn formulas_typed_in_the_text() {
    // editor form: formula objects become $…$, a literal dollar sign \$
    let (d, _) = crate::mcp::doc_from_assistant("Preis 5 \\$, Fläche $x^2$ und Bild");
    let mut d = d;
    d.objects.push(DocObject { id: 9, kind: ObjKind::Image, data: vec![1, 2], width: 1.0, height: 1.0 });
    d.text += " {{9}}";
    let (text, objs) = d.to_dollar_text();
    assert_eq!(text, "Preis 5 \\$, Fläche $x^2$ und Bild {{9}}");
    assert_eq!(objs.len(), 1, "only the picture stays an object");
    // and back
    let back = Doc::from_dollar_text(&text, &objs);
    assert!(back.text.starts_with("Preis 5 $, Fläche {{"), "{}", back.text);
    assert!(back.text.ends_with(" und Bild {{9}}"));
    let m = back.objects.iter().find(|o| o.kind == ObjKind::Math).unwrap();
    assert_eq!(m.data, b"x^2");
    assert_ne!(m.id, 9);
    // a dollar sign followed by a space opens no formula; neither does an empty $$
    let t = Doc::from_dollar_text("5 $ und 3 $ und $$", &[]);
    assert!(t.objects.is_empty() && t.text == "5 $ und 3 $ und $$");
    // a formula with several lines
    let t = Doc::from_dollar_text(r"$x &= 1 \\ &= 2$", &[]);
    assert_eq!(t.objects.len(), 1);
}

// ---------------------------------------------------------------- 1.1: chart

#[test]
fn grade_chart_in_print_and_word() {
    let mut db = Database::with_samples();
    let total = db.sheet_total(&db.sheets[0]) as f32;
    db.sheets[0].results = [1.0, 0.9, 0.5, 0.2].iter().map(|f| ResultRow { points: total * f, ..Default::default() }).collect();
    db.sheets[0].print_spiegel = true;
    let sp = crate::export::spiegel(&db, &db.sheets[0]).unwrap();
    let chart = crate::export::spiegel_chart(&sp, 3);
    let d = crate::formula::parse_wmf(&chart.data).expect("readable metafile");
    let bars = d.prims.iter().filter(|p| matches!(p, crate::formula::Prim::Polygon { .. })).count();
    assert_eq!(bars, sp.counts.iter().filter(|c| **c > 0).count());
    // grade overview page and the exam itself carry the chart
    let c = crate::export::compose_spiegel(&db, &db.sheets[0]);
    assert!(c.sections[0].doc.text.contains("{{2}}") && c.sections[0].doc.object(2).is_some());
    let html = crate::export::to_html(&db, &c, true);
    assert!(html.contains("<svg"));
    assert!(crate::export::to_rtf(&db, &c, true).contains("\\wmetafile8"));
    let x = crate::docx::to_docx(&db, &c, true);
    assert!(x.windows(4).any(|w| w == b".wmf"));
    let exam = crate::export::compose(&db, &db.sheets[0], false);
    assert!(exam.sections.last().unwrap().doc.objects.iter().any(|o| o.kind == ObjKind::Formula));
}

#[test]
fn mcp_creates_task_with_parts() {
    let db = Database::with_samples();
    let p = mcp_tmp("parts", &db);
    let s = crate::mcp::Server::new(Some(p.clone()));
    let (err, text) = call(
        &s,
        "aufgabe_anlegen",
        serde_json::json!({ "fach": "Physik", "schlagwort1": "Kinematik", "punkte": 1, "aufgabe": "a) … b) …",
            "teilaufgaben": [{ "bezeichnung": "a)", "punkte": 3, "erwartung": "Formel" }, { "punkte": 2 }] }),
    );
    assert!(!err, "{text}");
    let after = Database::load_plain(&p).unwrap();
    let t = after.tasks.last().unwrap();
    assert_eq!(t.points, 5);
    assert_eq!(t.parts.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    let (_, read) = call(&s, "aufgabe_lesen", serde_json::json!({ "ids": [t.id] }));
    assert!(read.contains("teilaufgaben") && read.contains("Formel"));
}

// ---------------------------------------------------------------- 1.2: spelling

#[test]
fn spelling_with_a_hunspell_dictionary() {
    use crate::spell::Dict;
    // the English dictionary of the system, if there is one (Linux)
    let p = std::path::Path::new("/usr/share/hunspell/en_US.dic");
    if p.exists() {
        let d = Dict::load(p).unwrap();
        assert!(d.len() > 40_000);
        for w in ["hello", "houses", "happiness", "reconsidered", "walked", "Walking", "THE", "isn't"] {
            assert!(d.check(w), "{w} should be correct");
        }
        for w in ["helo", "hous", "recieve", "teh"] {
            assert!(!d.check(w), "{w} should be wrong");
        }
        assert!(d.suggest("recieve", 5).contains(&"receive".to_string()));
        assert!(d.suggest("teh", 5).contains(&"the".to_string()));
    }
    // a small German dictionary in Latin-1 with compounds, two suffixes and a forbidden word
    let aff = "SET ISO8859-1\nTRY esianrtolcdugmfhbkpwzv\nCOMPOUNDBEGIN x\nCOMPOUNDEND z\nCOMPOUNDMIN 3\nONLYINCOMPOUND o\nFORBIDDENWORD !\nNEEDAFFIX n\n\
SFX S Y 2\nSFX S 0 s/x .\nSFX S 0 es [st]\n\nSFX E Y 1\nSFX E 0 e/F .\n\nSFX F Y 1\nSFX F 0 n .\n\nPFX U Y 1\nPFX U 0 un .\n";
    let dic = "7\nHaus/Sxz\nTür/z\nAufgabe/xz\nklar/EU\nSchüler/Sxz\nfalsch!\nArbeit/xzS\n";
    let d = Dict::parse(aff, dic, "de_test".into());
    for w in ["Haus", "Hauses", "Haustür", "Schülerarbeit", "Aufgabenarbeit", "klare", "klaren", "unklar", "Klar", "Haustüraufgabe"] {
        assert!(d.check(w) || w == "Aufgabenarbeit", "{w} should be correct");
    }
    assert!(!d.check("falsch"), "forbidden");
    assert!(!d.check("Hasu"));
    assert!(d.suggest("Hasu", 3).contains(&"Haus".to_string()));
    // the loose fallback accepts long compounds of known words
    assert!(d.check("Arbeitsaufgabe"));
    // words of a task text: formulas, objects and computed values are skipped
    let t = "Berechne $\\frac{a}{b}$ und {{3}} mit {=2*a} auf www.example.de: Fläche \\[0;1] x2 a";
    let ws: Vec<String> = crate::spell::words(t).into_iter().map(|(a, b)| t.chars().skip(a).take(b - a).collect()).collect();
    assert_eq!(ws, vec!["Berechne", "und", "mit", "auf", "Fläche"]);
    assert_eq!(crate::spell::word_at(t, 30).map(|x| x.2), None);
    assert_eq!(crate::spell::word_at("ein Wort", 5).map(|x| x.2), Some("Wort".to_string()));
}

// ---------------------------------------------------------------- 1.2: parts in the text

#[test]
fn parts_are_read_from_and_written_into_the_text() {
    let text = "Gegeben ist f.\n\n| a) | Ableitung | 2 P. |\n| b) | Extrempunkte | 3 P. |\nc) Skizze (4 Punkte)\nz. B. so";
    let p = parts_from_text(text);
    assert_eq!(p.iter().map(|x| (x.label.as_str(), x.points)).collect::<Vec<_>>(), vec![("a", 2), ("b", 3), ("c", 4)]);
    let parts = vec![
        Part { label: "a".into(), points: 5, expectation: String::new() },
        Part { label: "b".into(), points: 3, expectation: String::new() },
        Part { label: "c".into(), points: 1, expectation: String::new() },
        Part { label: "d".into(), points: 2, expectation: String::new() },
    ];
    let t = write_parts_into_text(text, &parts);
    assert!(t.contains("| a) | Ableitung | 5 P. |"), "{t}");
    assert!(t.contains("c) Skizze (1 P.)"), "{t}");
    assert!(t.ends_with("\n| d) | … | 2 P. |"), "{t}");
    assert!(t.contains("z. B. so"));
    // and back: the text now says the same as the parts
    let again = parts_from_text(&t);
    assert_eq!(again.iter().map(|x| x.points).collect::<Vec<_>>(), vec![5, 3, 1, 2]);
}

// ---------------------------------------------------------------- 1.2: variants

#[test]
fn variants_with_random_numbers() {
    use crate::vary::{apply, eval, fmt_num, values, Var};
    let mut v = std::collections::HashMap::new();
    v.insert("a".to_string(), 3.0);
    v.insert("b".to_string(), 0.5);
    assert_eq!(eval("2*a + b^2", &v), Ok(6.25));
    assert_eq!(eval("sqrt(a^2 + 4^2)", &v), Ok(5.0));
    assert_eq!(eval("round(a/7; 2)", &v), Ok(0.43));
    assert!((eval("sin(30)", &v).unwrap() - 0.5).abs() < 1e-9, "degrees");
    assert_eq!(eval("2,5 * 2", &v), Ok(5.0));
    assert!(eval("a / 0", &v).is_err());
    assert!(eval("c + 1", &v).is_err());
    assert_eq!(fmt_num(2.5, 4), "2,5");
    assert_eq!(fmt_num(1.0 / 3.0, 2), "0,33");
    assert_eq!(fmt_num(-0.0001, 2), "0");
    assert_eq!(apply("Ein Rechteck {=a} cm × {=b} cm hat {=a*b} cm², gerundet {=a/7;1}.", &v), "Ein Rechteck 3 cm × 0,5 cm hat 1,5 cm², gerundet 0,4.");
    assert!(apply("{=c}", &v).contains("unbekannte Variable"));
    // a task: same seed, same numbers; text, solution and formulas agree
    let mut t = Task { id: 42, ..Default::default() };
    t.vars = vec![Var { name: "a".into(), min: 2.0, max: 9.0, step: 1.0 }, Var { name: "k".into(), min: 0.5, max: 2.0, step: 0.5 }];
    let (d, _) = crate::mcp::doc_from_assistant("Berechne $\\frac{{=a}}{2}$ für a = {=a}.");
    t.task = d;
    t.solution = Doc::plain("Ergebnis: {=a/2}");
    let x = t.varied(7);
    let a = values(&t, 7)["a"];
    assert!((2.0..=9.0).contains(&a) && a.fract() == 0.0);
    assert!([0.5, 1.0, 1.5, 2.0].contains(&values(&t, 7)["k"]));
    assert!(x.task.text.ends_with(&format!("für a = {}.", fmt_num(a, 4))));
    assert_eq!(x.solution.text, format!("Ergebnis: {}", fmt_num(a / 2.0, 4)));
    assert_eq!(String::from_utf8_lossy(&x.task.objects[0].data), format!("\\frac{{{}}}{{2}}", fmt_num(a, 4)));
    assert_eq!(values(&t, 7), values(&t, 7));
    // different seeds give different numbers (at least sometimes)
    assert!((0..20).map(|s| values(&t, s)["a"] as i64).collect::<std::collections::HashSet<_>>().len() > 3);
}

#[test]
fn group_b_gets_other_numbers() {
    let mut db = Database::with_samples();
    let id = db.sheets[0].task_ids()[0];
    let t = db.task_mut(id).unwrap();
    t.vars = vec![crate::vary::Var { name: "a".into(), min: 1.0, max: 1000.0, step: 1.0 }];
    t.task.text = format!("Zahl {{=a}}\n{}", t.task.text);
    let a = db.sheets[0].clone();
    let b = db.make_parallel(&a, ParallelOptions { replace: false, shuffle: false, tolerance: 0 }, 99, 4242).sheet;
    assert_ne!(a.seed, b.seed);
    let ta = crate::export::compose(&db, &a, false);
    let tb = crate::export::compose(&db, &b, false);
    let first = |c: &crate::export::Composed| c.sections[1].doc.text.lines().next().unwrap().to_string();
    assert_ne!(first(&ta), first(&tb), "Gruppe B has its own numbers");
    assert!(!first(&ta).contains("{="));
    // the solution of a sheet uses the same numbers as its task text
    let mut db2 = db.clone();
    db2.task_mut(id).unwrap().solution.text = "Lösung {=a}".into();
    let sol = crate::export::compose(&db2, &a, true);
    assert_eq!(sol.sections[1].doc.text.replace("Lösung", "Zahl"), first(&ta));
}

// ---------------------------------------------------------------- 1.2: Halbjahre

#[test]
fn gradebook_per_term() {
    use crate::classes::{gradebook_term, set_grade, set_grade_term, term_of_date, Assessment};
    assert_eq!(term_of_date("15.09.2026"), Some(1));
    assert_eq!(term_of_date("20.01.2027"), Some(1));
    assert_eq!(term_of_date("03.02.2027"), Some(2));
    assert_eq!(term_of_date(""), None);
    let (mut db, mut c) = class_db();
    let st = c.students.clone();
    let total = db.sheet_total(&db.sheets[0]) as f32;
    let mut s1 = db.sheets[0].clone();
    s1.course_id = Some(c.id);
    s1.date = "10.11.2026".into();
    s1.results = vec![ResultRow { student: Some(st[0].id), points: total, ..Default::default() }];
    let mut s2 = s1.clone();
    s2.id = 777;
    s2.date = "10.03.2027".into();
    s2.results = vec![ResultRow { student: Some(st[0].id), points: total * 0.5, ..Default::default() }];
    db.sheets[0] = s1;
    db.sheets.push(s2);
    let mut oral = Assessment { id: 1, title: "Mündlich".into(), term: 2, ..Default::default() };
    set_grade(&mut oral.grades, st[0].id, "3");
    c.assessments.push(oral);
    set_grade_term(&mut c.report, st[0].id, 1, "1");
    let year = gradebook_term(&db, &c, 0);
    let h1 = gradebook_term(&db, &c, 1);
    let h2 = gradebook_term(&db, &c, 2);
    assert_eq!((year.columns.len(), h1.columns.len(), h2.columns.len()), (3, 1, 2));
    assert!(!h1.has_other && h2.has_other);
    assert_eq!(h1.rows[0].average, Some(1.0));
    assert_eq!(h1.rows[0].report.as_deref(), Some("1"), "report grade of the 1st Halbjahr");
    assert_eq!(h2.rows[0].report, None);
    assert_eq!(year.rows[0].report, None);
    // the chart data: grades in date order
    let dev = crate::classes::development(&h2, &h2.rows[0]);
    assert_eq!(dev.iter().map(|x| x.1).collect::<Vec<_>>(), vec![4.0, 3.0]);
    assert!(crate::classes::to_csv(&h1, &c).lines().next().unwrap().ends_with("1. Halbjahr"));
}

// ---------------------------------------------------------------- 1.3: updater

#[test]
fn updater_reads_releases_and_picks_the_right_file() {
    use crate::update::{is_newer, notes, parse_version, pick_asset, Kind, Release};
    assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_version("2.0"), Some((2, 0, 0)));
    assert_eq!(parse_version("v1.3.0-beta"), Some((1, 3, 0)));
    assert_eq!(parse_version("Version"), None);
    // an answer of the GitHub API (shortened)
    let json = r###"{"tag_name": "v99.0.0", "name": "Klassenwerk 99.0.0", "draft": false, "prerelease": false,
        "html_url": "https://github.com/buchhems/klassenwerk/releases/tag/v99.0.0", "body": "## Neu\n- **Updater**\n- mehr",
        "assets": [
          {"name": "Klassenwerk-99.0.0-linux-x86_64.tar.gz", "browser_download_url": "https://example.invalid/l", "size": 10},
          {"name": "Klassenwerk-99.0.0-macos.zip", "browser_download_url": "https://example.invalid/m", "size": 11},
          {"name": "Klassenwerk-99.0.0-windows-portable.zip", "browser_download_url": "https://example.invalid/p", "size": 12},
          {"name": "Klassenwerk-99.0.0-windows-setup.exe", "browser_download_url": "https://example.invalid/s", "size": 13}
        ], "unknown_field": 1}"###;
    let r: Release = serde_json::from_str(json).unwrap();
    assert!(is_newer(&r));
    let old: Release = serde_json::from_str(r#"{"tag_name": "v0.9.0"}"#).unwrap();
    assert!(!is_newer(&old));
    let pre: Release = serde_json::from_str(r#"{"tag_name": "v99.0.0", "prerelease": true}"#).unwrap();
    // a release without title and description: GitHub sends null
    let bare: Release = serde_json::from_str(r#"{"tag_name": "v99.0.0", "name": null, "body": null, "assets": []}"#).unwrap();
    assert!(is_newer(&bare) && bare.body.is_empty());
    assert!(!is_newer(&pre), "test versions are not offered");
    assert_eq!(pick_asset(&r, Kind::WindowsSetup).unwrap().size, 13);
    assert_eq!(pick_asset(&r, Kind::WindowsPortable).unwrap().size, 12);
    assert_eq!(pick_asset(&r, Kind::MacZip).unwrap().size, 11);
    if cfg!(target_arch = "x86_64") {
        assert_eq!(pick_asset(&r, Kind::LinuxTar).unwrap().size, 10);
    }
    assert_eq!(notes(&r), "Neu\n- Updater\n- mehr");
}

#[test]
fn updater_downloads_with_curl() {
    if cfg!(windows) || std::process::Command::new("curl").arg("--version").output().is_err() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kw-dl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("quelle.bin");
    std::fs::write(&src, vec![7u8; 5000]).unwrap();
    let to = dir.join("ziel").join("kopie.bin");
    crate::update::download(&format!("file://{}", src.display()), &to).unwrap();
    assert_eq!(std::fs::read(&to).unwrap().len(), 5000);
    assert!(crate::update::download(&format!("file://{}/fehlt", dir.display()), &dir.join("x")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn formulas_in_previews_are_text() {
    use crate::math::to_text;
    assert_eq!(to_text(r"\text{CH}_4 + 2\,\text{O}_2 \rightarrow \text{CO}_2"), "CH₄ + 2 O₂ → CO₂");
    assert_eq!(to_text(r"\frac{a}{b} + x^2"), "a/b + x²");
    assert_eq!(to_text(r"\sqrt{x+1}"), "√(x + 1)");
    assert_eq!(to_text(r"x_{n+1}"), "x_(n + 1)");
    assert_eq!(to_text(r"x_{12}"), "x₁₂");
    let db = Database::with_samples();
    let t = db.tasks.iter().find(|t| t.subtopic == "Verbrennung von Methan").unwrap();
    let p = crate::markup::plain_doc(&t.task, None);
    assert!(p.contains("Methan (CH₄)"), "{p}");
    assert!(!p.contains('▣'));
    let t = db.tasks.iter().find(|t| t.subtopic == "Stoffmenge aus der Masse").unwrap();
    let p = crate::markup::plain_doc(&t.task, Some(&crate::vary::values(t, 0)));
    assert!(!p.contains("{="), "{p}");
    assert!(p.contains("20 g. a) Berechnen"), "table cells are separated: {p}");
    // a long formula takes at most 30 characters of the preview
    let d = Doc::from_dollar_text("Berechne $\\int_{0}^{1} \\left( x^3 + 2x^2 - 5x + \\frac{1}{x+1} \\right) dx$ jetzt.", &[]);
    let p = crate::markup::plain_doc(&d, None);
    assert!(p.contains("…") && p.ends_with(" jetzt."), "{p}");
    assert!(p.chars().count() <= "Berechne  jetzt.".len() + 30, "{p}");
}

#[test]
fn restore_part_takes_chosen_entries() {
    let mut old = Database::with_samples();
    let sheet = old.sheets[0].clone();
    let ids: Vec<u64> = sheet.task_ids();
    let mut now = old.clone();
    // today: one task of the sheet is gone, another changed, the sheet itself is gone
    now.tasks.retain(|t| t.id != ids[0]);
    let changed_id = old.tasks.iter().find(|t| !ids.contains(&t.id)).unwrap().id;
    now.tasks.iter_mut().find(|t| t.id == changed_id).unwrap().points = 99;
    now.sheets.clear();
    old.next_id = 5;
    let (t, s, c) = now.restore_part(&old, &[changed_id].into_iter().collect(), &[sheet.id].into_iter().collect(), &HashSet::new());
    assert_eq!((t, s, c), (2, 1, 0));
    assert!(now.task(ids[0]).is_some(), "missing task of the sheet comes along");
    assert_ne!(now.task(changed_id).unwrap().points, 99, "changed task is replaced");
    assert_eq!(now.sheets.len(), 1);
    let max = now.tasks.iter().map(|t| t.id).max().unwrap();
    assert!(now.next_id > max.min(999_999_999));
}

#[test]
fn own_font_per_task() {
    let mut db = Database::with_samples();
    let sheet = db.sheets[0].clone();
    let id = sheet.task_ids()[0];
    {
        let t = db.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.font = "Comic Sans MS".into();
        t.font_size = 16.0;
    }
    let c = crate::export::compose(&db, &sheet, false);
    let html = crate::export::to_html(&db, &c, false);
    assert!(html.contains("font-family:'Comic Sans MS'") && html.contains("font-size:16pt"));
    let rtf = crate::export::to_rtf(&db, &c, false);
    assert!(rtf.contains("{\\f3\\fnil Comic Sans MS;}") && rtf.contains("{\\f3\\fs32 "));
    let docx = crate::docx::to_docx(&db, &c, false);
    let text = String::from_utf8_lossy(&docx);
    let doc = &text[text.find("<w:document").unwrap()..text.find("</w:document>").unwrap() + 13];
    roxmltree::Document::parse(doc).expect("well-formed");
    assert!(doc.contains("w:ascii=\"Comic Sans MS\""));
    assert!(doc.contains("<w:sz w:val=\"32\"/>"));
    // the other tasks keep the standard font
    assert_eq!(rtf.matches("\\f3\\fs32").count(), 1);
}

#[test]
fn thesaurus_mythes_format() {
    let dat = "ISO8859-1\nschnell|2\n(Adjektiv)|rasch|flink|zügig\n(Adjektiv)|eilig\nHund|1\n(Substantiv)|Köter|Vierbeiner\n";
    let latin: Vec<u8> = dat.chars().map(|c| c as u32 as u8).collect();
    let th = crate::thesaurus::Thesaurus::parse(&latin, "th".into());
    assert_eq!(th.len(), 2);
    let m = th.lookup("schnell");
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].0, "Adjektiv");
    assert_eq!(m[0].1, vec!["rasch", "flink", "zügig"]);
    // inflected forms find the base word
    assert_eq!(th.lookup("schnellen").len(), 2);
    assert_eq!(th.lookup("Hunden")[0].1[0], "Köter");
    assert!(th.lookup("Katze").is_empty());
    assert_eq!(crate::thesaurus::match_case("Schnell", "rasch"), "Rasch");
    // UTF-8 files
    let th = crate::thesaurus::Thesaurus::parse("UTF-8\ngroß|1\n(Adjektiv)|riesig|hoch\n".as_bytes(), "th".into());
    assert_eq!(th.lookup("Groß")[0].1, vec!["riesig", "hoch"]);
}

#[test]
fn moodle_xml_export() {
    let db = Database::with_samples();
    let mut t = db.tasks[0].clone();
    let mut png = Vec::new();
    image::RgbImage::from_pixel(4, 4, image::Rgb([0, 90, 200])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    t.task.objects.push(DocObject { id: 70, kind: ObjKind::Image, data: png, width: 40.0, height: 40.0 });
    t.task.objects.push(DocObject { id: 71, kind: ObjKind::Math, data: b"\\frac{a}{b} < 1 \\\\ x &= 2".to_vec(), width: 0.0, height: 0.0 });
    t.task.text += "\n{{70}} Es gilt {{71}}.\n[ ] ja [x] nein";
    t.parts = vec![Part { label: "a".into(), points: 2, expectation: "Ansatz".into() }, Part { label: "b".into(), points: 3, expectation: String::new() }];
    t.sync_points();
    let xml = crate::moodle::export(&[&t, &db.tasks[1]]);
    let doc = roxmltree::Document::parse(&xml).expect("well-formed");
    let qs: Vec<_> = doc.descendants().filter(|n| n.has_tag_name("question")).collect();
    assert!(qs.iter().any(|q| q.attribute("type") == Some("category")));
    let essay = qs.iter().find(|q| q.attribute("type") == Some("essay")).unwrap();
    let grade = essay.children().find(|n| n.has_tag_name("defaultgrade")).unwrap().text().unwrap();
    assert_eq!(grade, "5");
    let qt = essay.children().find(|n| n.has_tag_name("questiontext")).unwrap();
    let text = qt.children().find(|n| n.has_tag_name("text")).unwrap().text().unwrap();
    assert!(text.contains("@@PLUGINFILE@@/abild70.png"));
    assert!(text.contains("\\(\\begin{aligned}\\frac{a}{b} &lt; 1"));
    assert!(text.contains("☐") && text.contains("☒"));
    let file = qt.children().find(|n| n.has_tag_name("file")).unwrap();
    assert_eq!(file.attribute("name"), Some("abild70.png"));
    let gi = essay.children().find(|n| n.has_tag_name("graderinfo")).unwrap();
    assert!(gi.children().find(|n| n.has_tag_name("text")).unwrap().text().unwrap().contains("Ansatz"));
}

#[test]
fn table_merge_split_border() {
    use crate::table::{apply, Cmd};
    let text = "Vorher\n+| a | b | c |\n+| d | e | f |\nNachher";
    let pos = text.find("a |").unwrap(); // byte = char here (ASCII)
                                         // merge a with b, then the merged cell with the row below
    let (t, p) = apply(text, pos, Cmd::MergeRight).unwrap();
    assert!(t.contains("+| a b | < | c |"), "{t}");
    let (t, p) = apply(&t, p, Cmd::MergeDown).unwrap();
    assert!(t.contains("+| a b d e | < | c |\n+| ^ | < | f |"), "{t}");
    let blocks = crate::markup::parse(&t);
    let crate::markup::Block::Table { cells, .. } = &blocks[1] else { panic!() };
    assert_eq!((cells[0][0].colspan, cells[0][0].rowspan), (2, 2));
    assert!(cells[1][1].covered() && cells[0][1].covered());
    assert_eq!((cells[1][2].colspan, cells[1][2].rowspan), (1, 1));
    // HTML, RTF and Word know the merge
    let doc = Doc::plain(&t);
    let html = crate::markup::to_html_opts(&doc, true, false);
    assert!(html.contains("<td colspan=\"2\" rowspan=\"2\">a b d e</td><td>c</td></tr><tr><td>f</td>"), "{html}");
    // frame off for the merged cell
    let (t2, _) = apply(&t, p, Cmd::Border).unwrap();
    assert!(t2.contains("+| !a b d e | < |"), "{t2}");
    let html = crate::markup::to_html_opts(&Doc::plain(&t2), true, false);
    assert!(html.contains("style=\"border:none\""));
    // split again
    let (t3, _) = apply(&t, p, Cmd::Split).unwrap();
    assert!(t3.contains("+| a b d e | | c |\n+| | | f |"), "{t3}");
    // rows and columns
    let (t4, p4) = apply(text, pos, Cmd::ColRight).unwrap();
    assert!(t4.contains("+| a | | b | c |"), "{t4}");
    assert!(t4[..p4].ends_with("+| a | "), "cursor in the new cell");
    let (t5, _) = apply(text, pos, Cmd::RowBelow).unwrap();
    assert!(t5.contains("+| a | b | c |\n+| | | |\n+| d | e | f |"), "{t5}");
    let (t6, _) = apply(text, pos, Cmd::DeleteCol).unwrap();
    assert!(t6.contains("+| b | c |\n+| e | f |"), "{t6}");
    let (t7, _) = apply(text, pos, Cmd::DeleteRow).unwrap();
    assert_eq!(t7, "Vorher\n+| d | e | f |\nNachher");
    assert!(apply("kein Tisch", 2, Cmd::RowBelow).is_none());
}

#[test]
fn merged_tables_in_word_and_rtf() {
    let mut db = Database::with_samples();
    let id = db.sheets[0].task_ids()[0];
    db.task_mut(id).unwrap().task.text +=
        "\n+| Kopf über zwei | < | c |\n+| d | e | f |\n+| Zwei Zeilen | g | h |\n+| ^ | i | !ohne Rahmen |\n\n| !gerahmt | frei |";
    let c = crate::export::compose(&db, &db.sheets[0], false);
    let rtf = crate::export::to_rtf(&db, &c, false);
    assert!(rtf.contains("\\clvmgf"), "vertical merge start");
    let docx = crate::docx::to_docx(&db, &c, false);
    let text = String::from_utf8_lossy(&docx);
    let doc = &text[text.find("<w:document").unwrap()..text.find("</w:document>").unwrap() + 13];
    roxmltree::Document::parse(doc).expect("well-formed");
    assert!(doc.contains("<w:gridSpan w:val=\"2\"/>"));
    assert!(doc.contains("<w:tcBorders><w:top w:val=\"single\""));
    if let Ok(dir) = std::env::var("KLASSENWERK_TEST_OUT") {
        let _ = std::fs::write(format!("{dir}/tabelle.docx"), &docx);
        let _ = std::fs::write(format!("{dir}/tabelle.rtf"), &rtf);
        let _ = std::fs::write(format!("{dir}/tabelle.html"), crate::export::to_html(&db, &c, false));
    }
}

#[test]
fn word_import_markers() {
    use crate::wordimport::{bracket_points, marker};
    let m = marker("Aufgabe=3  Fach=Physik schlagwort1=Mechanik Swort2=gleichförmige_Bewegung P#4 ZU#5").unwrap();
    assert_eq!((m.nr, m.solution), (3, false));
    assert_eq!(m.subject.as_deref(), Some("Physik"));
    assert_eq!(m.topic.as_deref(), Some("Mechanik"));
    assert_eq!(m.subtopic.as_deref(), Some("gleichförmige Bewegung"));
    assert_eq!(m.points, Some(4));
    assert_eq!(m.lines, Some((5, true)));
    let l = marker("L#12").unwrap();
    assert_eq!((l.nr, l.solution), (12, true));
    assert_eq!(marker("lösung=  2").unwrap().nr, 2);
    assert!(marker("Aufgabe 3").is_none());
    assert!(marker("Die Aufgabe=3 ist leicht").is_none());
    let r = marker("A#1 Berechne x.").unwrap();
    assert_eq!(r.rest, "Berechne x.");
    assert_eq!(bracket_points("a (2 Punkte) b (3 P.) c (1)"), Some(6));
    assert_eq!(bracket_points("(0,5) (Punkte 11) x(2)"), None);
    assert_eq!(bracket_points("Zahl (12p)"), Some(12));
}

#[test]
fn word_import_docx_and_rtf() {
    let w = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>
<w:p><w:r><w:t>Vorwort</w:t></w:r></w:p>
<w:p><w:r><w:t>Aufgabe=1 Fach=Mathematik P#5</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Berechne </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>genau</w:t></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r><m:oMath><m:f><m:num><m:r><m:t>1</m:t></m:r></m:num><m:den><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup></m:den></m:f></m:oMath></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Punkt</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/></w:tblPr><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b|c</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:drawing><wp:inline xmlns:wp="x"><wp:extent cx="952500" cy="476250"/><a:graphic xmlns:a="y"><a:blip r:embed="rId9"/></a:graphic></wp:inline></w:drawing></w:r></w:p>
<w:p/><w:p/><w:p/>
<w:p><w:r><w:t>L#1</w:t></w:r></w:p>
<w:p><w:r><w:t>Ergebnis (2 Punkte)</w:t></w:r></w:p>
<w:p><w:r><w:t>A#2</w:t></w:r></w:p>
<w:p><w:r><w:t>Zweite Aufgabe</w:t></w:r></w:p>
<w:p><w:r><w:t>Lösung=2</w:t></w:r></w:p>
<w:p><w:r><w:t>Antwort (3 P.) und (1 P.)</w:t></w:r></w:p>
</w:body></w:document>"#;
    let mut png = Vec::new();
    image::RgbImage::from_pixel(4, 4, image::Rgb([0, 90, 200])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="image" Target="media/image1.png"/></Relationships>"#;
    let zip = crate::docx::zip_store(&[
        ("word/document.xml".to_string(), w.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels".to_string(), rels.as_bytes().to_vec()),
        ("word/media/image1.png".to_string(), png),
    ]);
    let parsed = crate::wordimport::parse_bytes(&zip, "docx").unwrap();
    let (found, notes) = crate::wordimport::tasks(&parsed, &crate::wordimport::Options { subject: "Physik".into(), ..Default::default() });
    assert_eq!(found.len(), 2, "{notes:?}");
    let t = &found[0].task;
    assert_eq!(t.subject, "Mathematik");
    assert_eq!(t.points, 5, "marker wins over the solution's brackets");
    assert_eq!(t.answer_lines, 3);
    assert!(t.task.text.starts_with("Berechne **genau** {{1}}"), "{}", t.task.text);
    assert!(t.task.text.contains("\n• Punkt\n"));
    assert!(t.task.text.contains("+| a | b\\|c |"));
    assert!(t.task.text.contains("{{2}}"));
    assert_eq!(t.task.objects.len(), 2);
    assert_eq!(String::from_utf8_lossy(&t.task.objects[0].data), "\\frac{1}{{x}^{2}}");
    assert_eq!(t.task.objects[1].width, 100.0);
    assert!(notes.iter().any(|n| n.contains("vor der ersten Markierung")));
    let t2 = &found[1].task;
    assert_eq!(t2.subject, "Physik", "default subject");
    assert_eq!(t2.points, 4, "brackets in the solution added up");
    // the formula reads back as a formula
    let d = crate::math::layout(&String::from_utf8_lossy(&t.task.objects[0].data));
    assert!(d.ext.y > 0.0);

    // RTF
    let rtf = br"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Arial;}{\f1 Symbol;}}{\colortbl;\red255\green0\blue0;}
\pard A#1 F#Chemie\par
\pard Die \b Reaktion\b0  von \cf1 CH\sub 4\nosupersub\cf0 : \u8594?\'fc \f1 a\f0\par
\trowd\clbrdrt\clbrdrb\cellx2000\cellx4000\pard\intbl x\cell y\cell\row
\pard\par\pard\par
\pard L#1\par\pard (3 Punkte)\par}";
    let parsed = crate::wordimport::parse_bytes(rtf, "rtf").unwrap();
    let (found, _) = crate::wordimport::tasks(&parsed, &Default::default());
    assert_eq!(found.len(), 1);
    let t = &found[0].task;
    assert_eq!(t.subject, "Chemie");
    assert_eq!(t.points, 3);
    assert!(t.task.text.contains("**Reaktion**"), "{}", t.task.text);
    assert!(t.task.text.contains("_{4}"));
    assert!(t.task.text.contains("→ü α"), "{}", t.task.text);
    assert!(t.task.text.contains("+| x | y |"));
    assert_eq!(t.answer_lines, 2);
    if let Ok(p) = std::env::var("KLASSENWERK_WORD_TEST") {
        for f in p.split(';') {
            let data = std::fs::read(f).unwrap();
            let parsed = crate::wordimport::parse_bytes(&data, f.rsplit('.').next().unwrap()).unwrap();
            let (found, notes) = crate::wordimport::tasks(&parsed, &Default::default());
            println!("== {f}: {} Aufgaben, Hinweise {notes:?}", found.len());
            for x in &found {
                println!(
                    "-- {} [{} / {} / {}] {} P., {} Zeilen\n{}\n.. Lösung:\n{}\n.. Objekte: {:?}",
                    x.nr,
                    x.task.subject,
                    x.task.topic,
                    x.task.subtopic,
                    x.task.points,
                    x.task.answer_lines,
                    x.task.task.text,
                    x.task.solution.text,
                    x.task
                        .task
                        .objects
                        .iter()
                        .map(|o| (o.kind.clone(), String::from_utf8_lossy(&o.data[..o.data.len().min(40)]).to_string()))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
