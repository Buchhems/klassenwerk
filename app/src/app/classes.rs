//! Classes with student lists and the grade book (Notenbuch).

use super::*;
use crate::classes::{self, Course};

/// School year for a new class: from August on the year starts now ("2026/27").
fn current_school_year() -> String {
    use chrono::Datelike;
    let d = chrono::Local::now().date_naive();
    let y = if d.month() >= 8 { d.year() } else { d.year() - 1 };
    format!("{}/{:02}", y, (y + 1) % 100)
}

impl App {
    pub(super) fn ui_classes(&mut self, ctx: &egui::Context) {
        let mut acts: Vec<Act> = Vec::new();
        let mut pick: Option<u64> = None;
        let mut new_course = false;
        egui::SidePanel::left("courses").resizable(true).default_width(270.0).width_range(220.0..=420.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Klassen & Kurse").strong().size(16.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("➕  Neu").color(ACCENT).strong()).clicked() {
                        new_course = true;
                    }
                });
            });
            ui.checkbox(&mut self.course_archived, "Auch archivierte anzeigen");
            ui.separator();
            egui::ScrollArea::vertical().id_salt("courses").auto_shrink([false, false]).show(ui, |ui| {
                let list: Vec<&Course> = self.db.courses.iter().filter(|c| self.course_archived || !c.archived).collect();
                if list.is_empty() {
                    ui.label(RichText::new("Noch keine Klasse angelegt.").weak());
                }
                for c in list {
                    let sel = self.course_sel == Some(c.id);
                    let fill = if sel { sel_fill(ui) } else { card_fill(ui) };
                    let stroke = if sel { Stroke::new(1.5_f32, ACCENT) } else { Stroke::new(1.0_f32, border(ui)) };
                    let n_exams = classes::gradebook(&self.db, c).columns.len();
                    let r = egui::Frame::new().fill(fill).stroke(stroke).corner_radius(9).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(&c.name).strong());
                        let info: Vec<String> = [c.subject.clone(), c.school_year.clone()].into_iter().filter(|s| !s.trim().is_empty()).collect();
                        let mut line = info.join(" · ");
                        if !line.is_empty() {
                            line += " · ";
                        }
                        line += &format!("{} Schüler · {} Leistungsnachweise", c.students.len(), n_exams);
                        ui.label(RichText::new(line).small().weak());
                        if c.archived {
                            ui.label(RichText::new("🗄  archiviert").small().weak());
                        }
                    });
                    if r.response.interact(egui::Sense::click()).clicked() {
                        pick = Some(c.id);
                    }
                    ui.add_space(5.0);
                }
            });
        });
        if let Some(id) = pick {
            self.course_sel = Some(id);
        }
        if new_course {
            let id = self.db.new_id();
            self.db.courses.push(Course { id, name: "Neue Klasse".into(), school_year: current_school_year(), ..Default::default() });
            self.course_sel = Some(id);
            self.changed();
        }

        let Some(cid) = self.course_sel.filter(|id| self.db.courses.iter().any(|c| c.id == *id)) else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Legen Sie links eine Klasse an und fügen Sie die Schülerliste ein.\nKlausuren, die Sie dieser Klasse zuordnen, erscheinen dann im Notenbuch.").weak())
                });
            });
            return;
        };

        let subjects: Vec<String> = self.subjects().into_iter().collect();
        let term = self.course_term;
        let gb = classes::gradebook_term(&self.db, self.db.courses.iter().find(|c| c.id == cid).unwrap(), term);
        let course_term = &mut self.course_term;
        let course_chart = &mut self.course_chart;
        // sheets that can still be assigned to this class (no class yet), newest first
        let assignable: Vec<(u64, String)> = self
            .db
            .sheets
            .iter()
            .rev()
            .filter(|s| s.course_id.is_none() && !s.archived)
            .map(|s| (s.id, format!("{} – {} ({})", s.kind, s.title_with_group(), s.date)))
            .collect();
        let mut changed = false;
        let mut assign: Option<u64> = None;
        let mut unassign: Option<Vec<u64>> = None;
        let mut weights: Vec<(Vec<u64>, f32)> = Vec::new();
        let (mut csv, mut print, mut delete, mut load_names) = (false, false, false, false);
        let mut msg: Option<String> = None;
        let names_paste = &mut self.names_paste;
        let next_id = &mut self.db.next_id;
        let course = self.db.courses.iter_mut().find(|c| c.id == cid).unwrap();

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("course").auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.heading(&course.name);
                    if !course.subject.is_empty() {
                        chip(ui, &course.subject, ACCENT);
                    }
                });
                ui.add_space(4.0);
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::Grid::new("coursemeta").num_columns(6).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label("Klasse / Kurs");
                        changed |= ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut course.name)).changed();
                        ui.label("Fach");
                        ui.horizontal(|ui| {
                            changed |= ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut course.subject)).changed();
                            egui::ComboBox::from_id_salt("csub").width(20.0).selected_text("").show_ui(ui, |ui| {
                                for t in &subjects {
                                    changed |= ui.selectable_value(&mut course.subject, t.clone(), t).changed();
                                }
                            });
                        });
                        ui.label("Schuljahr");
                        changed |= ui.add_sized([80.0, 22.0], egui::TextEdit::singleline(&mut course.school_year).hint_text("2026/27")).changed();
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(&mut course.archived, "Archiviert").changed();
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("🗑  Klasse löschen").clicked() {
                                delete = true;
                            }
                        });
                    });
                });
                ui.add_space(8.0);

                // ---- grade book
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        section_label(ui, "NOTENBUCH", GOOD);
                        for (t, l) in [(0u8, "Schuljahr"), (1, "1. Halbjahr"), (2, "2. Halbjahr")] {
                            ui.selectable_value(course_term, t, l).on_hover_text(
                                "Halbjahr: Klausuren nach ihrem Datum (August–Januar = 1., Februar–Juli = 2.), sonstige Noten nach Halbjahr bzw. Datum; die Zeugnisnote wird je Halbjahr geführt",
                            );
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add_enabled(!gb.columns.is_empty(), egui::Button::new("📄  CSV für Excel")).clicked() {
                                csv = true;
                            }
                            if ui.add_enabled(!gb.columns.is_empty(), egui::Button::new("🖶  Drucken / PDF")).clicked() {
                                print = true;
                            }
                        });
                    });
                    if gb.columns.is_empty() {
                        ui.label(
                            RichText::new("Noch keine Klausur zugeordnet. Wählen Sie unten eine aus – oder beim Bearbeiten einer Klausur unter „Klasse / Kurs“ diese Klasse. Mündliche und sonstige Noten tragen Sie mit „➕ Mündliche/sonstige Note“ ein.")
                                .weak(),
                        );
                    } else if gb.rows.is_empty() {
                        ui.label(RichText::new("Fügen Sie unten die Schülerliste ein.").weak());
                    }
                    let mut remove_assessment: Option<u64> = None;
                    if !gb.columns.is_empty() {
                        egui::ScrollArea::horizontal().id_salt("gbs").show(ui, |ui| {
                            // input fields stay visible on striped rows
                            let field_bg = ui.visuals().widgets.inactive.weak_bg_fill;
                            egui::Grid::new("gradebook").striped(true).spacing([14.0, 5.0]).show(ui, |ui| {
                                ui.label(RichText::new("Name").strong()).on_hover_text("Auf einen Namen klicken: Notenentwicklung als Diagramm");
                                for c in &gb.columns {
                                    ui.vertical(|ui| {
                                        // column titles are never broken inside a word
                                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                        if c.is_written() {
                                            ui.set_min_width(110.0);
                                            let label = RichText::new(&c.title).strong();
                                            if ui.add(egui::Link::new(label)).on_hover_text("Ergebnisse öffnen").clicked() {
                                                acts.push(Act::OpenSheet(c.sheet_ids[0], SheetView::Results));
                                            }
                                            ui.add(egui::Label::new(RichText::new(format!("{} · {}", c.kind, c.date)).small().weak()).extend());
                                            let mut w = c.weight;
                                            if ui
                                                .add(egui::DragValue::new(&mut w).range(0.0..=5.0).speed(0.05).prefix("×").max_decimals(2))
                                                .on_hover_text("Gewichtung unter den Klausuren (2 = zählt doppelt, 0 = zählt nicht)")
                                                .changed()
                                            {
                                                weights.push((c.sheet_ids.clone(), w));
                                            }
                                        } else if let Some(a) = course.assessments.iter_mut().find(|a| Some(a.id) == c.assessment) {
                                            ui.set_min_width(112.0);
                                            changed |= ui.add_sized([112.0, 20.0], egui::TextEdit::singleline(&mut a.title).hint_text("Titel")).changed();
                                            ui.horizontal(|ui| {
                                                changed |= ui.add_sized([74.0, 18.0], egui::TextEdit::singleline(&mut a.date).hint_text("Datum").font(egui::TextStyle::Small)).changed();
                                                let hj = match a.term {
                                                    1 => "1. Hj",
                                                    2 => "2. Hj",
                                                    _ => "Hj",
                                                };
                                                egui::ComboBox::from_id_salt(("hj", a.id)).width(34.0).selected_text(RichText::new(hj).small()).show_ui(ui, |ui| {
                                                    changed |= ui.selectable_value(&mut a.term, 0, "nach Datum (ohne Datum: ganzes Schuljahr)").changed();
                                                    changed |= ui.selectable_value(&mut a.term, 1, "1. Halbjahr").changed();
                                                    changed |= ui.selectable_value(&mut a.term, 2, "2. Halbjahr").changed();
                                                });
                                            });
                                            ui.horizontal(|ui| {
                                                changed |= ui
                                                    .add(egui::DragValue::new(&mut a.weight).range(0.0..=5.0).speed(0.05).prefix("×").max_decimals(2))
                                                    .on_hover_text("Gewichtung unter den sonstigen Leistungen")
                                                    .changed();
                                                if ui.small_button("✕").on_hover_text("Spalte entfernen").clicked() {
                                                    remove_assessment = Some(a.id);
                                                }
                                            });
                                        }
                                    });
                                }
                                if gb.has_other {
                                    ui.label(RichText::new("Ø schriftl.").strong()).on_hover_text("Durchschnitt der Klausuren");
                                    ui.label(RichText::new("Ø sonst.").strong()).on_hover_text("Durchschnitt der mündlichen und sonstigen Leistungen");
                                }
                                ui.label(RichText::new("Ø").strong().size(15.0));
                                ui.label(RichText::new("Zeugnis").strong())
                                    .on_hover_text("Vorschlag: der gerundete Gesamtdurchschnitt (x,5 wird aufgerundet). Eine eigene Eingabe ersetzt den Vorschlag.");
                                ui.end_row();
                                for r in &gb.rows {
                                    if ui.add(egui::Label::new(&r.name).sense(egui::Sense::click())).on_hover_text("Notenentwicklung anzeigen").clicked() {
                                        *course_chart = Some(r.student);
                                    }
                                    for (c, col) in r.cells.iter().zip(&gb.columns) {
                                        if !col.is_written() {
                                            // oral/other grades are typed in directly
                                            let a = course.assessments.iter_mut().find(|a| Some(a.id) == col.assessment);
                                            let st = course.students.iter().find(|s| s.id == r.student);
                                            if let (Some(a), Some(st)) = (a, st) {
                                                let mut g = classes::grade_in(&a.grades, st.id).to_string();
                                                let bad = !g.trim().is_empty() && classes::grade_value(&g).is_none();
                                                let te = egui::TextEdit::singleline(&mut g)
                                                    .desired_width(40.0)
                                                    .background_color(field_bg)
                                                    .text_color_opt(bad.then_some(WARN));
                                                if ui.add(te).on_hover_text("Note, z. B. 2, 2-, 11 oder 2,5").changed() {
                                                    classes::set_grade(&mut a.grades, st.id, &g);
                                                    changed = true;
                                                }
                                            }
                                            continue;
                                        }
                                        match c {
                                            Some(c) => {
                                                ui.horizontal(|ui| {
                                                    let g = ui
                                                        .add(egui::Label::new(RichText::new(c.grade.clone().unwrap_or_else(|| "–".into())).strong()).sense(egui::Sense::click()))
                                                        .on_hover_text(format!("{} von {} Punkten – klicken öffnet die Ergebnisse", export::fmt_pts(c.points), export::fmt_pts(c.total)));
                                                    if g.clicked() {
                                                        acts.push(Act::OpenSheet(c.sheet_id, SheetView::Results));
                                                    }
                                                    ui.label(RichText::new(format!("{} P.", export::fmt_pts(c.points))).small().weak());
                                                    if !c.group.trim().is_empty() {
                                                        ui.label(RichText::new(c.group.trim()).small().color(ACCENT)).on_hover_text("Gruppe");
                                                    }
                                                });
                                            }
                                            None => {
                                                ui.label(RichText::new("·").weak());
                                            }
                                        }
                                    }
                                    if gb.has_other {
                                        ui.label(r.written.map(classes::fmt2).unwrap_or_default());
                                        ui.label(r.other.map(classes::fmt2).unwrap_or_default());
                                    }
                                    ui.label(RichText::new(r.average.map(classes::fmt2).unwrap_or_default()).strong().color(ACCENT));
                                    {
                                        let mut g = classes::grade_in_term(&course.report, r.student, term).to_string();
                                        let hint = r.proposal.clone().unwrap_or_default();
                                        if ui
                                            .add(egui::TextEdit::singleline(&mut g).desired_width(40.0).background_color(field_bg).hint_text(hint))
                                            .on_hover_text("Leer = Vorschlag (grau) übernehmen; eine Eingabe ersetzt ihn")
                                            .changed()
                                        {
                                            classes::set_grade_term(&mut course.report, r.student, term, &g);
                                            changed = true;
                                        }
                                    }
                                    ui.end_row();
                                }
                                ui.label(RichText::new("Klassendurchschnitt").italics());
                                for c in &gb.columns {
                                    ui.label(RichText::new(c.average.map(classes::fmt2).unwrap_or_default()).italics());
                                }
                                if gb.has_other {
                                    ui.label("");
                                    ui.label("");
                                }
                                ui.label(RichText::new(gb.average.map(classes::fmt2).unwrap_or_default()).italics().strong());
                                ui.end_row();
                            });
                        });
                    }
                    if let Some(id) = remove_assessment {
                        course.assessments.retain(|a| a.id != id);
                        changed = true;
                    }
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("➕  Mündliche/sonstige Note").on_hover_text("Neue Spalte für mündliche Mitarbeit, Referat, Test …").clicked() {
                            let n = course.assessments.len() + 1;
                            course.assessments.push(classes::Assessment { id: *next_id, title: format!("Mündlich {n}"), term, ..Default::default() });
                            *next_id += 1;
                            changed = true;
                        }
                        if !course.assessments.is_empty() {
                            ui.label("Anteil schriftlich");
                            changed |= ui
                                .add(egui::DragValue::new(&mut course.written_percent).range(0.0..=100.0).speed(1.0).suffix(" %"))
                                .on_hover_text("So viel zählen die Klausuren in der Gesamtnote, der Rest sind die sonstigen Leistungen")
                                .changed();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        egui::ComboBox::from_id_salt("assign").width(320.0).selected_text("➕  Klausur/Übung zuordnen …").show_ui(ui, |ui| {
                            if assignable.is_empty() {
                                ui.label(RichText::new("Alle Mappen sind schon einer Klasse zugeordnet.").weak());
                            }
                            for (id, l) in &assignable {
                                if ui.selectable_label(false, l).clicked() {
                                    assign = Some(*id);
                                }
                            }
                        });
                        if !gb.columns.is_empty() {
                            ui.menu_button("✕ Zuordnung lösen …", |ui| {
                                for c in &gb.columns {
                                    if ui.button(format!("{} ({})", c.title, c.date)).clicked() {
                                        unassign = Some(c.sheet_ids.clone());
                                        ui.close_menu();
                                    }
                                }
                            });
                        }
                    });
                });
                ui.add_space(8.0);

                // ---- student list
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_label(ui, &format!("SCHÜLERINNEN & SCHÜLER  ({})", course.students.len()), ACCENT);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::multiline(names_paste)
                                .hint_text("Namen einfügen – eine Zeile pro Person,\nz. B. aus Excel oder einer Klassenliste kopiert")
                                .desired_rows(3)
                                .desired_width(320.0),
                        );
                        ui.vertical(|ui| {
                            if ui.button("➕  Hinzufügen").clicked() {
                                let names = classes::parse_names(names_paste);
                                let n = course.add_students(&names, next_id);
                                msg = Some(match (n, names.len()) {
                                    (0, 0) => "Keine Namen erkannt.".into(),
                                    (n, m) if n < m => format!("{n} hinzugefügt, {} waren schon in der Liste.", m - n),
                                    (n, _) => format!("{n} Schülerinnen/Schüler hinzugefügt."),
                                });
                                if n > 0 {
                                    names_paste.clear();
                                    changed = true;
                                }
                            }
                            if ui.button("📂  Aus Datei …").on_hover_text("Namen aus einer CSV- oder Textdatei (z. B. aus Excel oder dem Schulverwaltungsprogramm)").clicked() {
                                load_names = true;
                            }
                            if ui.button("🔤  A–Z sortieren").clicked() {
                                course.students.sort_by_key(|s| s.name.to_lowercase());
                                changed = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    let mut remove = None;
                    egui::Grid::new("students").striped(true).num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
                        for (i, s) in course.students.iter_mut().enumerate() {
                            ui.label(format!("{}", i + 1));
                            changed |= ui.add_sized([220.0, 22.0], egui::TextEdit::singleline(&mut s.name)).changed();
                            changed |= ui.add_sized([260.0, 22.0], egui::TextEdit::singleline(&mut s.remark).hint_text("🔒 interne Bemerkung")).changed();
                            if ui.small_button("✕").on_hover_text("Aus der Liste entfernen (Ergebnisse bleiben mit Namen erhalten)").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(i) = remove {
                        course.students.remove(i);
                        changed = true;
                    }
                    ui.add_space(6.0);
                    ui.label("Bemerkungen zur Klasse");
                    changed |= ui.add(egui::TextEdit::multiline(&mut course.remarks).desired_rows(2).desired_width(f32::INFINITY)).changed();
                });
            });
        });

        let course_name = self.db.courses.iter().find(|c| c.id == cid).map(|c| c.name.clone()).unwrap_or_default();
        if let Some(id) = assign {
            let fam = self.db.sheets.iter().find(|s| s.id == id).map(|s| self.db.parallel_family(s)).unwrap_or_default();
            for s in self.db.sheets.iter_mut().filter(|s| fam.contains(&s.id)) {
                s.course_id = Some(cid);
                if s.class_name.trim().is_empty() {
                    s.class_name = course_name.clone();
                }
            }
            changed = true;
        }
        if let Some(ids) = unassign {
            for s in self.db.sheets.iter_mut().filter(|s| ids.contains(&s.id)) {
                s.course_id = None;
            }
            changed = true;
        }
        for (ids, w) in weights {
            for s in self.db.sheets.iter_mut().filter(|s| ids.contains(&s.id)) {
                s.weight = w;
            }
            changed = true;
        }
        if changed {
            self.changed();
        }
        if load_names && self.shot.is_none() {
            if let Some(p) = rfd::FileDialog::new().add_filter("CSV / Text", &["csv", "txt"]).pick_file() {
                match std::fs::read(&p) {
                    Ok(bytes) => {
                        let names = classes::parse_names(&classes::decode_text(&bytes));
                        let next_id = &mut self.db.next_id;
                        if let Some(course) = self.db.courses.iter_mut().find(|c| c.id == cid) {
                            let n = course.add_students(&names, next_id);
                            msg = Some(match (n, names.len()) {
                                (0, 0) => "In der Datei wurden keine Namen erkannt.".into(),
                                (n, m) if n < m => format!("{n} hinzugefügt, {} waren schon in der Liste.", m - n),
                                (n, _) => format!("{n} Schülerinnen/Schüler hinzugefügt."),
                            });
                            changed = n > 0;
                        }
                    }
                    Err(e) => msg = Some(format!("Die Datei konnte nicht gelesen werden: {e}")),
                }
            }
        }
        if load_names && changed {
            self.changed();
        }
        if let Some(m) = msg {
            self.toast(m);
        }
        if delete {
            self.modal = Modal::DeleteCourse(cid);
        }
        if csv || print {
            let course = self.db.courses.iter().find(|c| c.id == cid).unwrap().clone();
            let gb = classes::gradebook_term(&self.db, &course, self.course_term);
            if csv {
                let data = classes::to_csv(&gb, &course);
                self.save_file(&format!("Notenbuch {}.csv", export::safe_name(&course.name)), "CSV (Excel)", "csv", data.as_bytes());
            }
            if print {
                let c = export::Composed {
                    title: format!("Notenbuch {}", course.name),
                    sections: vec![export::Section::text(&classes::to_markup(&gb, &course))],
                    page_numbers: true,
                    footer: String::new(),
                };
                let html = export::to_html(&self.db, &c, false);
                match export::open_html(&c.title, &html) {
                    Ok(_) => self.toast("Notenbuch im Browser geöffnet – dort drucken oder als PDF speichern."),
                    Err(e) => self.toast(format!("Fehler: {e}")),
                }
            }
        }
        self.ui_development(ctx, cid, &gb);
        for a in acts {
            self.apply(a);
        }
    }

    /// Window with the grades of one student over time (Notenentwicklung).
    fn ui_development(&mut self, ctx: &egui::Context, cid: u64, gb: &classes::Gradebook) {
        let Some(sid) = self.course_chart else { return };
        let Some(row) = gb.rows.iter().find(|r| r.student == sid) else {
            self.course_chart = None;
            return;
        };
        let course = self.db.courses.iter().find(|c| c.id == cid).unwrap();
        let scheme = classes::course_scheme(&self.db, course);
        let higher_better = scheme.map(|s| s.higher_is_better).unwrap_or(false);
        let (lo, hi) = scheme
            .map(|s| s.rows.iter().map(|r| GradeScheme::value(&r.label).round()).fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v))))
            .filter(|(a, b)| a < b)
            .unwrap_or((1.0, 6.0));
        let pts = classes::development(gb, row);
        let mut open = true;
        egui::Window::new(format!("Notenentwicklung – {}", row.name)).open(&mut open).collapsible(false).resizable(true).default_width(620.0).show(ctx, |ui| {
            if pts.is_empty() {
                ui.label(RichText::new("Noch keine Noten.").weak());
                return;
            }
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(420.0), 260.0), egui::Sense::hover());
            let p = ui.painter();
            let tc = ui.visuals().text_color();
            let grid = ui.visuals().widgets.noninteractive.bg_stroke.color;
            let (left, right, top, bottom) = (r.left() + 34.0, r.right() - 12.0, r.top() + 24.0, r.bottom() - 46.0);
            // better grades are always at the top
            let y_of = |v: f32| {
                let f = (v - lo) / (hi - lo);
                let f = if higher_better { 1.0 - f } else { f };
                top + f * (bottom - top)
            };
            let step = if hi - lo > 8.0 { 3.0 } else { 1.0 };
            let mut v = lo;
            while v <= hi + 0.01 {
                let y = y_of(v);
                p.hline(left..=right, y, Stroke::new(1.0_f32, grid));
                p.text(egui::pos2(left - 6.0, y), egui::Align2::RIGHT_CENTER, format!("{v:.0}"), egui::FontId::proportional(11.0), tc);
                v += step;
            }
            let n = pts.len();
            let x_of = |i: usize| if n == 1 { (left + right) / 2.0 } else { left + 50.0 + (right - left - 90.0) * i as f32 / (n - 1) as f32 };
            if let Some(avg) = row.average {
                let y = y_of(avg);
                p.add(egui::Shape::dashed_line(&[egui::pos2(left, y), egui::pos2(right, y)], Stroke::new(1.5_f32, GOOD), 6.0, 4.0));
                p.text(egui::pos2(left + 4.0, y - 3.0), egui::Align2::LEFT_BOTTOM, format!("Ø {}", classes::fmt2(avg)), egui::FontId::proportional(11.0), GOOD);
            }
            let line: Vec<egui::Pos2> = pts.iter().enumerate().map(|(i, (_, v, _))| egui::pos2(x_of(i), y_of(*v))).collect();
            p.add(egui::Shape::line(line.clone(), Stroke::new(2.0_f32, ACCENT)));
            for (i, ((title, v, written), q)) in pts.iter().zip(&line).enumerate() {
                if *written {
                    p.circle_filled(*q, 5.0, ACCENT);
                } else {
                    p.rect_filled(egui::Rect::from_center_size(*q, Vec2::splat(9.0)), 1.0, WARN);
                }
                p.text(*q + Vec2::new(0.0, -8.0), egui::Align2::CENTER_BOTTOM, crate::vary::fmt_num(*v as f64, 2), egui::FontId::proportional(11.0), tc);
                let t: String = if title.chars().count() > 14 { format!("{}…", title.chars().take(13).collect::<String>()) } else { title.clone() };
                let y = bottom + 8.0 + if i % 2 == 1 && n > 6 { 16.0 } else { 0.0 };
                p.text(egui::pos2(q.x, y), egui::Align2::CENTER_TOP, t, egui::FontId::proportional(11.0), tc);
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("●").color(ACCENT));
                ui.label(RichText::new("Klausur").small());
                ui.label(RichText::new("■").color(WARN));
                ui.label(RichText::new("mündlich / sonstige").small());
                ui.label(RichText::new("– – Gesamtdurchschnitt").small().color(GOOD));
                if let Some(g) = row.final_grade() {
                    ui.label(RichText::new(format!("Zeugnis ({}): {g}", classes::term_name(gb.term))).small().strong());
                }
            });
        });
        if !open {
            self.course_chart = None;
        }
    }
}
