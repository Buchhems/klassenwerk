//! Overview page, dialogs and settings.

use super::*;

impl App {
    pub(super) fn ui_stats(&mut self, ctx: &egui::Context) {
        let mut acts = Vec::new();
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(8.0);
                ui.heading("Übersicht");
                ui.add_space(6.0);
                let never = self.db.tasks.iter().filter(|t| t.last_any_use().is_none()).count();
                let pts: i32 = self.db.tasks.iter().map(|t| t.points.max(0)).sum();
                let exams = self.db.sheets.iter().filter(|s| self.db.settings.is_exam(&s.kind)).count();
                ui.horizontal_wrapped(|ui| {
                    for (n, l, c) in [
                        (self.db.tasks.len().to_string(), "Aufgaben im Pool", ACCENT),
                        (self.tree().len().to_string(), "Fächer", Color32::from_rgb(14, 116, 144)),
                        (exams.to_string(), "Klausuren & Tests", WARN),
                        ((self.db.sheets.len() - exams).to_string(), "Übungen & Blätter", GOOD),
                        (never.to_string(), "nie verwendet", Color32::GRAY),
                        (pts.to_string(), "Punkte insgesamt", Color32::from_rgb(120, 80, 200)),
                    ] {
                        card(ui, |ui| {
                            ui.set_min_width(150.0);
                            ui.label(RichText::new(n).size(28.0).strong().color(c));
                            ui.label(RichText::new(l).weak());
                        });
                    }
                });
                ui.add_space(10.0);
                ui.columns(2, |cols| {
                    card(&mut cols[0], |ui| {
                        ui.set_width(ui.available_width());
                        section_label(ui, "AUFGABEN JE FACH UND THEMA", ACCENT);
                        let tree = self.tree();
                        let maxn = tree.values().map(|t| t.values().sum::<usize>()).max().unwrap_or(1).max(1) as f32;
                        for (s, topics) in &tree {
                            let n: usize = topics.values().sum();
                            ui.horizontal(|ui| {
                                ui.add_sized([110.0, 20.0], egui::Label::new(RichText::new(s).strong()).truncate());
                                let w = (ui.available_width() - 40.0).max(20.0);
                                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), egui::Sense::hover());
                                ui.painter().rect_filled(egui::Rect::from_min_size(r.min, Vec2::new(w * n as f32 / maxn, 16.0)), 4.0, ACCENT);
                                ui.label(n.to_string());
                            });
                            for (t, c) in topics {
                                ui.label(RichText::new(format!("      {t}: {c}")).small().weak());
                            }
                        }
                    });
                    card(&mut cols[1], |ui| {
                        ui.set_width(ui.available_width());
                        section_label(ui, "ZULETZT BEARBEITET", GOOD);
                        let mut v: Vec<&Task> = self.db.tasks.iter().collect();
                        v.sort_by_key(|t| std::cmp::Reverse(t.updated));
                        for t in v.into_iter().take(8) {
                            if ui.link(format!("{}  ·  {}  ({})", t.title(), t.subject, format_date(&t.updated))).clicked() {
                                acts.push(Act::Select(t.id));
                            }
                        }
                        ui.add_space(8.0);
                        section_label(ui, "KLAUSUREN & ÜBUNGEN", WARN);
                        for s in self.db.sheets.iter().rev().take(6) {
                            if ui.link(format!("{} – {} ({})", s.kind, s.title, s.date)).clicked() {
                                acts.push(Act::OpenSheet(s.id, SheetView::Edit));
                            }
                        }
                    });
                });
            });
        });
        for a in acts {
            if let Act::Select(_) = a {
                self.tab = Tab::Pool;
            }
            self.apply(a);
        }
    }

    pub(super) fn ui_modals(&mut self, ctx: &egui::Context) {
        match std::mem::replace(&mut self.modal, Modal::None) {
            Modal::None => {}
            Modal::DeleteTasks(ids) => {
                let mut keep = true;
                egui::Modal::new(egui::Id::new("del")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("Aufgaben löschen?");
                    ui.label(format!("{} Aufgabe(n) werden endgültig gelöscht und aus allen Mappen entfernt.", ids.len()));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Löschen").color(Color32::WHITE)).fill(Color32::from_rgb(200, 40, 40))).clicked() {
                            self.db.tasks.retain(|t| !ids.contains(&t.id));
                            for s in &mut self.db.sheets {
                                s.items.retain(|i| !matches!(i, SheetItem::Task(id) if ids.contains(id)));
                            }
                            self.marked.retain(|m| !ids.contains(m));
                            if self.selected.map(|s| ids.contains(&s)).unwrap_or(false) {
                                self.selected = None;
                            }
                            self.changed();
                            keep = false;
                        }
                        if ui.button("Abbrechen").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep {
                    self.modal = Modal::DeleteTasks(ids);
                }
            }
            Modal::DeleteSheet(id) => {
                let mut keep = true;
                egui::Modal::new(egui::Id::new("dels")).show(ctx, |ui| {
                    ui.set_width(360.0);
                    ui.heading("Mappe löschen?");
                    ui.label("Die Mappe wird aus dem Klausurverzeichnis gelöscht. Die Aufgaben bleiben im Pool.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Löschen").color(Color32::WHITE)).fill(Color32::from_rgb(200, 40, 40))).clicked() {
                            self.db.sheets.retain(|s| s.id != id);
                            self.sheet_sel = self.db.sheets.first().map(|s| s.id);
                            self.changed();
                            keep = false;
                        }
                        if ui.button("Abbrechen").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep {
                    self.modal = Modal::DeleteSheet(id);
                }
            }
            Modal::DeleteCourse(id) => {
                let mut keep = true;
                egui::Modal::new(egui::Id::new("delc")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("Klasse löschen?");
                    ui.label("Die Klassenliste wird gelöscht. Klausuren und eingetragene Ergebnisse bleiben erhalten (mit den Namen), sind aber keiner Klasse mehr zugeordnet.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Löschen").color(Color32::WHITE)).fill(Color32::from_rgb(200, 40, 40))).clicked() {
                            self.db.courses.retain(|c| c.id != id);
                            for s in self.db.sheets.iter_mut().filter(|s| s.course_id == Some(id)) {
                                s.course_id = None;
                                for r in s.results.iter_mut() {
                                    r.student = None;
                                }
                            }
                            self.course_sel = self.db.courses.first().map(|c| c.id);
                            self.changed();
                            keep = false;
                        }
                        if ui.button("Abbrechen").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep {
                    self.modal = Modal::DeleteCourse(id);
                }
            }
            Modal::Rename { mut field, mut from, mut to } => {
                let mut keep = true;
                let values: BTreeSet<String> = match field {
                    0 => self.db.tasks.iter().map(|t| t.subject.clone()).collect(),
                    1 => self.db.tasks.iter().map(|t| t.topic.clone()).collect(),
                    2 => self.db.tasks.iter().map(|t| t.subtopic.clone()).collect(),
                    _ => self.all_tags(),
                };
                let mut done: Option<usize> = None;
                egui::Modal::new(egui::Id::new("ren")).show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.heading("Global umbenennen");
                    ui.label("Ersetzt eine Bezeichnung in allen Aufgaben auf einmal.");
                    ui.horizontal(|ui| {
                        for (i, l) in ["Fach", "Schlagwort 1", "Schlagwort 2", "Eigenes Schlagwort"].iter().enumerate() {
                            if ui.selectable_label(field == i, *l).clicked() {
                                field = i;
                                from.clear();
                            }
                        }
                    });
                    egui::Grid::new("rg").num_columns(2).show(ui, |ui| {
                        ui.label("Alte Bezeichnung");
                        egui::ComboBox::from_id_salt("rf").width(240.0).selected_text(from.clone()).show_ui(ui, |ui| {
                            for v in &values {
                                ui.selectable_value(&mut from, v.clone(), v);
                            }
                        });
                        ui.end_row();
                        ui.label("Neue Bezeichnung");
                        ui.add(egui::TextEdit::singleline(&mut to).desired_width(240.0));
                        ui.end_row();
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!from.is_empty() && !to.trim().is_empty(), egui::Button::new("Ersetzen")).clicked() {
                            done = Some(self.db.rename(field, &from, to.trim()));
                            keep = false;
                        }
                        if ui.button("Schließen").clicked() {
                            keep = false;
                        }
                    });
                });
                if let Some(n) = done {
                    if field == 0 && self.f_subject.as_deref() == Some(from.as_str()) {
                        self.f_subject = Some(to.trim().to_string());
                    }
                    self.changed();
                    self.toast(format!("{n} Einträge umbenannt."));
                }
                if keep {
                    self.modal = Modal::Rename { field, from, to };
                }
            }
            Modal::Help => {
                let mut open = true;
                egui::Window::new("Formatierung in Aufgaben").open(&mut open).collapsible(false).default_width(540.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                    egui::Grid::new("help").striped(true).num_columns(2).show(ui, |ui| {
                        for (a, b) in [
                            ("**fett**", "fett"),
                            ("{blau:Text}", "farbig: blau, rot, grün, orange, lila, grau"),
                            ("*kursiv*", "kursiv"),
                            ("++unterstrichen++", "unterstrichen"),
                            ("x^{2}  /  x_{1}", "hoch- / tiefgestellt"),
                            ("[ ]  /  [x]", "Ankreuzkästchen leer / angekreuzt"),
                            ("• Punkt", "Aufzählung (Zeile beginnt mit • oder -)"),
                            ("# Überschrift", "Überschrift"),
                            ("---", "Trennlinie"),
                            ("| a) | Text | 3 P. |", "Tabelle ohne Rahmen (z. B. Unteraufgaben)"),
                            ("+| A | B |", "Tabelle mit Rahmen"),
                            ("+| A | < |", "Zelle mit der linken verbinden"),
                            ("+| ^ | B |", "Zelle mit der darüber verbinden"),
                            ("| !A | B |", "Rahmen nur um diese Zelle (in Tabellen mit Rahmen: ohne)"),
                            ("[[Linien:5]]", "5 Leerzeilen für handschriftliche Lösungen"),
                            ("[Text](https://…)", "Hyperlink"),
                            ("{{3}}", "eingefügtes Bild / Formel Nr. 3"),
                            ("√x Formel", "Formeleditor: Cursor auf {{3}} setzen und „√x Formel“ klicken, um die Formel zu ändern"),
                            ("📋  Bild / Alt+V", "Bild aus der Zwischenablage; Bilddateien auch per Ziehen in den Editor"),
                            ("\\*", "Sonderzeichen wörtlich schreiben"),
                        ] {
                            ui.monospace(a);
                            ui.label(b);
                            ui.end_row();
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(RichText::new("Rechtsklick ins Textfeld öffnet ein Einfügemenü. Formeln aus alten Aufgaben bleiben als Grafik erhalten (auch im Word-Export); neue Formeln mit ^{ } _{ } und der Symbolleiste Ω schreiben.").small());
                });
                if open {
                    self.modal = Modal::Help;
                }
            }
            Modal::Settings => {
                let mut open = true;
                self.ui_settings(ctx, &mut open);
                if open {
                    self.modal = Modal::Settings;
                }
            }
            Modal::Update => {
                self.modal = Modal::Update;
                self.ui_update(ctx);
            }
            Modal::Restore => {
                self.modal = Modal::Restore;
                self.ui_restore(ctx);
            }
            Modal::Exchange => {
                self.modal = Modal::Exchange;
                self.ui_exchange(ctx);
            }
        }
    }

    fn ui_settings(&mut self, ctx: &egui::Context, open: &mut bool) {
        let mut changed = false;
        let mut theme = false;
        let mut msg: Option<String> = None;
        let mut reload_samples = false;
        let mut reload_spell = false;
        let mut open_restore = false;
        let mut check_now = false;
        let update_idle = self.update_rx.is_none() && self.download_rx.is_none();
        let update_status = self.update_status.clone();
        let spell_status = self.spell_status.clone();
        let thes_status = self.thes_status.clone();
        let mut reload_thes = false;
        let logo_tex = self.logo_tex.as_ref().map(|(_, t)| t.clone());
        // one size for all tabs, so the window does not jump when switching; smaller on small screens
        let screen = ctx.screen_rect().size();
        let size = egui::vec2(940.0_f32.min(screen.x - 40.0), 640.0_f32.min(screen.y - 60.0));
        egui::Window::new("Einstellungen")
            .open(open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(size)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, l) in ["Schule & Logo", "Darstellung & Druck", "Notenschlüssel", "Druckvorlagen", "Prüfungstypen", "Daten", "KI-Assistent"].iter().enumerate() {
                    ui.selectable_value(&mut self.settings_tab, i as u8, *l);
                }
            });
            ui.separator();
            let st = &mut self.db.settings;
            let h = ui.available_height();
            egui::ScrollArea::vertical().id_salt("set").auto_shrink([false, false]).max_height(h).show(ui, |ui| match self.settings_tab {
                0 => {
                    egui::Grid::new("s0").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label("Name der Schule");
                        changed |= ui.add(egui::TextEdit::singleline(&mut st.school).desired_width(320.0)).changed();
                        ui.end_row();
                        ui.label("Adresse");
                        changed |= ui.add(egui::TextEdit::singleline(&mut st.school_address).desired_width(320.0)).changed();
                        ui.end_row();
                        ui.label("Lehrkraft");
                        changed |= ui.add(egui::TextEdit::singleline(&mut st.teacher).desired_width(320.0)).changed();
                        ui.end_row();
                        ui.label("Fußzeile");
                        changed |= ui.add(egui::TextEdit::singleline(&mut st.footer).hint_text("z. B. Schuljahr 2026/27").desired_width(320.0)).changed();
                        ui.end_row();
                        ui.label("Schullogo");
                        ui.horizontal(|ui| {
                            if ui.button("🖼  Logo wählen …").clicked() {
                                if let Some(p) = rfd::FileDialog::new().add_filter("Bilder", &["png", "jpg", "jpeg", "gif", "bmp"]).pick_file() {
                                    if let Some(img) = std::fs::read(p).ok().and_then(|b| image::load_from_memory(&b).ok()) {
                                        let img = if img.height() > 300 { img.resize(10_000, 300, image::imageops::FilterType::Lanczos3) } else { img };
                                        let mut out = Vec::new();
                                        if img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).is_ok() {
                                            st.logo = out;
                                            changed = true;
                                        }
                                    }
                                }
                            }
                            if !st.logo.is_empty() && ui.button("Entfernen").clicked() {
                                st.logo.clear();
                                changed = true;
                            }
                        });
                        ui.end_row();
                    });
                    if let Some(t) = &logo_tex {
                        if !st.logo.is_empty() {
                            let s = t.size_vec2();
                            ui.add(egui::Image::new((t.id(), Vec2::new(80.0 * s.x / s.y.max(1.0), 80.0))));
                        }
                    }
                    ui.label(RichText::new("Das Logo erscheint oben im Programm und über den Platzhalter {Logo} auf Klausuren, Übungen und dem Notenspiegel.").small().weak());
                }
                1 => {
                    egui::Grid::new("s1").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label("Dunkles Design");
                        if ui.checkbox(&mut st.dark, "").changed() {
                            theme = true;
                            changed = true;
                        }
                        ui.end_row();
                        ui.label("Textgröße der Aufgaben");
                        changed |= ui.add(egui::Slider::new(&mut st.view_size, 11.0..=22.0)).changed();
                        ui.end_row();
                        ui.label("Standardschrift für Druck & Word");
                        egui::ComboBox::from_id_salt("pfont").selected_text(st.print_font.clone()).show_ui(ui, |ui| {
                            for f in PRINT_FONTS {
                                changed |= ui.selectable_value(&mut st.print_font, f.to_string(), f).changed();
                            }
                        });
                        ui.end_row();
                        ui.label("Schriftgröße beim Druck");
                        changed |= ui.add(egui::Slider::new(&mut st.print_size, 9.0..=16.0).suffix(" pt")).changed();
                        ui.end_row();
                    });
                    ui.label(RichText::new("Alle Aufgaben – auch importierte – werden in dieser Schrift gedruckt, außer eine Aufgabe hat im Editor eine eigene Schrift.").small().weak());
                    ui.add_space(10.0);
                    ui.label(RichText::new("Rechtschreibung").strong());
                    if ui.checkbox(&mut st.spell_check, "Im Aufgabeneditor prüfen (unbekannte Wörter rot unterstrichen, Vorschläge per Rechtsklick)").changed() {
                        changed = true;
                        reload_spell = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Wörterbuch");
                        ui.add(egui::TextEdit::singleline(&mut st.spell_dict).hint_text("automatisch (LibreOffice / Linux)").desired_width(330.0));
                        if ui.button("Wählen …").on_hover_text("Hunspell-Wörterbuch (.dic, die .aff-Datei muss daneben liegen), z. B. de_DE_frami.dic").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Hunspell-Wörterbuch", &["dic"]).pick_file() {
                                st.spell_dict = p.display().to_string();
                                changed = true;
                                reload_spell = true;
                            }
                        }
                        if ui.button("Neu laden").clicked() {
                            changed = true;
                            reload_spell = true;
                        }
                    });
                    ui.label(RichText::new(spell_status.clone()).small().weak());
                    ui.add_space(6.0);
                    if ui.checkbox(&mut st.thesaurus, "Thesaurus: Synonyme per Rechtsklick auf ein Wort im Aufgabeneditor").changed() {
                        changed = true;
                        reload_thes = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label("Thesaurus");
                        ui.add(egui::TextEdit::singleline(&mut st.thesaurus_file).hint_text("automatisch (LibreOffice / Linux)").desired_width(330.0));
                        if ui.button("Wählen …").on_hover_text("OpenThesaurus im MyThes-Format, z. B. th_de_DE_v2.dat").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Thesaurus", &["dat", "idx"]).pick_file() {
                                st.thesaurus_file = p.display().to_string();
                                changed = true;
                                reload_thes = true;
                            }
                        }
                        if ui.button("Neu laden").clicked() {
                            changed = true;
                            reload_thes = true;
                        }
                    });
                    ui.label(RichText::new(thes_status.clone()).small().weak());
                    ui.add_space(6.0);
                    ui.label("Eigene Wörter (eins pro Zeile)");
                    let mut words = st.user_words.join("\n");
                    if ui.add(egui::TextEdit::multiline(&mut words).desired_rows(3).desired_width(420.0)).changed() {
                        st.user_words = words.split('\n').map(|w| w.trim_end_matches('\r').to_string()).collect();
                        changed = true;
                    }
                }
                2 => {
                    if self.scheme_sel >= st.schemes.len() {
                        self.scheme_sel = 0;
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (i, s) in st.schemes.iter().enumerate() {
                            ui.selectable_value(&mut self.scheme_sel, i, s.name.clone());
                        }
                        if ui.button("➕  Neu").clicked() {
                            let mut n = st.schemes.get(self.scheme_sel).cloned().unwrap_or_else(|| GradeScheme::defaults().remove(0));
                            n.name = format!("{} (eigener)", n.name);
                            st.schemes.push(n);
                            self.scheme_sel = st.schemes.len() - 1;
                            changed = true;
                        }
                    });
                    ui.separator();
                    let mut delete = false;
                    if let Some(s) = st.schemes.get_mut(self.scheme_sel) {
                        egui::Grid::new("sch").num_columns(2).show(ui, |ui| {
                            ui.label("Name");
                            changed |= ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(260.0)).changed();
                            ui.end_row();
                            ui.label("Nicht bestanden ab Note");
                            changed |= ui.add_sized([60.0, 22.0], egui::TextEdit::singleline(&mut s.fail_from)).changed();
                            ui.end_row();
                            ui.label("Punktgrenzen in");
                            ui.horizontal(|ui| {
                                for (v, l) in KEY_STEPS {
                                    if ui.radio((s.step - v).abs() < 1e-4, l).clicked() {
                                        s.step = v;
                                        changed = true;
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label("Überschrift im Ausdruck");
                            changed |= ui.add(egui::TextEdit::singleline(&mut s.heading).desired_width(260.0)).changed();
                            ui.end_row();
                            ui.label("");
                            ui.horizontal(|ui| {
                                changed |= ui.checkbox(&mut s.show_percent, "Prozentwerte mitdrucken").changed();
                                changed |= ui.checkbox(&mut s.count_row, "Zeile „Anzahl“ zum Eintragen").changed();
                            });
                            ui.end_row();
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Je Note der Anteil der Gesamtpunkte in %, ab dem sie gilt. Die nötigen Punkte werden für jede Klausur ausgerechnet und auf die gewählte Stufung aufgerundet.")
                                .small()
                                .weak(),
                        );
                        let mut rm = None;
                        egui::Grid::new("schr").striped(true).num_columns(3).min_col_width(24.0).show(ui, |ui| {
                            ui.label(RichText::new("Note").strong());
                            ui.label(RichText::new("ab %").strong());
                            ui.label("");
                            ui.end_row();
                            for (i, r) in s.rows.iter_mut().enumerate() {
                                changed |= ui.add_sized([70.0, 22.0], egui::TextEdit::singleline(&mut r.label)).changed();
                                changed |= ui.add(egui::DragValue::new(&mut r.min_percent).range(0.0..=100.0).speed(0.5).suffix(" %")).changed();
                                if ui.small_button("✕").clicked() {
                                    rm = Some(i);
                                }
                                ui.end_row();
                            }
                        });
                        let example = s.rows_for(60.0);
                        if !example.is_empty() {
                            let txt: Vec<String> = example
                                .iter()
                                .filter(|r| r.1 <= r.2)
                                .map(|r| format!("{}: {}–{}", r.0, crate::export::fmt_bound(r.1), crate::export::fmt_bound(r.2)))
                                .collect();
                            ui.label(RichText::new(format!("Beispiel bei 60 Punkten: {}", txt.join("  ·  "))).small().weak());
                        }
                        let lost = s.lost_grades(60.0);
                        if !lost.is_empty() {
                            ui.label(
                                RichText::new(format!("Bei 60 Punkten ist keine Punktzahl für {} möglich – die Prozentwerte liegen zu dicht beieinander.", lost.join(", ")))
                                    .small()
                                    .color(egui::Color32::from_rgb(200, 120, 0)),
                            );
                        }
                        if let Some(i) = rm {
                            s.rows.remove(i);
                            changed = true;
                        }
                        ui.horizontal(|ui| {
                            if ui.button("➕  Notenstufe").clicked() {
                                s.rows.push(GradeRow { label: "?".into(), min_percent: 0.0 });
                                changed = true;
                            }
                            if ui.button("🗑  Notenschlüssel löschen").clicked() {
                                delete = true;
                            }
                        });
                    }
                    if delete && st.schemes.len() > 1 {
                        st.schemes.remove(self.scheme_sel);
                        self.scheme_sel = 0;
                        changed = true;
                    }
                }
                3 => {
                    if self.template_sel >= st.templates.len() {
                        self.template_sel = 0;
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (i, s) in st.templates.iter().enumerate() {
                            ui.selectable_value(&mut self.template_sel, i, s.name.clone());
                        }
                        if ui.button("➕  Neue Vorlage").clicked() {
                            let mut n = st.templates.get(self.template_sel).cloned().unwrap_or_default();
                            n.name = format!("{} (eigene)", n.name);
                            st.templates.push(n);
                            self.template_sel = st.templates.len() - 1;
                            changed = true;
                        }
                    });
                    ui.separator();
                    let mut delete = false;
                    if let Some(t) = st.templates.get_mut(self.template_sel) {
                        ui.columns(2, |cols| {
                            let ui = &mut cols[0];
                            ui.label("Name");
                            changed |= ui.add(egui::TextEdit::singleline(&mut t.name)).changed();
                            ui.label("Kopf der Seite (Formatierung wie in Aufgaben)");
                            changed |= ui.add(egui::TextEdit::multiline(&mut t.header).desired_rows(7).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace)).changed();
                            ui.label("Überschrift jeder Aufgabe");
                            changed |= ui.add(egui::TextEdit::singleline(&mut t.task_heading).desired_width(f32::INFINITY)).changed();
                            ui.label("Schlusstext");
                            changed |= ui.add(egui::TextEdit::singleline(&mut t.closing).desired_width(f32::INFINITY)).changed();
                            changed |= ui.checkbox(&mut t.show_points, "Punkte je Aufgabe anzeigen").changed();
                            changed |= ui.checkbox(&mut t.scale_table, "Notenschlüssel-Tabelle einfügen").changed();
                            changed |= ui.checkbox(&mut t.page_numbers, "Seitenzahlen").changed();
                            if ui.button("🗑  Vorlage löschen").clicked() {
                                delete = true;
                            }
                            let ui = &mut cols[1];
                            ui.label(RichText::new("Platzhalter (klicken zum Einfügen)").strong());
                            for (p, d) in PLACEHOLDERS {
                                ui.horizontal(|ui| {
                                    if ui.small_button(p).clicked() {
                                        t.header.push_str(p);
                                        changed = true;
                                    }
                                    ui.label(RichText::new(d).small().weak());
                                });
                            }
                        });
                    }
                    if delete && st.templates.len() > 1 {
                        st.templates.remove(self.template_sel);
                        self.template_sel = 0;
                        changed = true;
                    }
                }
                4 => {
                    ui.label("Eigene Prüfungs- und Materialtypen. „Zählt als Prüfung“ vermerkt die Verwendung als Klausur.");
                    let mut rm = None;
                    egui::Grid::new("types").striped(true).num_columns(3).min_col_width(24.0).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label(RichText::new("Bezeichnung").strong());
                        ui.label(RichText::new("Art").strong());
                        ui.label("");
                        ui.end_row();
                        for (i, t) in st.types.iter_mut().enumerate() {
                            // Grid cells only get last frame's column width, so reserve the width explicitly.
                            changed |= ui.add_sized([260.0, 22.0], egui::TextEdit::singleline(&mut t.name)).changed();
                            changed |= ui.checkbox(&mut t.exam, "zählt als Prüfung").changed();
                            if ui.small_button("✕").clicked() {
                                rm = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(i) = rm {
                        st.types.remove(i);
                        changed = true;
                    }
                    if ui.button("➕  Typ hinzufügen").clicked() {
                        st.types.push(SheetType { name: "Neuer Typ".into(), exam: false });
                        changed = true;
                    }
                }
                5 => {
                    ui.label(format!("Ihre Daten liegen in: {}", Database::path().display()));
                    ui.label(RichText::new("Es wird automatisch gespeichert.").small().weak());
                    ui.add_space(6.0);
                    if ui.button("💾  Sicherung speichern …").clicked() {
                        if let Some(p) = rfd::FileDialog::new().set_file_name("Klassenwerk-Sicherung.json").add_filter("Sicherung", &["json"]).save_file() {
                            msg = Some("save:".to_string() + &p.to_string_lossy());
                        }
                    }
                    if ui
                        .button("📂  Sicherung öffnen …")
                        .on_hover_text("Eine automatische Sicherung oder eine gespeicherte Datei öffnen und daraus einzelne Aufgaben, Klausuren und Klassen – oder alles – zurückholen")
                        .clicked()
                    {
                        open_restore = true;
                    }
                    if ui.button("📥  Beispielaufgaben erneut laden").on_hover_text("Ergänzt die mitgelieferten Beispielaufgaben, die noch fehlen").clicked() {
                        reload_samples = true;
                    }
                    ui.add_space(12.0);
                    ui.label(RichText::new("Updates").strong());
                    ui.label(format!("Installiert: Klassenwerk {}", env!("CARGO_PKG_VERSION")));
                    changed |= ui
                        .checkbox(&mut st.update_check, "Beim Start nach einer neuen Version suchen (höchstens einmal am Tag)")
                        .on_hover_text(format!("Fragt nur bei GitHub (github.com/{}) die neueste Version ab; es werden keine Daten gesendet.", crate::update::REPO))
                        .changed();
                    ui.horizontal(|ui| {
                        if ui.add_enabled(update_idle, egui::Button::new("🔄  Jetzt prüfen")).clicked() {
                            check_now = true;
                        }
                        ui.label(RichText::new(update_status.clone()).small().weak());
                    });
                }
                _ => {
                    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "Klassenwerk.exe".into());
                    ui.label(RichText::new("KI-Assistent über MCP").strong().size(16.0));
                    ui.label(
                        "Über das Model Context Protocol (MCP) können KI-Assistenten wie Claude auf Ihre Aufgabensammlung zugreifen – \
                         z. B. „Such mir drei Aufgaben zur Integralrechnung, die noch in keiner Klausur waren“ oder \
                         „Schreib eine neue Aufgabe zum Kaufvertrag mit Musterlösung“.",
                    );
                    ui.add_space(6.0);
                    egui::Grid::new("mcpperm").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label(RichText::new("✔  darf").color(GOOD).strong());
                        ui.label("Aufgaben, Klausuren, Fächer und Notenschlüssel lesen; neue Aufgaben und Klausur-Entwürfe anlegen (mit ✦ markiert)");
                        ui.end_row();
                        ui.label(RichText::new("✕ darf nicht").color(Color32::from_rgb(200, 40, 40)).strong());
                        ui.label("Vorhandenes ändern oder löschen");
                        ui.end_row();
                        ui.label(RichText::new("🔒 nie").strong());
                        ui.label("Schülerdaten: Klassenlisten, Namen, Ergebnisse, Notenbuch");
                        ui.end_row();
                    });
                    ui.label(
                        RichText::new("Datenschutz: Aufgabentexte, die der Assistent liest, werden an dessen Anbieter übertragen. Interne Bemerkungen zu Aufgaben werden nicht weitergegeben.")
                            .small()
                            .weak(),
                    );
                    ui.add_space(10.0);
                    section_label(ui, "CLAUDE DESKTOP", ACCENT);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("➕  In Claude Desktop eintragen").on_hover_text("Ergänzt die Konfiguration von Claude Desktop (andere Einträge bleiben erhalten, eine Sicherung wird angelegt)").clicked() {
                            let mut done = Vec::new();
                            let mut errs = Vec::new();
                            for p in crate::mcp::claude_desktop_configs() {
                                match crate::mcp::register_in(&p, &exe) {
                                    Ok(()) => done.push(p.display().to_string()),
                                    Err(e) => errs.push(e),
                                }
                            }
                            msg = Some(if errs.is_empty() && !done.is_empty() {
                                format!("Eingetragen in {}. Bitte Claude Desktop ganz beenden und neu starten.", done.join(" und "))
                            } else if done.is_empty() {
                                format!("Nicht eingetragen: {}", errs.join(" "))
                            } else {
                                format!("Eingetragen in {}; Problem: {}", done.join(", "), errs.join(" "))
                            });
                        }
                        if ui.button("📋  Konfiguration kopieren").clicked() {
                            ui.ctx().copy_text(crate::mcp::config_snippet(&exe));
                            msg = Some("Konfiguration kopiert – in Claude Desktop unter Einstellungen → Entwickler → Konfiguration bearbeiten einfügen.".into());
                        }
                    });
                    ui.add(egui::Label::new(RichText::new(crate::mcp::config_snippet(&exe)).monospace()).wrap());
                    ui.add_space(10.0);
                    section_label(ui, "CLAUDE CODE / ANDERE MCP-PROGRAMME", ACCENT);
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::Label::new(RichText::new(crate::mcp::claude_code_command(&exe)).monospace()).wrap());
                        if ui.small_button("📋").on_hover_text("Befehl kopieren").clicked() {
                            ui.ctx().copy_text(crate::mcp::claude_code_command(&exe));
                            msg = Some("Befehl kopiert.".into());
                        }
                    });
                    ui.label(RichText::new("Andere Programme: Befehl = Pfad zu Klassenwerk.exe, Argument = --mcp (Übertragung über stdio).").small().weak());
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Klassenwerk muss dafür nicht geöffnet sein. Ist es geöffnet, erscheinen neue Einträge des Assistenten nach wenigen Sekunden automatisch.")
                            .small(),
                    );
                }
            });
        });
        if let Some(m) = msg {
            if let Some(p) = m.strip_prefix("save:") {
                let r = self.db.save_to(std::path::Path::new(p));
                self.toast(match r {
                    Ok(_) => "Sicherung gespeichert.".to_string(),
                    Err(e) => format!("Fehler: {e}"),
                });
            } else {
                self.toast(m);
            }
        }
        if reload_spell {
            self.start_spell();
        }
        if reload_thes {
            self.start_thes();
        }
        if open_restore {
            self.open_backup(None);
        }
        if check_now {
            self.check_update(false);
        }
        if reload_samples {
            let sample = Database::with_samples();
            let keys: HashSet<String> = self.db.tasks.iter().map(|t| t.dup_key()).collect();
            let mut n = 0;
            for mut t in sample.tasks {
                if !keys.contains(&t.dup_key()) {
                    t.id = self.db.new_id();
                    self.db.tasks.push(t);
                    n += 1;
                }
            }
            changed = true;
            self.toast(format!("{n} Beispielaufgaben ergänzt."));
        }
        if self.db.settings.logo.is_empty() {
            self.logo_tex = None;
        }
        if theme {
            apply_theme(ctx, self.db.settings.dark);
        }
        if changed {
            self.changed();
        }
    }

    /// Opens a backup for taking over entries: an automatic generation (path given)
    /// or, with `None`, the newest generation if there is one.
    pub(crate) fn open_backup(&mut self, path: Option<std::path::PathBuf>) {
        let path = match path.or_else(|| Database::backup_files().first().map(|b| b.0.clone())) {
            Some(p) => p,
            None => match rfd::FileDialog::new().add_filter("Sicherung", &["json"]).pick_file() {
                Some(p) => p,
                None => return,
            },
        };
        match Database::read_backup(&path) {
            Ok(src) => {
                let label = backup_label(&path);
                self.restore =
                    Some(RestoreState { src, label, tasks: HashSet::new(), sheets: HashSet::new(), courses: HashSet::new(), only_diff: true, tab: 0 });
                self.modal = Modal::Restore;
            }
            Err(e) => self.toast(e),
        }
    }

    fn ui_restore(&mut self, ctx: &egui::Context) {
        let Some(st) = &mut self.restore else {
            self.modal = Modal::None;
            return;
        };
        let mut open = true;
        let (mut take, mut take_all, mut close) = (false, false, false);
        let mut other: Option<Option<std::path::PathBuf>> = None;
        let db = &self.db;
        // state of an entry compared with the current data
        fn state<T: serde::Serialize>(now: Option<&T>, old: &T) -> (&'static str, Color32, bool) {
            match now {
                None => ("fehlt jetzt", WARN, true),
                Some(n) if serde_json::to_string(n).ok() != serde_json::to_string(old).ok() => ("geändert", ACCENT, true),
                Some(_) => ("gleich", Color32::GRAY, false),
            }
        }
        egui::Window::new("Aus einer Sicherung zurückholen")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([680.0, 520.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&st.label).strong());
                    egui::ComboBox::from_id_salt("backup_src").selected_text("Andere Sicherung …").show_ui(ui, |ui| {
                        for (p, _) in Database::backup_files() {
                            if ui.selectable_label(false, backup_label(&p)).clicked() {
                                other = Some(Some(p));
                            }
                        }
                        if ui.selectable_label(false, "Datei wählen …").clicked() {
                            other = Some(None);
                        }
                    });
                });
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {}. Ausgewählte Einträge ersetzen den heutigen Stand oder werden ergänzt; alles andere bleibt, wie es ist.",
                        count(st.src.tasks.len(), "Aufgabe", "Aufgaben"),
                        count(st.src.sheets.len(), "Klausur/Übung", "Klausuren/Übungen"),
                        count(st.src.courses.len(), "Klasse", "Klassen")
                    ))
                    .small()
                    .weak(),
                );
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut st.tab, 0, format!("Aufgaben ({})", st.tasks.len()));
                    ui.selectable_value(&mut st.tab, 1, format!("Klausuren & Übungen ({})", st.sheets.len()));
                    ui.selectable_value(&mut st.tab, 2, format!("Klassen ({})", st.courses.len()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.checkbox(&mut st.only_diff, "nur fehlende und geänderte");
                    });
                });
                ui.separator();
                // (id, title, detail, state)
                let rows: Vec<RestoreRow> = match st.tab {
                    0 => st.src.tasks.iter().map(|t| (t.id, t.title(), format!("{} › {}", t.subject, t.topic), state(db.task(t.id), t))).collect(),
                    1 => st
                        .src
                        .sheets
                        .iter()
                        .map(|s| (s.id, s.title_with_group(), format!("{} · {}", s.kind, s.date), state(db.sheets.iter().find(|x| x.id == s.id), s)))
                        .collect(),
                    _ => st
                        .src
                        .courses
                        .iter()
                        .map(|c| {
                            (c.id, c.name.clone(), format!("{} Schülerinnen und Schüler", c.students.len()), state(db.courses.iter().find(|x| x.id == c.id), c))
                        })
                        .collect(),
                };
                let rows: Vec<_> = rows.into_iter().filter(|r| !st.only_diff || r.3 .2).collect();
                let set = match st.tab {
                    0 => &mut st.tasks,
                    1 => &mut st.sheets,
                    _ => &mut st.courses,
                };
                ui.horizontal(|ui| {
                    if ui.small_button("alle auswählen").clicked() {
                        set.extend(rows.iter().map(|r| r.0));
                    }
                    if ui.small_button("keine").clicked() {
                        for r in &rows {
                            set.remove(&r.0);
                        }
                    }
                });
                egui::ScrollArea::vertical().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
                    if rows.is_empty() {
                        ui.label(RichText::new("Nichts, was fehlt oder sich unterscheidet.").weak());
                    }
                    egui::Grid::new("restore_rows").striped(true).num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
                        for (id, title, detail, (st_text, color, _)) in &rows {
                            let mut on = set.contains(id);
                            if ui.checkbox(&mut on, RichText::new(title).strong()).changed() {
                                if on {
                                    set.insert(*id);
                                } else {
                                    set.remove(id);
                                }
                            }
                            ui.label(RichText::new(detail).small().weak());
                            ui.label(RichText::new(*st_text).small().color(*color));
                            ui.end_row();
                        }
                    });
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let n = st.tasks.len() + st.sheets.len() + st.courses.len();
                    if ui
                        .add_enabled(n > 0, egui::Button::new(RichText::new(format!("Ausgewählte zurückholen ({n})")).color(Color32::WHITE)).fill(ACCENT))
                        .clicked()
                    {
                        take = true;
                    }
                    if ui.button("Ganze Sicherung wiederherstellen").on_hover_text("Ersetzt alle heutigen Daten durch diese Sicherung").clicked() {
                        take_all = true;
                    }
                    if ui.button("Schließen").clicked() {
                        close = true;
                    }
                });
                if !st.sheets.is_empty() {
                    ui.label(RichText::new("Fehlende Aufgaben einer gewählten Klausur werden mit zurückgeholt.").small().weak());
                }
            });
        if let Some(p) = other {
            self.restore = None;
            match p {
                Some(p) => self.open_backup(Some(p)),
                None => {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Sicherung", &["json"]).pick_file() {
                        self.open_backup(Some(p));
                    } else {
                        self.modal = Modal::None;
                    }
                }
            }
            return;
        }
        if take {
            let st = self.restore.take().unwrap();
            let (t, s, c) = self.db.restore_part(&st.src, &st.tasks, &st.sheets, &st.courses);
            self.cache = ObjCache::default();
            self.changed();
            self.toast(format!("Zurückgeholt: {t} Aufgaben, {s} Klausuren/Übungen, {c} Klassen."));
            self.modal = Modal::None;
            return;
        }
        if take_all {
            let st = self.restore.take().unwrap();
            self.db = st.src;
            self.cache = ObjCache::default();
            self.changed();
            self.toast("Sicherung wiederhergestellt.");
            self.modal = Modal::None;
            return;
        }
        if !open || close {
            self.restore = None;
            self.modal = Modal::None;
        }
    }

    /// Writes the tasks as a package into the exchange folder.
    pub(crate) fn share_tasks(&mut self, ids: &[u64]) {
        let dir = self.db.settings.exchange_dir.trim().to_string();
        if dir.is_empty() {
            self.toast("Bitte zuerst einen Austauschordner wählen (Datei → Austauschordner).");
            self.modal = Modal::Exchange;
            return;
        }
        let b = Bundle {
            format: "Klassenwerk-Paket 1".into(),
            author: self.db.settings.teacher.clone(),
            created: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
            tasks: ids.iter().filter_map(|id| self.db.task(*id).cloned()).collect(),
            sheets: vec![],
        };
        let who = if b.author.trim().is_empty() { "Aufgaben".to_string() } else { b.author.clone() };
        let name = format!("{}-{}.kwerk", export::safe_name(&who), chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
        let p = std::path::Path::new(&dir).join(name);
        let json = serde_json::to_string_pretty(&b).unwrap_or_default();
        match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&p, json)) {
            Ok(_) => self.toast(format!("{} Aufgaben im Austauschordner veröffentlicht.", b.tasks.len())),
            Err(e) => self.toast(format!("Austauschordner nicht beschreibbar: {e}")),
        }
    }

    fn ui_exchange(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let dir = self.db.settings.exchange_dir.clone();
        // the folder may be on a network drive: read it again every few seconds only
        let now = ctx.input(|i| i.time);
        if self.exchange_list.as_ref().map(|(t, d, _)| now - *t > 4.0 || *d != dir).unwrap_or(true) {
            self.exchange_list = Some((now, dir.clone(), exchange_files(&dir, &self.db)));
        }
        let files = &self.exchange_list.as_ref().unwrap().2;
        let mut pick_dir = false;
        let mut share_shown = false;
        let mut take: Vec<std::path::PathBuf> = Vec::new();
        let n_shown = self.filtered().len();
        egui::Window::new("Austauschordner")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([640.0, 420.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Ein gemeinsamer Ordner – Netzlaufwerk der Schule, Cloud-Ordner oder USB-Stick. Wer Aufgaben teilt, legt dort ein Paket ab; die anderen holen sich daraus, was ihnen noch fehlt. Doppelte Aufgaben werden übersprungen.")
                        .small()
                        .weak(),
                );
                ui.horizontal(|ui| {
                    ui.label("Ordner");
                    ui.label(RichText::new(if dir.is_empty() { "– noch keiner gewählt –".to_string() } else { dir.clone() }).strong());
                    if ui.button("Wählen …").clicked() {
                        pick_dir = true;
                    }
                });
                if dir.is_empty() {
                    return;
                }
                ui.add_space(4.0);
                if ui
                    .button(format!("📤  Angezeigte Aufgaben teilen ({n_shown})"))
                    .on_hover_text("Einzelne Aufgaben teilen: in der Liste markieren und „Teilen“ wählen")
                    .clicked()
                {
                    share_shown = true;
                }
                ui.separator();
                ui.label(RichText::new("Pakete im Ordner").strong());
                egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                    if files.is_empty() {
                        ui.label(RichText::new("Noch keine Pakete im Ordner.").weak());
                    }
                    egui::Grid::new("exchange_files").striped(true).num_columns(4).spacing([12.0, 4.0]).show(ui, |ui| {
                        for f in files {
                            ui.label(RichText::new(&f.author).strong());
                            ui.label(RichText::new(&f.created).small().weak());
                            ui.label(format!("{}, davon {} neu", count(f.tasks, "Aufgabe", "Aufgaben"), f.new));
                            if ui.add_enabled(f.new > 0, egui::Button::new("Holen")).clicked() {
                                take.push(f.path.clone());
                            }
                            ui.end_row();
                        }
                    });
                });
                let all_new: Vec<std::path::PathBuf> = files.iter().filter(|f| f.new > 0).map(|f| f.path.clone()).collect();
                if !all_new.is_empty() && ui.button(format!("Alle neuen Aufgaben holen ({})", files.iter().map(|f| f.new).sum::<usize>())).clicked() {
                    take = all_new;
                }
            });
        let refresh = pick_dir || share_shown || !take.is_empty();
        if pick_dir {
            if let Some(p) = rfd::FileDialog::new().set_title("Austauschordner wählen").pick_folder() {
                self.db.settings.exchange_dir = p.display().to_string();
                self.changed();
            }
        }
        if share_shown {
            let ids = self.filtered();
            self.share_tasks(&ids);
        }
        if !take.is_empty() {
            self.import_paths(take);
        }
        if refresh {
            self.exchange_list = None;
        }
        if !open {
            self.modal = Modal::None;
        }
    }
}

