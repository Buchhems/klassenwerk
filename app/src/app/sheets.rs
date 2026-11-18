//! Exams, exercises and scripts: archive, composer, preview, results.

use super::*;
use crate::markup::{self, View};

/// Points in a DragValue with a decimal comma ("27,5"); typing "27.5" works too.
fn de_num(v: f64, _: std::ops::RangeInclusive<usize>) -> String {
    export::fmt_pts(v as f32)
}

fn de_parse(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse().ok()
}

impl App {
    pub(super) fn ui_sheets(&mut self, ctx: &egui::Context) {
        let mut acts: Vec<Act> = Vec::new();
        let types: Vec<String> = self.db.settings.types.iter().map(|t| t.name.clone()).collect();
        let subjects: Vec<String> = self.subjects().into_iter().collect();
        let mut ask_delete: Option<u64> = None;
        egui::SidePanel::left("archive").resizable(true).default_width(320.0).width_range(260.0..=480.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Klausurverzeichnis").strong().size(16.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button(RichText::new("➕  Neu").color(ACCENT).strong(), |ui| {
                        for t in &types {
                            if ui.button(t).clicked() {
                                acts.push(Act::NewSheet(t.clone()));
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
            ui.add(egui::TextEdit::singleline(&mut self.arch_search).hint_text("🔍  Suchen …").desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                let st = if self.arch_subject.is_empty() { "Alle Fächer".to_string() } else { self.arch_subject.clone() };
                egui::ComboBox::from_id_salt("af").width(115.0).selected_text(st).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.arch_subject, String::new(), "Alle Fächer");
                    for s in &subjects {
                        ui.selectable_value(&mut self.arch_subject, s.clone(), s);
                    }
                });
                let tt = if self.arch_type.is_empty() { "Alle Typen".to_string() } else { self.arch_type.clone() };
                egui::ComboBox::from_id_salt("at").width(115.0).selected_text(tt).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.arch_type, String::new(), "Alle Typen");
                    for s in &types {
                        ui.selectable_value(&mut self.arch_type, s.clone(), s);
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Sortieren:").small());
                for (i, l) in ["Datum", "Titel", "Fach", "Typ", "Erstellt"].iter().enumerate() {
                    let sel = self.arch_sort == i as u8;
                    let label = if sel { format!("{l} {}", if self.arch_desc { "▼" } else { "▲" }) } else { l.to_string() };
                    if ui.selectable_label(sel, RichText::new(label).small()).clicked() {
                        if sel {
                            self.arch_desc = !self.arch_desc;
                        } else {
                            self.arch_sort = i as u8;
                        }
                    }
                }
            });
            ui.checkbox(&mut self.arch_archived, "Auch archivierte anzeigen");
            ui.separator();
            let q = self.arch_search.to_lowercase();
            let mut list: Vec<&Sheet> = self
                .db
                .sheets
                .iter()
                .filter(|s| self.arch_archived || !s.archived)
                .filter(|s| self.arch_subject.is_empty() || s.subject == self.arch_subject)
                .filter(|s| self.arch_type.is_empty() || s.kind == self.arch_type)
                .filter(|s| q.is_empty() || format!("{} {} {}", s.title, s.class_name, s.subject).to_lowercase().contains(&q))
                .collect();
            let date_key = |s: &Sheet| chrono::NaiveDate::parse_from_str(&s.date, "%d.%m.%Y").ok();
            match self.arch_sort {
                0 => list.sort_by_key(|s| date_key(s)),
                1 => list.sort_by_key(|s| s.title.to_lowercase()),
                2 => list.sort_by_key(|s| s.subject.to_lowercase()),
                3 => list.sort_by_key(|s| s.kind.clone()),
                _ => list.sort_by_key(|s| s.created),
            }
            if self.arch_desc {
                list.reverse();
            }
            egui::ScrollArea::vertical().id_salt("sheets").auto_shrink([false, false]).show(ui, |ui| {
                for s in list {
                    let sel = self.sheet_sel == Some(s.id);
                    let fill = if sel { sel_fill(ui) } else { card_fill(ui) };
                    let stroke = if sel { Stroke::new(1.5_f32, ACCENT) } else { Stroke::new(1.0_f32, border(ui)) };
                    let r = egui::Frame::new().fill(fill).stroke(stroke).corner_radius(9).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&s.title).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let col = if self.db.settings.is_exam(&s.kind) { WARN } else { Color32::from_rgb(14, 116, 144) };
                                chip(ui, &s.kind, col);
                                if !s.group.trim().is_empty() {
                                    chip(ui, &format!("Gruppe {}", s.group.trim()), ACCENT);
                                }
                                if !s.origin.is_empty() {
                                    chip(ui, "✦ Entwurf", Color32::from_rgb(190, 90, 20));
                                }
                            });
                        });
                        let n = s.task_ids().len();
                        ui.label(
                            RichText::new(format!("{} · {} · {} · {n} Aufgaben · {} P.", s.subject, s.class_name, s.date, self.db.sheet_total(s)))
                                .small()
                                .weak(),
                        );
                        if s.archived {
                            ui.label(RichText::new("🗄  archiviert").small().weak());
                        } else if s.used_marked {
                            ui.label(RichText::new("✔  durchgeführt").small().color(GOOD));
                        }
                    });
                    let resp = r.response.interact(egui::Sense::click());
                    if resp.clicked() {
                        acts.push(Act::SelectSheet(s.id));
                    }
                    if resp.double_clicked() {
                        acts.push(Act::OpenSheet(s.id, SheetView::Edit));
                    }
                    let sid = s.id;
                    let archived = s.archived;
                    resp.context_menu(|ui| {
                        if ui.button("📂  Öffnen").clicked() {
                            acts.push(Act::OpenSheet(sid, SheetView::Edit));
                            ui.close_menu();
                        }
                        if ui.button("👁  Druckvorschau").clicked() {
                            acts.push(Act::OpenSheet(sid, SheetView::Preview));
                            ui.close_menu();
                        }
                        if ui.button("🖶  Drucken / PDF").clicked() {
                            acts.push(Act::PrintSheet(sid, false));
                            ui.close_menu();
                        }
                        if ui.button("🗐  Als Grundlage für neue Mappe").clicked() {
                            acts.push(Act::DuplicateSheet(sid));
                            ui.close_menu();
                        }
                        if ui.button(if archived { "📤  Aus dem Archiv holen" } else { "🗄  Archivieren" }).clicked() {
                            acts.push(Act::ArchiveSheet(sid));
                            ui.close_menu();
                        }
                        if ui.button("🗑  Löschen").clicked() {
                            ask_delete = Some(sid);
                            ui.close_menu();
                        }
                    });
                    ui.add_space(5.0);
                }
            });
        });
        if let Some(id) = ask_delete {
            self.modal = Modal::DeleteSheet(id);
        }

        let Some(sid) = self.sheet_sel.filter(|id| self.db.sheets.iter().any(|s| s.id == *id)) else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| ui.label(RichText::new("Legen Sie links eine neue Klausur oder Übung an.").weak()));
            });
            for a in acts {
                self.apply(a);
            }
            return;
        };

        if self.sheet_view == SheetView::Edit {
            self.ui_picker(ctx, sid, &mut acts);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let (title, kind) = {
                let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
                (s.title.clone(), s.kind.clone())
            };
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(title);
                chip(ui, &kind, ACCENT);
            });
            ui.horizontal(|ui| {
                for (v, l) in [
                    (SheetView::Edit, "✏  Bearbeiten"),
                    (SheetView::Preview, "👁  Druckvorschau"),
                    (SheetView::Solution, "✅ Lösungen"),
                    (SheetView::Results, "📈 Ergebnisse & Notenspiegel"),
                ] {
                    ui.selectable_value(&mut self.sheet_view, v, l);
                }
            });
            ui.separator();
            match self.sheet_view {
                SheetView::Edit => self.ui_sheet_edit(ui, sid, &mut acts),
                SheetView::Preview => self.ui_sheet_preview(ui, sid, false, &mut acts),
                SheetView::Solution => self.ui_sheet_preview(ui, sid, true, &mut acts),
                SheetView::Results => self.ui_results(ui, sid),
            }
        });
        for a in acts {
            self.apply(a);
        }
    }

    fn ui_picker(&mut self, ctx: &egui::Context, sid: u64, acts: &mut Vec<Act>) {
        let subjects: Vec<String> = self.subjects().into_iter().collect();
        egui::SidePanel::right("picker").resizable(true).default_width(300.0).width_range(240.0..=460.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("Aus dem Aufgabenpool").strong().size(15.0));
            ui.add(egui::TextEdit::singleline(&mut self.picker_search).hint_text("🔍  Aufgaben suchen …").desired_width(f32::INFINITY));
            let st = if self.picker_subject.is_empty() { "Alle Fächer".to_string() } else { self.picker_subject.clone() };
            egui::ComboBox::from_id_salt("pf").width(ui.available_width() - 8.0).selected_text(st).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.picker_subject, String::new(), "Alle Fächer");
                for s in &subjects {
                    ui.selectable_value(&mut self.picker_subject, s.clone(), s);
                }
            });
            ui.label(RichText::new("Aufgaben verschiedener Fächer lassen sich kombinieren.").small().weak());
            ui.separator();
            let sheet = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let q = self.picker_search.to_lowercase();
            egui::ScrollArea::vertical().id_salt("pick").auto_shrink([false, false]).show(ui, |ui| {
                for t in &self.db.tasks {
                    if !self.picker_subject.is_empty() && t.subject != self.picker_subject {
                        continue;
                    }
                    if !q.is_empty() && !format!("{} {} {} {} {}", t.subject, t.topic, t.subtopic, t.tags.join(" "), t.task.text).to_lowercase().contains(&q) {
                        continue;
                    }
                    let inside = sheet.contains(t.id);
                    ui.horizontal(|ui| {
                        let b =
                            if inside { egui::Button::new(RichText::new("✔").color(GOOD)) } else { egui::Button::new(RichText::new("➕").color(ACCENT)) };
                        let tip = if inside { "Bereits enthalten" } else { "Mit Musterlösung übernehmen" };
                        if ui.add(b.min_size(Vec2::splat(26.0))).on_hover_text(tip).clicked() {
                            acts.push(Act::AddToSheet(sid, t.id));
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new(t.title()).strong());
                            let mut info = format!("{} › {} · {} P.", t.subject, t.topic, t.points);
                            if let Some(d) = t.exam_last {
                                info += &format!(" · ⚠ Klausur {}", d.format("%d.%m.%Y"));
                            }
                            ui.label(RichText::new(info).small().weak());
                        });
                    });
                    ui.add_space(3.0);
                }
            });
        });
    }

    fn ui_sheet_edit(&mut self, ui: &mut egui::Ui, sid: u64, acts: &mut Vec<Act>) {
        let types: Vec<String> = self.db.settings.types.iter().map(|t| t.name.clone()).collect();
        let schemes: Vec<String> = self.db.settings.schemes.iter().map(|t| t.name.clone()).collect();
        let templates: Vec<String> = self.db.settings.templates.iter().map(|t| t.name.clone()).collect();
        let subjects: Vec<String> = self.subjects().into_iter().collect();
        let total = self.db.sheet_total(self.db.sheets.iter().find(|s| s.id == sid).unwrap());
        let tasks_snapshot: Vec<(u64, String, String, i32, u32, Option<chrono::NaiveDateTime>)> =
            self.db.tasks.iter().map(|t| (t.id, t.title(), format!("{} › {}", t.subject, t.topic), t.points, t.minutes, t.exam_last)).collect();
        let scheme = {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            self.db.settings.scheme(&s.scheme).cloned()
        };
        let has_vars = {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            s.task_ids().iter().filter_map(|id| self.db.task(*id)).any(|t| !t.vars.is_empty())
        };
        let mut reseed = false;
        let mut changed = false;
        let (mut mark, mut fill, mut xml, mut del_sheet) = (false, false, false, false);
        let mut moodle = false;
        let mut rtf: Option<(bool, bool)> = None;
        let courses: Vec<(u64, String)> = self.db.courses.iter().filter(|c| !c.archived).map(|c| (c.id, c.name.clone())).collect();
        let mut course_pick: Option<Option<u64>> = None;
        let rnd_target = &mut self.rnd_target;
        let (par_replace, par_shuffle, par_tol) = (&mut self.par_replace, &mut self.par_shuffle, &mut self.par_tolerance);
        let family: Vec<(u64, String)> = {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let fam = self.db.parallel_family(s);
            self.db.sheets.iter().filter(|x| fam.contains(&x.id) && x.id != sid).map(|x| (x.id, x.group.clone())).collect()
        };
        let s = self.db.sheets.iter_mut().find(|s| s.id == sid).unwrap();
        egui::ScrollArea::vertical().id_salt("sheet_edit").auto_shrink([false, false]).show(ui, |ui| {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("sheetmeta").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Titel");
                    changed |= ui.add_sized([220.0, 22.0], egui::TextEdit::singleline(&mut s.title)).changed();
                    ui.label("Typ");
                    egui::ComboBox::from_id_salt("kind").width(150.0).selected_text(s.kind.clone()).show_ui(ui, |ui| {
                        for t in &types {
                            changed |= ui.selectable_value(&mut s.kind, t.clone(), t).changed();
                        }
                    });
                    ui.end_row();
                    ui.label("Fach");
                    ui.horizontal(|ui| {
                        changed |= ui.add_sized([190.0, 22.0], egui::TextEdit::singleline(&mut s.subject)).changed();
                        egui::ComboBox::from_id_salt("ssub").width(20.0).selected_text("").show_ui(ui, |ui| {
                            for t in &subjects {
                                changed |= ui.selectable_value(&mut s.subject, t.clone(), t).changed();
                            }
                        });
                    });
                    ui.label("Klasse / Kurs");
                    ui.horizontal(|ui| {
                        changed |= ui.add_sized([126.0, 22.0], egui::TextEdit::singleline(&mut s.class_name)).changed();
                        let linked = s.course_id.is_some();
                        egui::ComboBox::from_id_salt("scourse")
                            .width(20.0)
                            .selected_text(if linked { "👥" } else { "" })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(!linked, "keine Klassenliste").clicked() {
                                    course_pick = Some(None);
                                }
                                for (id, name) in &courses {
                                    if ui.selectable_label(s.course_id == Some(*id), format!("👥  {name}")).clicked() {
                                        course_pick = Some(Some(*id));
                                    }
                                }
                                if courses.is_empty() {
                                    ui.label(RichText::new("Klassenlisten legen Sie unter „Klassen & Notenbuch“ an.").weak());
                                }
                            })
                            .response
                            .on_hover_text("Klassenliste zuordnen – die Ergebnisse erscheinen dann im Notenbuch der Klasse");
                    });
                    ui.end_row();
                    ui.label("Datum");
                    changed |= ui.add_sized([110.0, 22.0], egui::TextEdit::singleline(&mut s.date).hint_text("TT.MM.JJJJ")).changed();
                    ui.label("Bearbeitungszeit");
                    ui.horizontal(|ui| {
                        changed |= ui.add(egui::DragValue::new(&mut s.minutes).range(0..=600).suffix(" Min.")).changed();
                        ui.label("Gruppe");
                        changed |= ui
                            .add_sized([36.0, 22.0], egui::TextEdit::singleline(&mut s.group).hint_text("–"))
                            .on_hover_text("Gruppe einer Parallelklausur (A, B, …); erscheint im Titel bzw. im Platzhalter {Gruppe}")
                            .changed();
                    });
                    ui.end_row();
                    ui.label("Notenschlüssel");
                    let sc = if s.scheme.is_empty() { "keiner".to_string() } else { s.scheme.clone() };
                    egui::ComboBox::from_id_salt("scheme").width(220.0).selected_text(sc).show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut s.scheme, String::new(), "keiner").changed();
                        for t in &schemes {
                            changed |= ui.selectable_value(&mut s.scheme, t.clone(), t).changed();
                        }
                    });
                    ui.label("Druckvorlage");
                    egui::ComboBox::from_id_salt("tpl").width(150.0).selected_text(s.template.clone()).show_ui(ui, |ui| {
                        for t in &templates {
                            changed |= ui.selectable_value(&mut s.template, t.clone(), t).changed();
                        }
                    });
                    ui.end_row();
                });
                ui.horizontal_wrapped(|ui| {
                    changed |= ui.checkbox(&mut s.allow_duplicates, "Doppelte Aufgaben zulassen").changed();
                    changed |= ui.checkbox(&mut s.print_lines, "Leerzeilen drucken").changed();
                    changed |= ui.checkbox(&mut s.underline_lines, "Leerzeilen unterstreichen").changed();
                    changed |= ui.checkbox(&mut s.print_spiegel, "Notenspiegel in Klausur einfügen").changed();
                });
            });
            ui.add_space(8.0);
            let mut swap: Option<(usize, usize)> = None;
            let mut remove: Option<usize> = None;
            let mut sum_min = 0;
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    section_label(ui, "AUFGABEN & ZUSATZTEXTE", ACCENT);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("➕  Zusatztext").on_hover_text("Hinweis oder Text zwischen den Aufgaben").clicked() {
                            s.items.push(SheetItem::Text("Hinweis: …".into()));
                            changed = true;
                        }
                    });
                });
                if s.items.is_empty() {
                    ui.label(RichText::new("Noch leer – rechts Aufgaben aus dem Pool hinzufügen.").weak());
                }
                let n = s.items.len();
                let mut nr = 0;
                for (i, item) in s.items.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let controls = |ui: &mut egui::Ui, swap: &mut Option<(usize, usize)>, remove: &mut Option<usize>| {
                            if ui.small_button("✕").on_hover_text("Entfernen").clicked() {
                                *remove = Some(i);
                            }
                            if i + 1 < n && ui.small_button("▼").clicked() {
                                *swap = Some((i, i + 1));
                            }
                            if i > 0 && ui.small_button("▲").clicked() {
                                *swap = Some((i, i - 1));
                            }
                        };
                        match item {
                            SheetItem::Task(tid) => {
                                nr += 1;
                                let tid = *tid;
                                let info = tasks_snapshot.iter().find(|x| x.0 == tid);
                                ui.label(RichText::new(format!("{nr}.")).strong().size(15.0));
                                ui.vertical(|ui| match info {
                                    Some((_, title, path, pts, mins, exam_last)) => {
                                        sum_min += mins;
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(title).strong());
                                            chip(ui, &format!("{pts} P."), GOOD);
                                        });
                                        let mut sub = path.clone();
                                        if let Some(d) = exam_last {
                                            sub += &format!("   ⚠ schon in Klausur am {}", d.format("%d.%m.%Y"));
                                        }
                                        ui.label(RichText::new(sub).small().weak());
                                    }
                                    None => {
                                        ui.label(RichText::new("(gelöschte Aufgabe)").weak());
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    controls(ui, &mut swap, &mut remove);
                                    if ui.small_button("✏").on_hover_text("Aufgabe bearbeiten – die Änderung gilt auch im Pool").clicked() {
                                        acts.push(Act::Edit(tid));
                                    }
                                });
                            }
                            SheetItem::Text(t) => {
                                ui.label(RichText::new("📝").size(15.0));
                                let w = (ui.available_width() - 110.0).max(100.0);
                                changed |= ui.add(egui::TextEdit::multiline(t).desired_rows(2).desired_width(w)).changed();
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| controls(ui, &mut swap, &mut remove));
                            }
                        }
                    });
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Gesamt: {total} Punkte")).strong().size(15.0));
                    if sum_min > 0 {
                        ui.label(RichText::new(format!("· ca. {sum_min} Min. Bearbeitungszeit")).weak());
                    }
                });
            });
            if let Some((a, b)) = swap {
                s.items.swap(a, b);
                changed = true;
            }
            if let Some(i) = remove {
                s.items.remove(i);
                changed = true;
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("🎲  Zufällig auffüllen bis");
                ui.add(egui::DragValue::new(rnd_target).range(1..=300).suffix(" Punkte"));
                if ui.button("Auffüllen").on_hover_text("Ergänzt Aufgaben des Fachs (oder des Filters rechts), bevorzugt solche, die noch nie in einer Klausur waren").clicked() {
                    fill = true;
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("👥  Parallelklausur (Gruppe A/B) anlegen")
                    .on_hover_text("Legt eine zweite Fassung dieser Mappe als neue Gruppe an – mit eigener Musterlösung")
                    .clicked()
                {
                    acts.push(Act::ParallelSheet(sid, ParallelOptions { replace: *par_replace, shuffle: *par_shuffle, tolerance: *par_tol }));
                }
                ui.checkbox(par_replace, "Ersatzaufgaben")
                    .on_hover_text("Jede Aufgabe wird durch eine andere mit gleichem Fach und gleichem Schlagwort 1 ersetzt (falls vorhanden); gleiche Punktzahl und gleiches Schlagwort 2 haben Vorrang");
                ui.add_enabled(*par_replace, egui::DragValue::new(par_tol).range(0..=10).prefix("± ").suffix(" P."))
                    .on_hover_text("Um so viele Punkte darf eine Ersatzaufgabe abweichen (0 = nur gleiche Punktzahl)");
                ui.checkbox(par_shuffle, "Reihenfolge mischen");
            });
            if has_vars {
                ui.horizontal_wrapped(|ui| {
                    ui.label("🎲  Aufgaben mit Zufallszahlen:");
                    if ui
                        .button("🎲  Neue Zahlen würfeln")
                        .on_hover_text("Andere Zahlen für diese Mappe; Druck, Lösung und Erwartungshorizont verwenden immer dieselben Zahlen")
                        .clicked()
                    {
                        reseed = true;
                    }
                    ui.label(RichText::new("Jede Gruppe einer Parallelklausur hat eigene Zahlen.").small().weak());
                });
            }
            if !family.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Andere Gruppen:").small().weak());
                    for (fid, g) in &family {
                        let label = if g.trim().is_empty() { "ohne Gruppe".to_string() } else { format!("Gruppe {g}") };
                        if ui.small_button(label).clicked() {
                            acts.push(Act::OpenSheet(*fid, SheetView::Edit));
                        }
                    }
                });
            }
            ui.add_space(8.0);
            if let Some(sc) = &scheme {
                if total > 0 {
                    card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        section_label(ui, &format!("NOTENSCHLÜSSEL – {}", sc.name.to_uppercase()), Color32::GRAY);
                        egui::ScrollArea::horizontal().id_salt("scs").show(ui, |ui| {
                            egui::Grid::new("scale").striped(true).show(ui, |ui| {
                                let rows = sc.rows_for(total as f32);
                                ui.label(RichText::new("Note").strong());
                                for r in &rows {
                                    ui.label(RichText::new(&r.0).strong());
                                }
                                ui.end_row();
                                ui.label(RichText::new("Punkte").strong());
                                for r in &rows {
                                    if r.1 > r.2 {
                                        ui.label(RichText::new("–").weak()).on_hover_text("Bei dieser Gesamtpunktzahl hat diese Note keinen eigenen Punktbereich.");
                                    } else {
                                        ui.label(format!("{}–{}", export::fmt_bound(r.1), export::fmt_bound(r.2)));
                                    }
                                }
                                ui.end_row();
                            });
                        });
                    });
                }
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui.add(egui::Button::new(RichText::new("🖶  Drucken / PDF").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                    acts.push(Act::PrintSheet(sid, false));
                }
                if ui.button("🖶  Lösungen drucken").clicked() {
                    acts.push(Act::PrintSheet(sid, true));
                }
                if let Some(r) = word_menu(ui, "📄  Word") {
                    rtf = Some((false, r));
                }
                if let Some(r) = word_menu(ui, "📄  Lösungen als Word") {
                    rtf = Some((true, r));
                }
                if ui.button("📄  Aufgaben als XML").clicked() {
                    xml = true;
                }
                if ui.button("🎓  Für Moodle (XML)").on_hover_text("Die Aufgaben als Moodle-Fragen (Freitext) mit Punkten, Musterlösung und Erwartungshorizont").clicked() {
                    moodle = true;
                }
                if ui.button("✔  Als durchgeführt vermerken").on_hover_text("Trägt Datum und Mappe bei allen Aufgaben ein").clicked() {
                    mark = true;
                }
                if ui.button("🗄  Archivieren").clicked() {
                    acts.push(Act::ArchiveSheet(sid));
                }
                if ui.button("🗑  Löschen").clicked() {
                    del_sheet = true;
                }
            });
        });
        if let Some(pick) = course_pick {
            let fam = self.db.sheets.iter().find(|s| s.id == sid).map(|s| self.db.parallel_family(s)).unwrap_or_default();
            let name = pick.and_then(|id| self.db.courses.iter().find(|c| c.id == id)).map(|c| c.name.clone());
            for x in self.db.sheets.iter_mut().filter(|x| fam.contains(&x.id)) {
                x.course_id = pick;
                if let Some(n) = &name {
                    x.class_name = n.clone();
                }
            }
            changed = true;
        }
        if reseed {
            let r = self.rand();
            if let Some(s) = self.db.sheets.iter_mut().find(|s| s.id == sid) {
                s.seed = r;
            }
            changed = true;
            self.toast("Neue Zahlen gewürfelt – Druck und Lösung passen zusammen.");
        }
        if changed {
            self.changed();
        }
        if mark {
            self.mark_used(sid);
        }
        if fill {
            self.random_fill(sid);
        }
        if del_sheet {
            self.modal = Modal::DeleteSheet(sid);
        }
        if moodle {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let ids = s.task_ids();
            let name = s.title_with_group();
            self.export_moodle(&ids, &name);
        }
        if xml {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let ids = s.task_ids();
            let name = s.title.clone();
            self.export_xml_tasks(&ids, &name);
        }
        if let Some((sol, as_rtf)) = rtf {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let c = export::compose(&self.db, s, sol);
            let ul = s.underline_lines;
            self.save_word(&c, ul, as_rtf);
        }
    }

    fn ui_sheet_preview(&mut self, ui: &mut egui::Ui, sid: u64, solution: bool, acts: &mut Vec<Act>) {
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.preview_zoom, 0.6..=1.6).show_value(false));
            if ui.add(egui::Button::new(RichText::new("🖶  Drucken / PDF").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                acts.push(Act::PrintSheet(sid, solution));
            }
            ui.label(RichText::new("Seitenumbrüche und Seitenzahlen setzt der Druck automatisch; Aufgaben werden nicht zerteilt.").small().weak());
        });
        let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
        let c = export::compose(&self.db, s, solution);
        let underline = s.underline_lines;
        let zoom = self.preview_zoom;
        let size = self.db.settings.print_size * 1.33 * zoom;
        let cache = &mut self.cache;
        egui::ScrollArea::vertical().id_salt("prev").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            let w = (794.0 * zoom).min(ui.available_width() - 20.0).max(300.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(w);
                paper(ui, |ui| {
                    ui.set_width(w - 44.0);
                    ui.set_min_height(1123.0 * zoom - 32.0);
                    ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                        for sec in &c.sections {
                            if let Some((h, p)) = &sec.heading {
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(markup::plain(h)).strong().size(size * 1.08).color(markup::INK));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(RichText::new(p).size(size).color(markup::INK));
                                    });
                                });
                                let r = ui.available_rect_before_wrap();
                                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0_f32, Color32::from_gray(160)));
                                ui.add_space(4.0);
                            }
                            let size = if sec.font_size > 0.0 { size * sec.font_size / self.db.settings.print_size.max(1.0) } else { size };
                            View { doc: &sec.doc, cache, size, zoom }.show(ui);
                            if sec.lines > 0 {
                                let _ = underline;
                                View { doc: &Doc::plain(&format!("[[Linien:{}]]", sec.lines)), cache, size, zoom }.show(ui);
                            }
                        }
                        if !c.footer.is_empty() {
                            ui.add_space(20.0);
                            ui.vertical_centered(|ui| ui.label(RichText::new(&c.footer).size(11.0).color(Color32::GRAY)));
                        }
                    });
                });
            });
            ui.add_space(20.0);
        });
    }

    fn ui_results(&mut self, ui: &mut egui::Ui, sid: u64) {
        let total = self.db.sheet_total(self.db.sheets.iter().find(|s| s.id == sid).unwrap()) as f32;
        let scheme = {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            self.db.settings.scheme(&s.scheme).cloned()
        };
        let Some(sc) = scheme else {
            ui.label("Bitte unter „Bearbeiten“ einen Notenschlüssel wählen.");
            return;
        };
        let rows = sc.rows_for(total);
        let cols = self.db.score_columns(self.db.sheets.iter().find(|s| s.id == sid).unwrap());
        let mut changed = false;
        let (mut print, mut rtf, mut as_rtf) = (false, false, false);
        let (mut import_csv, mut template_csv, mut print_eh) = (false, false, false);
        let mut word_eh: Option<bool> = None;
        let spiegel = export::spiegel(&self.db, self.db.sheets.iter().find(|s| s.id == sid).unwrap());
        // class list of the sheet; students who already wrote another group are left out
        let class: Option<(String, Vec<crate::classes::Student>)> = {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let fam = self.db.parallel_family(s);
            s.course_id.and_then(|cid| self.db.courses.iter().find(|c| c.id == cid)).map(|c| {
                let other: Vec<&Sheet> = self.db.sheets.iter().filter(|x| fam.contains(&x.id) && x.id != sid).collect();
                let missing = c
                    .students
                    .iter()
                    .filter(|st| !s.results.iter().any(|r| r.is_for(st)))
                    .filter(|st| !other.iter().any(|x| x.present_results().any(|r| r.is_for(st))))
                    .cloned()
                    .collect();
                (c.name.clone(), missing)
            })
        };
        let student_names: std::collections::HashMap<u64, String> =
            self.db.courses.iter().flat_map(|c| c.students.iter().map(|s| (s.id, s.name.clone()))).collect();
        let quick = &mut self.quick_points;
        let s = self.db.sheets.iter_mut().find(|s| s.id == sid).unwrap();
        let detail = s.detail_entry && !cols.is_empty();
        let mut results_ui = |ui: &mut egui::Ui| {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                section_label(ui, &format!("ERGEBNISSE  ({} Teilnehmer, max. {} P.)", s.present_results().count(), export::fmt_pts(total)), ACCENT);
                ui.horizontal_wrapped(|ui| {
                    if !detail {
                        ui.add(egui::TextEdit::singleline(quick).hint_text("Punkte, z. B. 34; 27,5; 40").desired_width(190.0));
                        if ui.button("➕  Übernehmen").clicked() {
                            for p in quick.split(|c: char| c == ';' || c.is_whitespace()).filter(|x| !x.is_empty()) {
                                if let Ok(v) = p.replace(',', ".").parse::<f32>() {
                                    s.results.push(ResultRow { points: v.max(0.0), ..Default::default() });
                                }
                            }
                            quick.clear();
                            changed = true;
                        }
                    }
                    if ui.button("➕  Zeile").clicked() {
                        s.results.push(ResultRow::default());
                        changed = true;
                    }
                    if ui
                        .button("📂  Aus CSV …")
                        .on_hover_text(
                            "Ergebnisse aus einer CSV-Datei (z. B. aus Excel) übernehmen:\nSpalten „Name“ und „Punkte“ oder je Aufgabe (1, 2a, 2b …)",
                        )
                        .clicked()
                    {
                        import_csv = true;
                    }
                    if ui
                        .button("📄  CSV-Vorlage")
                        .on_hover_text("Leere Tabelle mit allen Aufgaben und der Klassenliste – in Excel ausfüllen und wieder einlesen")
                        .clicked()
                    {
                        template_csv = true;
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let mut d = s.detail_entry;
                    if ui
                        .add_enabled(!cols.is_empty(), egui::Checkbox::new(&mut d, "Punkte je Aufgabe / Teilaufgabe erfassen"))
                        .on_hover_text("Eine Spalte je Aufgabe bzw. Teilaufgabe (a, b, c); die Summe wird berechnet")
                        .changed()
                    {
                        s.detail_entry = d;
                        changed = true;
                    }
                    if let Some((cname, missing)) = &class {
                        ui.label(RichText::new(format!("👥  Klasse {cname}")).small());
                        if !missing.is_empty()
                            && ui
                                .button(format!("Schülerliste übernehmen ({})", missing.len()))
                                .on_hover_text(
                                    "Legt für jede Schülerin und jeden Schüler eine Zeile an (ohne die, die schon eine andere Gruppe geschrieben haben)",
                                )
                                .clicked()
                        {
                            for st in missing {
                                s.results.push(ResultRow { name: st.name.clone(), student: Some(st.id), ..Default::default() });
                            }
                            changed = true;
                        }
                    }
                });
                let mut remove = None;
                egui::ScrollArea::horizontal().id_salt("resgrid").show(ui, |ui| {
                    let ncols = if detail { 6 + cols.len() } else { 6 };
                    egui::Grid::new("results").striped(true).num_columns(ncols).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label(RichText::new("Nr.").strong());
                        ui.label(RichText::new(if detail { "Name" } else { "Name (optional)" }).strong());
                        if detail {
                            for c in &cols {
                                ui.label(RichText::new(&c.label).strong()).on_hover_text(format!("Aufgabe {} – max. {} P.", c.label, export::fmt_pts(c.max)));
                            }
                        }
                        ui.label(RichText::new(if detail { "Summe" } else { "Punkte" }).strong());
                        ui.label(RichText::new("Note").strong());
                        ui.label(RichText::new("fehlt").strong());
                        ui.label("");
                        ui.end_row();
                        for (i, r) in s.results.iter_mut().enumerate() {
                            ui.label(format!("{}", i + 1));
                            match r.student.and_then(|id| student_names.get(&id)) {
                                // linked to the class list: the name comes from there
                                Some(n) => {
                                    ui.add_sized([150.0, 22.0], egui::Label::new(format!("👥  {n}")).truncate());
                                }
                                None => {
                                    changed |= ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut r.name)).changed();
                                }
                            }
                            if detail {
                                for c in &cols {
                                    let mut v = r.scores.get(&c.key).copied().unwrap_or(0.0);
                                    let resp = ui.add_enabled(
                                        !r.absent,
                                        egui::DragValue::new(&mut v)
                                            .range(0.0..=c.max.max(0.5))
                                            .speed(0.5)
                                            .max_decimals(1)
                                            .custom_formatter(de_num)
                                            .custom_parser(de_parse),
                                    );
                                    if resp.changed() {
                                        r.scores.insert(c.key.clone(), v);
                                        r.sum_scores(&cols);
                                        changed = true;
                                    }
                                }
                                ui.label(RichText::new(export::fmt_pts(r.points)).strong());
                            } else if r.scores.is_empty() {
                                changed |= ui
                                    .add_enabled(
                                        !r.absent,
                                        egui::DragValue::new(&mut r.points)
                                            .range(0.0..=total.max(1.0))
                                            .speed(0.5)
                                            .custom_formatter(de_num)
                                            .custom_parser(de_parse),
                                    )
                                    .changed();
                            } else {
                                ui.label(export::fmt_pts(r.points)).on_hover_text("Summe der Punkte je Aufgabe – ändern mit „Punkte je Aufgabe erfassen“");
                            }
                            let g = if r.absent { "–".to_string() } else { sc.grade_for(total, r.points).map(|g| rows[g].0.clone()).unwrap_or_default() };
                            ui.label(RichText::new(g).strong());
                            changed |= ui.checkbox(&mut r.absent, "").on_hover_text("Nicht mitgeschrieben – zählt nicht im Notenspiegel").changed();
                            if ui.small_button("✕").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                        if detail && !s.results.is_empty() {
                            // average per task: shows which task went well and which did not
                            ui.label("");
                            ui.label(RichText::new("Ø (in % der Punkte)").italics().small());
                            for c in &cols {
                                let v: Vec<f32> = s.present_results().filter_map(|r| r.scores.get(&c.key).copied()).collect();
                                let t = if v.is_empty() || c.max <= 0.0 {
                                    String::new()
                                } else {
                                    format!("{:.0} %", v.iter().sum::<f32>() / v.len() as f32 / c.max * 100.0)
                                };
                                ui.label(RichText::new(t).italics().small());
                            }
                            ui.end_row();
                        }
                    });
                });
                if let Some(i) = remove {
                    s.results.remove(i);
                    changed = true;
                }
                ui.add_space(6.0);
                if s.course_id.is_some() {
                    ui.horizontal(|ui| {
                        ui.label("Gewichtung im Notenbuch");
                        changed |= ui
                            .add(egui::DragValue::new(&mut s.weight).range(0.0..=5.0).speed(0.05).prefix("×").max_decimals(2))
                            .on_hover_text("2 = zählt doppelt, 0,5 = halb, 0 = zählt nicht")
                            .changed();
                    });
                }
                ui.label("Bemerkungen");
                changed |= ui.add(egui::TextEdit::multiline(&mut s.remarks).desired_rows(2).desired_width(f32::INFINITY)).changed();
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("🖶  Erwartungshorizont").on_hover_text("Teilaufgaben mit erwarteter Leistung und Punkten, dazu die Musterlösungen").clicked()
                    {
                        print_eh = true;
                    }
                    if let Some(r) = word_menu(ui, "📄  Erwartungshorizont als Word") {
                        word_eh = Some(r);
                    }
                });
            });
        };
        let mut spiegel_ui = |ui: &mut egui::Ui| {
            let Some(sp) = &spiegel else { return };
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                section_label(ui, "NOTENSPIEGEL", GOOD);
                let maxc = sp.counts.iter().copied().max().unwrap_or(1).max(1) as f32;
                let h = 150.0;
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(560.0), h + 22.0), egui::Sense::hover());
                let n = sp.labels.len().max(1) as f32;
                let bw = r.width() / n;
                let tc = ui.visuals().text_color();
                let p = ui.painter();
                p.hline(r.x_range(), r.top() + h, Stroke::new(1.0_f32, Color32::GRAY));
                for (i, (l, c)) in sp.labels.iter().zip(&sp.counts).enumerate() {
                    let x0 = r.left() + i as f32 * bw;
                    let bh = (*c as f32 / maxc) * (h - 18.0);
                    let br = egui::Rect::from_min_max(egui::pos2(x0 + bw * 0.18, r.top() + h - bh), egui::pos2(x0 + bw * 0.82, r.top() + h));
                    p.rect_filled(br, 3.0, ACCENT);
                    if *c > 0 {
                        p.text(egui::pos2(br.center().x, br.top() - 2.0), egui::Align2::CENTER_BOTTOM, c.to_string(), egui::FontId::proportional(12.0), tc);
                    }
                    p.text(egui::pos2(x0 + bw / 2.0, r.top() + h + 4.0), egui::Align2::CENTER_TOP, l, egui::FontId::proportional(12.0), tc);
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(a) = sp.average {
                        chip(ui, &format!("Durchschnitt {}", format!("{a:.2}").replace('.', ",")), ACCENT);
                    }
                    if let Some(pp) = sp.avg_points {
                        chip(ui, &format!("Ø {} Punkte", format!("{pp:.1}").replace('.', ",")), GOOD);
                    }
                    chip(ui, &format!("nicht bestanden: {}", sp.failed), WARN);
                });
                ui.add_space(6.0);
                egui::ScrollArea::horizontal().id_salt("sps").show(ui, |ui| {
                    egui::Grid::new("sp").striped(true).show(ui, |ui| {
                        ui.label(RichText::new("Note").strong());
                        for l in &sp.labels {
                            ui.label(RichText::new(l).strong());
                        }
                        ui.end_row();
                        ui.label(RichText::new("Anzahl").strong());
                        for c in &sp.counts {
                            ui.label(c.to_string());
                        }
                        ui.end_row();
                    });
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("🖶  Notenspiegel drucken").clicked() {
                        print = true;
                    }
                    if let Some(r) = word_menu(ui, "📄  Als Word") {
                        rtf = true;
                        as_rtf = r;
                    }
                });
                ui.label(
                    RichText::new("Der Ausdruck enthält das Diagramm. Unter „Bearbeiten“ lässt sich der Notenspiegel auch direkt in die Klausur einfügen.")
                        .small()
                        .weak(),
                );
            });
        };
        egui::ScrollArea::vertical().id_salt("res").auto_shrink([false, false]).show(ui, |ui| {
            if detail {
                // one column per task needs the whole width
                results_ui(ui);
                ui.add_space(8.0);
                spiegel_ui(ui);
            } else {
                ui.columns(2, |c| {
                    results_ui(&mut c[0]);
                    spiegel_ui(&mut c[1]);
                });
            }
        });
        if changed {
            // the weight belongs to the whole family of parallel versions (one column in the grade book)
            if let Some(src) = self.db.sheets.iter().find(|s| s.id == sid) {
                let (w, fam) = (src.weight, self.db.parallel_family(src));
                for x in self.db.sheets.iter_mut().filter(|x| fam.contains(&x.id)) {
                    x.weight = w;
                }
            }
            self.changed();
        }
        if print || rtf {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let c = export::compose_spiegel(&self.db, s);
            if print {
                let html = export::to_html(&self.db, &c, true);
                let _ = export::open_html(&c.title, &html);
            } else {
                self.save_word(&c, true, as_rtf);
            }
        }
        if print_eh || word_eh.is_some() {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let c = export::compose_expectation(&self.db, s);
            match word_eh {
                Some(r) => self.save_word(&c, false, r),
                None => {
                    let html = export::to_html(&self.db, &c, false);
                    let _ = export::open_html(&c.title, &html);
                    self.toast("Erwartungshorizont im Browser geöffnet.");
                }
            }
        }
        if template_csv {
            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
            let names: Vec<String> = match s.course_id.and_then(|cid| self.db.courses.iter().find(|c| c.id == cid)) {
                Some(c) => c.students.iter().map(|st| st.name.clone()).collect(),
                None => s.results.iter().map(|r| r.name.clone()).filter(|n| !n.trim().is_empty()).collect(),
            };
            let data = crate::classes::results_template(&cols, &names);
            let name = format!("Ergebnisse {}.csv", export::safe_name(&s.title_with_group()));
            self.save_file(&name, "CSV (Excel)", "csv", data.as_bytes());
        }
        if import_csv && self.shot.is_none() {
            if let Some(p) = rfd::FileDialog::new().add_filter("CSV / Text", &["csv", "txt"]).pick_file() {
                match std::fs::read(&p) {
                    Ok(bytes) => {
                        let text = crate::classes::decode_text(&bytes);
                        let students: Vec<crate::classes::Student> = {
                            let s = self.db.sheets.iter().find(|s| s.id == sid).unwrap();
                            s.course_id.and_then(|cid| self.db.courses.iter().find(|c| c.id == cid)).map(|c| c.students.clone()).unwrap_or_default()
                        };
                        let s = self.db.sheets.iter_mut().find(|s| s.id == sid).unwrap();
                        match crate::classes::import_results(&text, &cols, &students, &mut s.results) {
                            Ok(rep) => {
                                if s.results.iter().any(|r| !r.scores.is_empty()) {
                                    s.detail_entry = true;
                                }
                                let mut m = format!("{} Ergebnisse übernommen", rep.matched + rep.added);
                                if rep.added > 0 && !students.is_empty() {
                                    m += &format!(", davon {} ohne passenden Namen in der Klassenliste", rep.added);
                                }
                                if !rep.ignored_columns.is_empty() {
                                    m += &format!(". Nicht erkannte Spalten: {}", rep.ignored_columns.join(", "));
                                }
                                self.changed();
                                self.toast(m + ".");
                            }
                            Err(e) => self.toast(e),
                        }
                    }
                    Err(e) => self.toast(format!("Die Datei konnte nicht gelesen werden: {e}")),
                }
            }
        }
    }
}