/// "Automatische Sicherung vom 17.11.2026, 14:05" or the file name.
fn backup_label(p: &std::path::Path) -> String {
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let time = std::fs::metadata(p).and_then(|m| m.modified()).ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%d.%m.%Y, %H:%M").to_string());
    if name.starts_with("datenbank.") && p.parent() == Database::path().parent() {
        format!("Automatische Sicherung vom {}", time.unwrap_or_default())
    } else {
        match time {
            Some(t) => format!("{name} ({t})"),
            None => name,
        }
    }
}

/// "1 Aufgabe", "3 Aufgaben"
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A line of the restore window: id, title, detail, (state text, colour, missing or changed).
type RestoreRow = (u64, String, String, (&'static str, Color32, bool));

/// A package in the exchange folder.
pub(crate) struct ExchangeFile {
    path: std::path::PathBuf,
    author: String,
    created: String,
    tasks: usize,
    /// tasks that are not in the collection yet
    new: usize,
}

/// The packages in the exchange folder, newest first.
fn exchange_files(dir: &str, db: &Database) -> Vec<ExchangeFile> {
    if dir.trim().is_empty() {
        return vec![];
    }
    let keys: HashSet<String> = db.tasks.iter().map(|t| t.dup_key()).collect();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x.to_string_lossy().to_lowercase()) != Some("kwerk".into()) {
            continue;
        }
        let Some(b) = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Bundle>(&s).ok()) else { continue };
        let new = b.tasks.iter().filter(|t| !keys.contains(&t.dup_key())).count();
        let author = if b.author.trim().is_empty() { p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() } else { b.author };
        out.push(ExchangeFile { path: p, author, created: b.created, tasks: b.tasks.len(), new });
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}
