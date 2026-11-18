//! Task pool: subject tree, task list, detail view and editor.

use super::*;
use crate::markup::{self, View};

const SYMBOLS: &str = "αβγδελμπρσφωΔΣΩΦ∫√∞≈≠≤≥±·×÷→⇒⇔∈∉∅∩∪⊂∀∃∂∑∏∠⊥∥°²³½¼€‰ℝℕ";

impl App {
    pub(super) fn ui_pool(&mut self, ctx: &egui::Context) {
        let mut acts: Vec<Act> = Vec::new();
        egui::SidePanel::left("nav").resizable(true).default_width(240.0).width_range(200.0..=360.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("🔍  Volltextsuche …").desired_width(f32::INFINITY));
            ui.horizontal_wrapped(|ui| {
                ui.toggle_value(&mut self.only_fav, "⭐ Favoriten");
                ui.toggle_value(&mut self.only_unused, "Nie verwendet");
            });
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(RichText::new("➕  Neue Aufgabe").color(Color32::WHITE)).fill(ACCENT).min_size(Vec2::new(ui.available_width(), 30.0)))
                .clicked()
            {
                acts.push(Act::NewTask);
            }
            ui.add_space(6.0);
            ui.separator();
            egui::ScrollArea::vertical().id_salt("tree").auto_shrink([false, false]).show(ui, |ui| {
                section_label(ui, "FÄCHER", Color32::GRAY);
                let total = self.db.tasks.len();
                if ui.selectable_label(self.f_subject.is_none() && self.f_tag.is_none(), format!("📂  Alle Aufgaben  ({total})")).clicked() {
                    self.f_subject = None;
                    self.f_topic = None;
                    self.f_tag = None;
                }
                for (subj, topics) in self.tree() {
                    let n: usize = topics.values().sum();
                    let open = self.f_subject.as_deref() == Some(subj.as_str());
                    let name = if subj.is_empty() { "(ohne Fach)".to_string() } else { subj.clone() };
                    if ui.selectable_label(open && self.f_topic.is_none(), format!("{}  {name}  ({n})", if open { "▾" } else { "▸" })).clicked() {
                        if open && self.f_topic.is_none() {
                            self.f_subject = None;
                        } else {
                            self.f_subject = Some(subj.clone());
                        }
                        self.f_topic = None;
                    }
                    if open {
                        for (topic, c) in topics {
                            let sel = self.f_topic.as_deref() == Some(topic.as_str());
                            ui.horizontal(|ui| {
                                ui.add_space(18.0);
                                let tn = if topic.is_empty() { "(ohne Schlagwort 1)".to_string() } else { topic.clone() };
                                if ui.selectable_label(sel, format!("{tn}  ({c})")).clicked() {
                                    self.f_topic = if sel { None } else { Some(topic.clone()) };
                                }
                            });
                        }
                    }
                }
                let tags = self.all_tags();
                ui.add_space(10.0);
                section_label(ui, "SCHLAGWÖRTER", Color32::GRAY);
                if tags.is_empty() {
                    ui.label(RichText::new("Noch keine – beim Bearbeiten einer Aufgabe vergeben.").small().weak());
                }
                ui.horizontal_wrapped(|ui| {
                    for tag in tags {
                        let sel = self.f_tag.as_deref() == Some(tag.as_str());
                        if ui.selectable_label(sel, format!("# {tag}")).clicked() {
                            self.f_tag = if sel { None } else { Some(tag) };
                        }
                    }
                });
            });
        });

        let ids = self.filtered();
        let sheets: Vec<(u64, String)> = self.db.sheets.iter().filter(|s| !s.archived).map(|s| (s.id, s.title.clone())).collect();
        let mut bulk_export: Option<Vec<u64>> = None;
        let mut bulk_moodle: Option<Vec<u64>> = None;
        let mut bulk_share: Option<Vec<u64>> = None;
        egui::SidePanel::left("list").resizable(true).default_width(330.0).width_range(260.0..=520.0).show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} Aufgaben", ids.len())).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = match self.sort {
                        TaskSort::Subject => "Nach Fach",
                        TaskSort::Title => "Nach Titel",
                        TaskSort::Points => "Nach Punkten",
                        TaskSort::LastUsed => "Zuletzt verwendet",
                        TaskSort::Newest => "Neueste",
                    };
                    egui::ComboBox::from_id_salt("sort").width(130.0).selected_text(label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.sort, TaskSort::Subject, "Nach Fach");
                        ui.selectable_value(&mut self.sort, TaskSort::Title, "Nach Titel");
                        ui.selectable_value(&mut self.sort, TaskSort::Points, "Nach Punkten");
                        ui.selectable_value(&mut self.sort, TaskSort::LastUsed, "Zuletzt verwendet");
                        ui.selectable_value(&mut self.sort, TaskSort::Newest, "Neueste");
                    });
                });
            });
            if !self.marked.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{} markiert", self.marked.len())).color(ACCENT));
                    if ui.small_button("🗑  Löschen").clicked() {
                        acts.push(Act::AskDelete(self.marked.iter().copied().collect()));
                    }
                    if ui.small_button("📦  Exportieren").clicked() {
                        bulk_export = Some(self.marked.iter().copied().collect());
                    }
                    if ui.small_button("🎓  Moodle").on_hover_text("Als Moodle-XML-Fragen exportieren").clicked() {
                        bulk_moodle = Some(self.marked.iter().copied().collect());
                    }
                    if ui.small_button("📤  Teilen").on_hover_text("Im Austauschordner veröffentlichen").clicked() {
                        bulk_share = Some(self.marked.iter().copied().collect());
                    }
                    if ui.small_button("✕ Aufheben").clicked() {
                        self.marked.clear();
                    }
                });
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical().id_salt("tasks").auto_shrink([false, false]).show(ui, |ui| {
                for id in &ids {
                    let Some(t) = self.db.task(*id) else { continue };
                    let sel = self.selected == Some(*id);
                    let fill = if sel { sel_fill(ui) } else { card_fill(ui) };
                    let stroke = if sel { Stroke::new(1.5_f32, ACCENT) } else { Stroke::new(1.0_f32, border(ui)) };
                    let r = egui::Frame::new().fill(fill).stroke(stroke).corner_radius(9).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let mut m = self.marked.contains(id);
                            if ui.checkbox(&mut m, "").on_hover_text("Für Sammelaktionen markieren").changed() {
                                if m {
                                    self.marked.insert(*id);
                                } else {
                                    self.marked.remove(id);
                                }
                            }
                            ui.label(RichText::new(t.title()).strong().size(14.5));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                chip(ui, &format!("{} P.", t.points), ACCENT);
                                if t.favorite {
                                    ui.label(RichText::new("⭐").size(12.0));
                                }
                            });
                        });
                        ui.label(RichText::new(format!("{} › {}", t.subject, t.topic)).small().weak());
                        let vals = (!t.vars.is_empty()).then(|| crate::vary::values(t, 0));
                        let snippet: String = markup::plain_doc(&t.task, vals.as_ref()).chars().take(90).collect();
                        ui.label(RichText::new(snippet).small());
                        ui.horizontal(|ui| {
                            match t.last_any_use() {
                                Some(d) => {
                                    let col = if t.exam_last.is_some() { WARN } else { GOOD };
                                    ui.label(RichText::new(format!("● zuletzt verwendet {}", d.format("%d.%m.%Y"))).size(11.0).color(col));
                                }
                                None => {
                                    ui.label(RichText::new("○ noch nie verwendet").size(11.0).weak());
                                }
                            }
                            for tag in t.tags.iter().take(3) {
                                ui.label(RichText::new(format!("#{tag}")).size(11.0).color(ACCENT));
                            }
                        });
                    });
                    let resp = r.response.interact(egui::Sense::click());
                    if resp.clicked() {
                        acts.push(Act::Select(*id));
                    }
                    if resp.double_clicked() {
                        acts.push(Act::Edit(*id));
                    }
                    resp.context_menu(|ui| {
                        if ui.button("✏  Bearbeiten").clicked() {
                            acts.push(Act::Edit(*id));
                            ui.close_menu();
                        }
                        if ui.button("🗐  Kopieren").clicked() {
                            acts.push(Act::Duplicate(*id));
                            ui.close_menu();
                        }
                        ui.menu_button("➕  Zur Klausur/Übung", |ui| {
                            for (sid, st) in &sheets {
                                if ui.button(st).clicked() {
                                    acts.push(Act::AddToSheet(*sid, *id));
                                    ui.close_menu();
                                }
                            }
                            if ui.button("➕  Neue Klausur …").clicked() {
                                acts.push(Act::AddToNewSheet(*id));
                                ui.close_menu();
                            }
                        });
                        if ui.button("👁  Druckvorschau").clicked() {
                            acts.push(Act::PrintTask(*id));
                            ui.close_menu();
                        }
                        if ui.button("🗑  Löschen").clicked() {
                            acts.push(Act::AskDelete(vec![*id]));
                            ui.close_menu();
                        }
                    });
                    ui.add_space(5.0);
                }
                if ids.is_empty() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Keine Aufgaben gefunden.").weak());
                        ui.label(RichText::new("Suche oder Filter anpassen.").small().weak());
                    });
                }
            });
        });
        if let Some(m) = bulk_moodle {
            self.export_moodle(&m, "Aufgaben");
        }
        if let Some(m) = bulk_share {
            self.share_tasks(&m);
        }
        if let Some(m) = bulk_export {
            self.export_bundle(&m, &[]);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.editor.is_some() {
                self.ui_editor(ui);
                return;
            }
            self.ui_detail(ui, &mut acts, &sheets);
        });
        for a in acts {
            self.apply(a);
        }
    }

    fn ui_detail(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, sheets: &[(u64, String)]) {
        let Some(id) = self.selected else {
            ui.centered_and_justified(|ui| ui.label(RichText::new("Wählen Sie links eine Aufgabe aus.").weak()));
            return;
        };
        let Some(t) = self.db.task(id) else {
            self.selected = None;
            return;
        };
        let in_sheets: Vec<String> = self.db.sheets.iter().filter(|s| s.contains(id)).map(|s| format!("{} ({})", s.title, s.date)).collect();
        // tasks with random numbers are shown with example values
        let tv = t.varied(0);
        let t = &tv;
        let size = self.db.settings.view_size * t.print_font(&self.db.settings).1 / self.db.settings.print_size.max(1.0);
        let show_sol = self.db.settings.show_solution;
        let side = self.db.settings.side_by_side;
        let mut set_side = side;
        let mut set_sol = show_sol;
        let cache = &mut self.cache;
        egui::ScrollArea::vertical().id_salt("detail").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let star = if t.favorite { "⭐" } else { "☆" };
                if ui.add(egui::Button::new(RichText::new(star).size(18.0)).frame(false)).on_hover_text("Favorit").clicked() {
                    acts.push(Act::ToggleFav(id));
                }
                ui.heading(t.title());
            });
            ui.horizontal_wrapped(|ui| {
                chip(ui, &t.subject, ACCENT);
                chip(ui, &t.topic, Color32::from_rgb(14, 116, 144));
                chip(ui, &format!("{} Punkte", t.points), GOOD);
                if !t.parts.is_empty() {
                    let parts: Vec<String> = t.parts.iter().map(|p| format!("{}) {}", p.label, p.points)).collect();
                    ui.scope(|ui| chip(ui, &parts.join(" · "), Color32::from_rgb(14, 116, 144))).response.on_hover_text(
                        t.parts
                            .iter()
                            .map(|p| {
                                format!(
                                    "{}) {} P.{}",
                                    p.label,
                                    p.points,
                                    if p.expectation.trim().is_empty() { String::new() } else { format!(" – {}", p.expectation.trim()) }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
                if !t.origin.is_empty() {
                    chip(ui, &format!("✦ {}", t.origin), Color32::from_rgb(190, 90, 20));
                }
                if t.minutes > 0 {
                    chip(ui, &format!("⏱ {} Min.", t.minutes), Color32::GRAY);
                }
                if t.difficulty > 0 {
                    chip(ui, ["", "leicht", "mittel", "schwer"][t.difficulty.min(3) as usize], WARN);
                }
                if t.answer_lines > 0 {
                    chip(ui, &format!("{} Leerzeilen", t.answer_lines), Color32::GRAY);
                }
                for tag in &t.tags {
                    chip(ui, &format!("#{tag}"), Color32::from_rgb(120, 80, 200));
                }
                if !t.vars.is_empty() {
                    ui.scope(|ui| chip(ui, "🎲  Zufallszahlen", Color32::from_rgb(120, 80, 200)))
                        .response
                        .on_hover_text("Die Zahlen sind Beispielwerte; jede Klausur und jede Gruppe bekommt eigene.");
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("✏  Bearbeiten").clicked() {
                    acts.push(Act::Edit(id));
                }
                if ui.button("🗐  Kopieren").on_hover_text("Als Grundlage für eine neue Aufgabe kopieren").clicked() {
                    acts.push(Act::Duplicate(id));
                }
                ui.menu_button("➕  Zur Klausur/Übung", |ui| {
                    for (sid, st) in sheets {
                        if ui.button(st).clicked() {
                            acts.push(Act::AddToSheet(*sid, id));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button("➕  Neue Klausur anlegen …").clicked() {
                        acts.push(Act::AddToNewSheet(id));
                        ui.close_menu();
                    }
                });
                if ui.button("👁  Druckvorschau").clicked() {
                    acts.push(Act::PrintTask(id));
                }
                if let Some(rtf) = word_menu(ui, "📄  Word") {
                    acts.push(Act::WordTask(id, rtf));
                }
                if ui.button("🗑  Löschen").clicked() {
                    acts.push(Act::AskDelete(vec![id]));
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut set_sol, "Musterlösung zeigen");
                ui.checkbox(&mut set_side, "Aufgabe und Lösung nebeneinander");
            });
            ui.add_space(8.0);
            let lines = t.answer_lines;
            let render = |ui: &mut egui::Ui, cache: &mut ObjCache, doc: &Doc, label: &str, col: Color32, lines: u32| {
                section_label(ui, label, col);
                paper(ui, |ui| {
                    ui.set_width(ui.available_width());
                    View { doc, cache, size, zoom: 1.0 }.show(ui);
                    if lines > 0 {
                        View { doc: &Doc::plain(&format!("[[Linien:{lines}]]")), cache, size, zoom: 1.0 }.show(ui);
                    }
                });
            };
            if show_sol && side && ui.available_width() > 700.0 {
                ui.columns(2, |cols| {
                    render(&mut cols[0], cache, &t.task, "AUFGABE", ACCENT, lines);
                    render(&mut cols[1], cache, &t.solution, "MUSTERLÖSUNG", GOOD, 0);
                });
            } else {
                render(ui, cache, &t.task, "AUFGABE", ACCENT, lines);
                if show_sol {
                    ui.add_space(10.0);
                    render(ui, cache, &t.solution, "MUSTERLÖSUNG", GOOD, 0);
                }
            }
            ui.add_space(12.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                section_label(ui, "VERWENDUNG & INFOS", Color32::GRAY);
                egui::Grid::new("info").num_columns(4).spacing([24.0, 4.0]).show(ui, |ui| {
                    ui.label(RichText::new("Erstellt").weak());
                    ui.label(format!("{} ({})", format_date(&t.created), t.author_new));
                    ui.label(RichText::new("Geändert").weak());
                    ui.label(format!("{} ({})", format_date(&t.updated), t.author_upd));
                    ui.end_row();
                    ui.label(RichText::new("Im Unterricht").weak());
                    ui.label(format!("{} – {}", format_date(&t.first_used), format_date(&t.last_used)));
                    ui.label(RichText::new("In Klausuren").weak());
                    ui.label(format!("{} – {}", format_date(&t.exam_first), format_date(&t.exam_last)));
                    ui.end_row();
                });
                if !in_sheets.is_empty() {
                    ui.label(RichText::new(format!("Enthalten in: {}", in_sheets.join(", "))).small());
                }
                for u in t.usage.iter().rev().take(6) {
                    ui.label(RichText::new(format!("• {} – {} {}", u.date, u.sheet, if u.exam { "(Klausur)" } else { "" })).small());
                }
                if !t.notes.trim().is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("🔒 Interne Bemerkung").small().strong());
                    ui.label(&t.notes);
                }
            });
        });
        if set_side != side || set_sol != show_sol {
            self.db.settings.side_by_side = set_side;
            self.db.settings.show_solution = set_sol;
            self.changed();
        }
    }

    /// Formatting toolbar above a task/solution editor. Returns a message for the
    /// status line (e.g. "no picture on the clipboard"), if any.
    fn toolbar(
        ui: &mut egui::Ui,
        text: &mut String,
        cur: &mut (usize, usize),
        objects: &mut Vec<DocObject>,
        id: egui::Id,
        formula: &mut bool,
    ) -> Option<String> {
        let mut ins: Option<(String, String)> = None;
        let mut msg: Option<String> = None;
        let mut table_cmd: Option<crate::table::Cmd> = None;
        let paste = |objects: &mut Vec<DocObject>, ins: &mut Option<(String, String)>, msg: &mut Option<String>| match clipboard_png() {
            Ok(png) => match add_image_object(objects, &png) {
                Some(m) => *ins = Some((m, String::new())),
                None => *msg = Some("Das Bild konnte nicht gelesen werden.".into()),
            },
            Err(e) => *msg = Some(e),
        };
        // Alt+V: picture from the clipboard (Ctrl+V stays text paste)
        let alt_v = ui.ctx().memory(|m| m.has_focus(id))
            && ui.input_mut(|i| {
                let hit = i.consume_key(egui::Modifiers::ALT, egui::Key::V);
                if hit {
                    // some keyboards also deliver the letter itself; keep it out of the text
                    i.events.retain(|e| !matches!(e, egui::Event::Text(t) if t.eq_ignore_ascii_case("v")));
                }
                hit
            });
        if alt_v {
            paste(objects, &mut ins, &mut msg);
        }
        let btn = |ui: &mut egui::Ui, label: RichText, tip: &str| ui.add(egui::Button::new(label).min_size(Vec2::new(28.0, 26.0))).on_hover_text(tip).clicked();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if btn(ui, RichText::new("B").strong(), "Fett") {
                ins = Some(("**".into(), "**".into()));
            }
            if btn(ui, RichText::new("I").italics(), "Kursiv") {
                ins = Some(("*".into(), "*".into()));
            }
            if btn(ui, RichText::new("U").underline(), "Unterstrichen") {
                ins = Some(("++".into(), "++".into()));
            }
            if btn(ui, RichText::new("x²"), "Hochgestellt") {
                ins = Some(("^{".into(), "}".into()));
            }
            if btn(ui, RichText::new("x₂"), "Tiefgestellt") {
                ins = Some(("_{".into(), "}".into()));
            }
            ui.separator();
            if btn(ui, RichText::new("☐"), "Ankreuzkästchen") {
                ins = Some(("[ ] ".into(), String::new()));
            }
            if btn(ui, RichText::new("☒"), "Angekreuztes Kästchen") {
                ins = Some(("[x] ".into(), String::new()));
            }
            if btn(ui, RichText::new("•"), "Aufzählung") {
                ins = Some(("\n• ".into(), String::new()));
            }
            if btn(ui, RichText::new("H"), "Überschrift") {
                ins = Some(("\n# ".into(), String::new()));
            }
            if btn(ui, RichText::new("―"), "Trennlinie") {
                ins = Some(("\n---\n".into(), String::new()));
            }
            ui.separator();
            let in_table = crate::table::in_table(text, cur.0);
            ui.menu_button("▦ Tabelle", |ui| {
                if ui.button("Neue Tabelle mit Rahmen").clicked() {
                    ins = Some(("\n+| Spalte 1 | Spalte 2 | Spalte 3 |\n+| … | … | … |\n".into(), String::new()));
                    ui.close_menu();
                }
                if ui.button("Neue Tabelle ohne Rahmen").clicked() {
                    ins = Some(("\n| Spalte 1 | Spalte 2 | Spalte 3 |\n| … | … | … |\n".into(), String::new()));
                    ui.close_menu();
                }
                ui.separator();
                if !in_table {
                    ui.label(RichText::new("Für die folgenden Befehle den Cursor\nin eine Tabellenzelle setzen.").small().weak());
                }
                use crate::table::Cmd;
                for (label, c, tip) in [
                    ("⬆  Zeile darüber einfügen", Cmd::RowAbove, ""),
                    ("⬇  Zeile darunter einfügen", Cmd::RowBelow, ""),
                    ("⬅  Spalte links einfügen", Cmd::ColLeft, ""),
                    ("➡  Spalte rechts einfügen", Cmd::ColRight, ""),
                    ("🗑  Zeile löschen", Cmd::DeleteRow, ""),
                    ("🗑  Spalte löschen", Cmd::DeleteCol, ""),
                    ("⇥  Mit Zelle rechts verbinden", Cmd::MergeRight, "Steht im Text als < in der verbundenen Zelle"),
                    ("⇣  Mit Zelle darunter verbinden", Cmd::MergeDown, "Steht im Text als ^ in der verbundenen Zelle"),
                    ("⊞  Verbundene Zelle teilen", Cmd::Split, ""),
                    ("▢  Rahmen dieser Zelle an/aus", Cmd::Border, "Steht im Text als ! vor dem Zellinhalt"),
                ] {
                    let r = ui.add_enabled(in_table, egui::Button::new(label));
                    let r = if tip.is_empty() { r } else { r.on_hover_text(tip) };
                    if r.clicked() {
                        table_cmd = Some(c);
                        ui.close_menu();
                    }
                }
            });
            if btn(ui, RichText::new("a) b)"), "Unteraufgaben mit Punkten") {
                ins = Some(("\n| a) | Teilaufgabe | 2 P. |\n| b) | Teilaufgabe | 3 P. |\n".into(), String::new()));
            }
            if btn(ui, RichText::new("☰ Leerzeilen"), "Linien für handschriftliche Lösungen") {
                ins = Some(("\n[[Linien:4]]\n".into(), String::new()));
            }
            if btn(ui, RichText::new("🔗"), "Hyperlink") {
                ins = Some(("[".into(), "](https://)".into()));
            }
            if btn(ui, RichText::new("🖼 Bild"), "Bild oder Grafik einfügen (wird automatisch optimiert)") {
                if let Some(p) = rfd::FileDialog::new().add_filter("Bilder", &["png", "jpg", "jpeg", "gif", "bmp"]).pick_file() {
                    match std::fs::read(p).ok().and_then(|bytes| add_image_object(objects, &bytes)) {
                        Some(m) => ins = Some((m, String::new())),
                        None => msg = Some("Die Datei ist kein lesbares Bild.".into()),
                    }
                }
            }
            if btn(
                ui,
                RichText::new("√x Formel"),
                "Formel schreiben (Brüche, Wurzeln, Integrale, Matrizen …).\nSteht der Cursor auf einer Formel $…$, wird sie bearbeitet.\nFormeln lassen sich auch direkt tippen: $x^2 + 1$.",
            ) {
                *formula = true;
            }
            if btn(
                ui,
                RichText::new("📋 Bild"),
                "Bild aus der Zwischenablage einfügen (Alt+V), z. B. einen Screenshot.\nBilddateien kann man auch einfach in den Editor ziehen.",
            ) {
                paste(objects, &mut ins, &mut msg);
            }
            ui.menu_button("Ω Symbole", |ui| {
                ui.set_max_width(260.0);
                ui.horizontal_wrapped(|ui| {
                    for c in SYMBOLS.chars() {
                        if ui.add(egui::Button::new(RichText::new(c.to_string()).size(16.0)).min_size(Vec2::splat(28.0))).clicked() {
                            ins = Some((c.to_string(), String::new()));
                            ui.close_menu();
                        }
                    }
                });
            });
        });
        if let Some(c) = table_cmd {
            match crate::table::apply(text, cur.0, c) {
                Some((t, pos)) => {
                    *text = t;
                    *cur = (pos, pos);
                    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
                        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(pos))));
                        st.store(ui.ctx(), id);
                    }
                    ui.ctx().memory_mut(|m| m.request_focus(id));
                }
                None => msg = Some("Das geht an dieser Stelle der Tabelle nicht.".into()),
            }
        }
        if let Some((a, b)) = ins {
            let pos = insert_wrap(text, cur, &a, &b);
            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
                st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(pos))));
                st.store(ui.ctx(), id);
            }
            ui.ctx().memory_mut(|m| m.request_focus(id));
        }
        msg
    }

    fn ui_editor(&mut self, ui: &mut egui::Ui) {
        let subjects: Vec<String> = self.subjects().into_iter().collect();
        let topics: Vec<String> = self.db.tasks.iter().map(|t| t.topic.clone()).collect::<BTreeSet<_>>().into_iter().filter(|s| !s.is_empty()).collect();
        let (print_font, print_size) = (self.db.settings.print_font.clone(), self.db.settings.print_size);
        let size = self.db.settings.view_size;
        let cache = &mut self.cache;
        let toast = &mut self.toast;
        // spell checking: underline unknown words, suggestions in the right-click menu
        let spell = if self.db.settings.spell_check { self.spell.clone() } else { None };
        let thes = if self.db.settings.thesaurus { self.thes.clone() } else { None };
        let user: HashSet<String> = self.db.settings.user_words.iter().map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect();
        let spell_cache = &mut self.spell_cache;
        let mut add_word: Option<String> = None;
        let Some(ed) = &mut self.editor else { return };
        let mut save = false;
        let mut cancel = false;
        egui::ScrollArea::vertical().id_salt("editor").auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(if ed.is_new { "Neue Aufgabe" } else { "Aufgabe bearbeiten" });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("💾  Speichern").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                        save = true;
                    }
                    if ui.button("Abbrechen").clicked() {
                        cancel = true;
                    }
                });
            });
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("meta").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                    let t = &mut ed.task;
                    ui.label("Fach");
                    ui.horizontal(|ui| {
                        ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut t.subject));
                        egui::ComboBox::from_id_salt("subj").width(20.0).selected_text("").show_ui(ui, |ui| {
                            for s in &subjects {
                                ui.selectable_value(&mut t.subject, s.clone(), s);
                            }
                        });
                    });
                    ui.label("Punkte");
                    if t.parts.is_empty() {
                        ui.add(egui::DragValue::new(&mut t.points).range(0..=200).suffix(" P."));
                    } else {
                        let sum: i32 = t.parts.iter().map(|p| p.points.max(0)).sum();
                        ui.label(RichText::new(format!("{sum} P.")).strong()).on_hover_text("Summe der Teilaufgaben");
                    }
                    ui.end_row();
                    ui.label("Schlagwort 1");
                    ui.horizontal(|ui| {
                        ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut t.topic));
                        egui::ComboBox::from_id_salt("topic").width(20.0).selected_text("").show_ui(ui, |ui| {
                            for s in &topics {
                                ui.selectable_value(&mut t.topic, s.clone(), s);
                            }
                        });
                    });
                    ui.label("Bearbeitungszeit");
                    ui.add(egui::DragValue::new(&mut t.minutes).range(0..=600).suffix(" Min."));
                    ui.end_row();
                    ui.label("Schlagwort 2");
                    ui.add_sized([190.0, 22.0], egui::TextEdit::singleline(&mut t.subtopic));
                    ui.label("Schwierigkeit");
                    ui.horizontal(|ui| {
                        for (i, l) in ["–", "leicht", "mittel", "schwer"].iter().enumerate() {
                            ui.selectable_value(&mut t.difficulty, i as u8, *l);
                        }
                    });
                    ui.end_row();
                    ui.label("Schlagwörter");
                    ui.add_sized([190.0, 22.0], egui::TextEdit::singleline(&mut ed.tags_text).hint_text("z. B. Abitur, Wiederholung"));
                    ui.label("Leerzeilen für Lösung");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut t.answer_lines).range(0..=60).suffix(" Zeilen"));
                        ui.checkbox(&mut t.underline_answer, "unterstreichen");
                    });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    ui.label("Schrift beim Druck");
                    let t = &mut ed.task;
                    let shown = if t.font.is_empty() { format!("Standard ({})", print_font) } else { t.font.clone() };
                    egui::ComboBox::from_id_salt("task_font").selected_text(shown).width(170.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut t.font, String::new(), format!("Standard ({})", print_font));
                        for f in PRINT_FONTS {
                            ui.selectable_value(&mut t.font, f.to_string(), f);
                        }
                    });
                    let mut own = t.font_size > 0.0;
                    if ui.checkbox(&mut own, "eigene Größe").changed() {
                        t.font_size = if own { print_size } else { 0.0 };
                    }
                    if own {
                        ui.add(egui::DragValue::new(&mut t.font_size).range(8.0..=28.0).speed(0.5).suffix(" pt"));
                    }
                    ui.label(RichText::new("nur für diese Aufgabe, z. B. größer für eine Förderklasse").small().weak());
                });
                ui.label(RichText::new("🔒 Interne Bemerkung (wird nie gedruckt)").small());
                ui.add(egui::TextEdit::multiline(&mut ed.task.notes).desired_rows(2).desired_width(f32::INFINITY));
            });
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                let t = &mut ed.task;
                ui.horizontal(|ui| {
                    section_label(ui, "TEILAUFGABEN & ERWARTUNGSHORIZONT", Color32::from_rgb(14, 116, 144));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("➕  Teilaufgabe").on_hover_text("Eigene Punkte je Teilaufgabe; die Aufgabe hat dann deren Summe").clicked() {
                            let label = t.next_part_label();
                            let points = if t.parts.is_empty() { t.points.max(1) } else { 1 };
                            t.parts.push(Part { label, points, expectation: String::new() });
                        }
                    });
                });
                if t.parts.is_empty() {
                    ui.label(
                        RichText::new(
                            "Keine Teilaufgaben. Mit Teilaufgaben (a, b, c) lassen sich Punkte je Teilaufgabe vergeben und erfassen und ein Erwartungshorizont drucken.",
                        )
                        .small()
                        .weak(),
                    );
                } else {
                    let mut remove = None;
                    let mut swap = None;
                    let n = t.parts.len();
                    egui::Grid::new("parts").num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.label(RichText::new("Teil").small().strong());
                        ui.label(RichText::new("Punkte").small().strong());
                        ui.label(RichText::new("Erwartete Leistung (Erwartungshorizont)").small().strong());
                        ui.label("");
                        ui.end_row();
                        for (i, p) in t.parts.iter_mut().enumerate() {
                            ui.add_sized([34.0, 22.0], egui::TextEdit::singleline(&mut p.label));
                            ui.add(egui::DragValue::new(&mut p.points).range(0..=200).suffix(" P."));
                            let w = (ui.available_width() - 90.0).max(200.0);
                            ui.add(egui::TextEdit::multiline(&mut p.expectation).desired_rows(1).desired_width(w).hint_text("z. B. Ansatz, Rechenweg und Ergebnis mit Einheit"));
                            ui.horizontal(|ui| {
                                if i > 0 && ui.small_button("▲").clicked() {
                                    swap = Some((i, i - 1));
                                }
                                if i + 1 < n && ui.small_button("▼").clicked() {
                                    swap = Some((i, i + 1));
                                }
                                if ui.small_button("✕").on_hover_text("Teilaufgabe entfernen").clicked() {
                                    remove = Some(i);
                                }
                            });
                            ui.end_row();
                        }
                    });
                    if let Some((a, b)) = swap {
                        t.parts.swap(a, b);
                    }
                    if let Some(i) = remove {
                        t.parts.remove(i);
                    }
                    t.sync_points();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button("⇡  Punkte in den Aufgabentext")
                            .on_hover_text("Schreibt die Punkte in die Zeilen „a) …“ bzw. „| a) | … | 2 P. |“ des Aufgabentexts; fehlende Teilaufgaben kommen als neue Zeilen dazu")
                            .clicked()
                        {
                            t.task.text = write_parts_into_text(&t.task.text, &t.parts);
                        }
                        ui.label(RichText::new("Den Text der Teilaufgaben schreiben Sie in die Aufgabe.").small().weak());
                    });
                }
                let found = parts_from_text(&t.task.text);
                let differs = !found.is_empty() && found.iter().map(|p| (&p.label, p.points)).ne(t.parts.iter().map(|p| (&p.label, p.points)));
                if differs {
                    ui.horizontal_wrapped(|ui| {
                        let list: Vec<String> = found.iter().map(|p| format!("{}) {} P.", p.label, p.points)).collect();
                        ui.label(RichText::new(format!("Im Aufgabentext erkannt: {}", list.join(", "))).small().color(WARN));
                        if ui.small_button("⇣  Übernehmen").on_hover_text("Teilaufgaben und Punkte aus dem Text übernehmen (Erwartungen bleiben erhalten)").clicked() {
                            let old = std::mem::take(&mut t.parts);
                            t.parts = found
                                .into_iter()
                                .map(|mut p| {
                                    if let Some(o) = old.iter().find(|o| o.label == p.label) {
                                        p.expectation = o.expectation.clone();
                                    }
                                    p
                                })
                                .collect();
                            t.sync_points();
                        }
                    });
                }
            });
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                let seed = &mut ed.seed;
                let t = &mut ed.task;
                ui.horizontal(|ui| {
                    section_label(ui, "ZUFALLSZAHLEN (AUFGABENVARIANTEN)", Color32::from_rgb(120, 80, 200));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("➕  Variable").on_hover_text("Eine Zahl, die bei jeder Klausur (und je Gruppe) neu gewählt wird").clicked() {
                            let used: Vec<String> = t.vars.iter().map(|v| v.name.clone()).collect();
                            let name = ["a", "b", "c", "d", "m", "n", "p", "q", "k", "x"].iter().find(|n| !used.contains(&n.to_string())).unwrap_or(&"z").to_string();
                            t.vars.push(crate::vary::Var { name, ..Default::default() });
                        }
                        if !t.vars.is_empty() && ui.button("🎲  Andere Beispielwerte").clicked() {
                            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        }
                    });
                });
                if t.vars.is_empty() {
                    ui.label(
                        RichText::new("Keine Zufallszahlen. Mit Variablen bekommt jede Klausur und jede Gruppe andere Zahlen; im Text steht dann z. B. {=a} oder {=2*a+b}.")
                            .small()
                            .weak(),
                    );
                } else {
                    let mut remove = None;
                    egui::Grid::new("vars").num_columns(6).spacing([8.0, 4.0]).show(ui, |ui| {
                        for l in ["Name", "von", "bis", "Schritt", "Werte", ""] {
                            ui.label(RichText::new(l).small().strong());
                        }
                        ui.end_row();
                        for (i, v) in t.vars.iter_mut().enumerate() {
                            ui.add_sized([46.0, 22.0], egui::TextEdit::singleline(&mut v.name));
                            v.name.retain(|c| c.is_alphanumeric() || c == '_');
                            ui.add(egui::DragValue::new(&mut v.min).speed(0.5).max_decimals(3));
                            ui.add(egui::DragValue::new(&mut v.max).speed(0.5).max_decimals(3));
                            ui.add(egui::DragValue::new(&mut v.step).range(0.001..=1000.0).speed(0.1).max_decimals(3));
                            let n = v.count();
                            ui.label(RichText::new(if n > 1 { format!("{n} mögliche") } else { "nur 1 Wert".into() }).small().color(if n > 1 { Color32::GRAY } else { WARN }));
                            if ui.small_button("✕").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(i) = remove {
                        t.vars.remove(i);
                    }
                    let vals = crate::vary::values(t, *seed);
                    let mut shown: Vec<String> = t.vars.iter().map(|v| format!("{} = {}", v.name, crate::vary::fmt_num(vals[&v.name], 6))).collect();
                    shown.dedup();
                    ui.label(RichText::new(format!("Beispiel in der Vorschau: {}", shown.join(", "))).small());
                }
                let all = format!("{}\n{}\n{}", t.task.text, t.solution.text, t.parts.iter().map(|p| p.expectation.as_str()).collect::<Vec<_>>().join("\n"));
                if !t.vars.is_empty() || all.contains("{=") {
                    let probs = crate::vary::problems(&all, &crate::vary::values(t, *seed));
                    if !probs.is_empty() {
                        ui.label(RichText::new(format!("Nicht berechenbar: {}", probs.join("; "))).small().color(WARN));
                    }
                    ui.label(
                        RichText::new("Im Text, in der Lösung und in Formeln: {=a} setzt den Wert ein, {=2*a+b} rechnet (+ − * / ^, sqrt, abs, round, sin/cos/tan in Grad), {=a/3;2} rundet auf 2 Stellen.")
                            .small()
                            .weak(),
                    );
                }
            });
            ui.add_space(8.0);
            let wide = ui.available_width() > 900.0;
            let mut editor_col = |ui: &mut egui::Ui, which: usize, cache: &mut ObjCache| {
                let (label, col) = if which == 0 { ("AUFGABE", ACCENT) } else { ("MUSTERLÖSUNG", GOOD) };
                section_label(ui, label, col);
                let id = egui::Id::new(("ed", which));
                let doc = if which == 0 { &mut ed.task.task } else { &mut ed.task.solution };
                let cur = &mut ed.cur[which];
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let mut open_formula = false;
                    if let Some(m) = App::toolbar(ui, &mut doc.text, cur, &mut doc.objects, id, &mut open_formula) {
                        *toast = Some((m, 0.0));
                    }
                    if open_formula {
                        // edit the $…$ formula under the cursor, or start a new one
                        ed.formula = Some(match dollar_at(&doc.text, cur.0) {
                            Some((a, b, src)) => FormulaDlg { which, src, edit: Some((a, b)), cur: (0, 0) },
                            None => FormulaDlg { which, src: String::new(), edit: None, cur: (0, 0) },
                        });
                    }
                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    let color = ui.visuals().override_text_color.unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
                    let mut layouter = |ui: &egui::Ui, text: &str, wrap: f32| {
                        let normal = egui::TextFormat { font_id: font.clone(), color, ..Default::default() };
                        let wrong = egui::TextFormat { underline: Stroke::new(1.5_f32, Color32::from_rgb(220, 40, 40)), ..normal.clone() };
                        let mut job = egui::text::LayoutJob::default();
                        let mut last = 0;
                        if let Some(d) = &spell {
                            let idx: Vec<usize> = text.char_indices().map(|(b, _)| b).chain(std::iter::once(text.len())).collect();
                            for (a, b) in crate::spell::words(text) {
                                let (ba, bb) = (idx[a], idx[b]);
                                if !spell_cache.ok(d, &user, &text[ba..bb]) {
                                    job.append(&text[last..ba], 0.0, normal.clone());
                                    job.append(&text[ba..bb], 0.0, wrong.clone());
                                    last = bb;
                                }
                            }
                        }
                        job.append(&text[last..], 0.0, normal);
                        job.wrap.max_width = wrap;
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let out = egui::TextEdit::multiline(&mut doc.text)
                        .id(id)
                        .desired_rows(12)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .layouter(&mut layouter)
                        .show(ui);
                    if out.response.secondary_clicked() {
                        ed.menu_word[which] = None;
                        ed.menu_syn[which] = None;
                        if let Some(pos) = out.response.interact_pointer_pos() {
                            let c = out.galley.cursor_from_pos(pos - out.galley_pos).ccursor.index;
                            if let Some((a, b, w)) = crate::spell::word_at(&doc.text, c) {
                                if let Some(d) = &spell {
                                    if !user.contains(&w) && !d.check(&w) {
                                        let sugg = d.suggest(&w, 6);
                                        ed.menu_word[which] = Some((a, b, w.clone(), sugg));
                                    }
                                }
                                if let Some(th) = &thes {
                                    ed.menu_syn[which] = Some((a, b, w.clone(), th.lookup(&w)));
                                }
                            }
                        }
                    }
                    if out.response.has_focus() {
                        ed.focus = which;
                    }
                    if let Some(cr) = out.cursor_range {
                        let a = cr.primary.ccursor.index;
                        let b = cr.secondary.ccursor.index;
                        *cur = (a.min(b), a.max(b));
                    }
                    let mut ctx_ins: Option<(&str, &str)> = None;
                    let mut replace: Option<(usize, usize, String, String)> = None;
                    let menu_word = &ed.menu_word[which];
                    let menu_syn = &ed.menu_syn[which];
                    out.response.context_menu(|ui| {
                        if let Some((a, b, w, meanings)) = menu_syn {
                            if meanings.is_empty() {
                                ui.label(RichText::new(format!("Keine Synonyme für „{w}“")).weak());
                            } else {
                                ui.menu_button(format!("📖  Synonyme für „{w}“"), |ui| {
                                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                                        for (k, (pos, syns)) in meanings.iter().take(12).enumerate() {
                                            if k > 0 {
                                                ui.separator();
                                            }
                                            if !pos.is_empty() {
                                                ui.label(RichText::new(pos).small().weak());
                                            }
                                            for sg in syns.iter().take(12) {
                                                if ui.button(sg).clicked() {
                                                    replace = Some((*a, *b, w.clone(), crate::thesaurus::match_case(w, sg)));
                                                    ui.close_menu();
                                                }
                                            }
                                        }
                                    });
                                });
                            }
                            ui.separator();
                        }
                        if let Some((a, b, w, sugg)) = menu_word {
                            ui.label(RichText::new(format!("„{w}“ ist unbekannt")).strong());
                            if sugg.is_empty() {
                                ui.label(RichText::new("Keine Vorschläge").weak());
                            }
                            for sg in sugg {
                                if ui.button(RichText::new(sg).strong()).clicked() {
                                    replace = Some((*a, *b, w.clone(), sg.clone()));
                                    ui.close_menu();
                                }
                            }
                            if ui.button("➕  Ins eigene Wörterbuch").on_hover_text("Das Wort gilt dann immer als richtig (Einstellungen → Darstellung)").clicked() {
                                add_word = Some(w.clone());
                                ui.close_menu();
                            }
                            ui.separator();
                        }
                        for (l, a, b) in [
                            ("Fett", "**", "**"),
                            ("Hochgestellt", "^{", "}"),
                            ("Ankreuzkästchen", "[ ] ", ""),
                            ("Unteraufgaben a) b)", "\n| a) | Teilaufgabe | 2 P. |\n| b) | Teilaufgabe | 3 P. |\n", ""),
                            ("Tabelle", "\n+| Spalte 1 | Spalte 2 |\n+| … | … |\n", ""),
                            ("Leerzeilen", "\n[[Linien:4]]\n", ""),
                            ("Trennlinie", "\n---\n", ""),
                        ] {
                            if ui.button(l).clicked() {
                                ctx_ins = Some((a, b));
                                ui.close_menu();
                            }
                        }
                    });
                    if let Some((a, b)) = ctx_ins {
                        insert_wrap(&mut doc.text, cur, a, b);
                    }
                    if let Some((a, b, w, new)) = replace {
                        let (ba, bb) = (char_to_byte(&doc.text, a), char_to_byte(&doc.text, b));
                        if doc.text.get(ba..bb) == Some(w.as_str()) {
                            doc.text.replace_range(ba..bb, &new);
                        }
                        ed.menu_word[which] = None;
                        ed.menu_syn[which] = None;
                    }
                });
                ui.label(RichText::new("Vorschau").small().weak());
                let size = if ed.task.font_size > 0.0 { size * ed.task.font_size / print_size.max(1.0) } else { size };
                let doc = ed.preview(which);
                paper(ui, |ui| {
                    ui.set_width(ui.available_width());
                    View { doc, cache, size, zoom: 1.0 }.show(ui);
                });
            };
            if wide {
                ui.columns(2, |cols| {
                    editor_col(&mut cols[0], 0, cache);
                    editor_col(&mut cols[1], 1, cache);
                });
            } else {
                editor_col(ui, 0, cache);
                ui.add_space(10.0);
                editor_col(ui, 1, cache);
            }
        });
        // ---- formula editor
        let mut close_formula = false;
        if let Some(fd) = &mut ed.formula {
            let ctx = ui.ctx().clone();
            let mut apply = false;
            egui::Window::new(if fd.edit.is_some() { "Formel bearbeiten" } else { "Neue Formel" })
                .collapsible(false)
                .resizable(true)
                .default_width(620.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    let src_id = egui::Id::new("formula_src");
                    let mut ins: Option<(&str, &str)> = None;
                    let tb = |ui: &mut egui::Ui, label: &str, tip: &str| ui.add(egui::Button::new(RichText::new(label).size(15.0)).min_size(Vec2::new(30.0, 28.0))).on_hover_text(tip).clicked();
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (label, pre, post, tip) in [
                            ("a⁄b", "\\frac{", "}{}", "Bruch: \\frac{Zähler}{Nenner}"),
                            ("xⁿ", "^{", "}", "Hochgestellt / Potenz: x^{2}"),
                            ("xₙ", "_{", "}", "Tiefgestellt / Index: x_{1}"),
                            ("√", "\\sqrt{", "}", "Wurzel: \\sqrt{x}"),
                            ("ⁿ√", "\\sqrt[3]{", "}", "n-te Wurzel: \\sqrt[3]{x}"),
                            ("( )", "\\left( ", " \\right)", "Klammern, die mit dem Inhalt wachsen"),
                            ("[ ]", "\\left[ ", " \\right]", "Eckige Klammern, die mit dem Inhalt wachsen"),
                            ("| |", "\\left| ", " \\right|", "Betragsstriche"),
                            ("∫", "\\int_{a}^{b} ", " \\, dx", "Integral mit Grenzen"),
                            ("∑", "\\sum_{i=1}^{n} ", "", "Summe"),
                            ("lim", "\\lim_{x \\to \\infty} ", "", "Grenzwert"),
                            ("v⃗", "\\vec{", "}", "Vektorpfeil"),
                            ("x̄", "\\overline{", "}", "Überstrich"),
                            ("Text", "\\text{", "}", "Normaler Text in der Formel, z. B. \\text{Umsatz}"),
                            ("(⋮⋮)", "\\begin{pmatrix} ", " & 0 \\\\ 0 & 1 \\end{pmatrix}", "Matrix oder Vektor: Spalten mit &, Zeilen mit \\\\\n(bmatrix = eckige Klammern, vmatrix = Determinante)"),
                            ("{ ⋮", "\\begin{cases} ", " & \\text{für } x \\ge 0 \\\\ -x & \\text{für } x < 0 \\end{cases}", "Fallunterscheidung: Wert & Bedingung, neue Zeile mit \\\\"),
                            ("≡⇣", "", " \\\\ &= ", "Neue Zeile einer Rechnung: \\\\ beginnt eine Zeile, & markiert die Stelle,\nan der die Zeilen ausgerichtet werden (meist vor dem =)"),
                        ] {
                            if tb(ui, label, tip) {
                                ins = Some((pre, post));
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (label, code) in [
                            ("·", "\\cdot "), ("×", "\\times "), ("±", "\\pm "), ("≤", "\\le "), ("≥", "\\ge "), ("≠", "\\ne "), ("≈", "\\approx "),
                            ("∞", "\\infty "), ("→", "\\to "), ("⇒", "\\Rightarrow "), ("⇔", "\\Leftrightarrow "), ("∈", "\\in "), ("ℝ", "\\mathbb{R} "),
                            ("Δ", "\\Delta "), ("°", "\\degree "),
                        ] {
                            if tb(ui, label, code.trim()) {
                                ins = Some((code, ""));
                            }
                        }
                        ui.menu_button(RichText::new("αβγ").size(15.0), |ui| {
                            ui.set_max_width(300.0);
                            ui.horizontal_wrapped(|ui| {
                                for (c, code) in [
                                    ("α", "\\alpha "), ("β", "\\beta "), ("γ", "\\gamma "), ("δ", "\\delta "), ("ε", "\\varepsilon "), ("η", "\\eta "),
                                    ("θ", "\\theta "), ("λ", "\\lambda "), ("μ", "\\mu "), ("π", "\\pi "), ("ρ", "\\rho "), ("σ", "\\sigma "),
                                    ("τ", "\\tau "), ("φ", "\\varphi "), ("ω", "\\omega "), ("Γ", "\\Gamma "), ("Δ", "\\Delta "), ("Θ", "\\Theta "),
                                    ("Λ", "\\Lambda "), ("Π", "\\Pi "), ("Σ", "\\Sigma "), ("Φ", "\\Phi "), ("Ω", "\\Omega "),
                                ] {
                                    if ui.add(egui::Button::new(RichText::new(c).size(16.0)).min_size(Vec2::splat(28.0))).on_hover_text(code.trim()).clicked() {
                                        ins = Some((code, ""));
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                    });
                    let out = egui::TextEdit::multiline(&mut fd.src)
                        .id(src_id)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("z. B.  x_{1,2} = -\\frac{p}{2} \\pm \\sqrt{\\left(\\frac{p}{2}\\right)^2 - q}")
                        .show(ui);
                    if let Some(cr) = out.cursor_range {
                        let a = cr.primary.ccursor.index;
                        let b = cr.secondary.ccursor.index;
                        fd.cur = (a.min(b), a.max(b));
                    }
                    if let Some((a, b)) = ins {
                        let pos = insert_wrap(&mut fd.src, &mut fd.cur, a, b);
                        if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), src_id) {
                            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(pos))));
                            st.store(ui.ctx(), src_id);
                        }
                        ui.ctx().memory_mut(|m| m.request_focus(src_id));
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new("Vorschau").small().weak());
                    let node = crate::math::parse(&fd.src);
                    let d = crate::math::layout_node(&node);
                    let (w, h) = crate::math::size_px(&d);
                    let scale = (2.0f32).min((ui.available_width() - 24.0) / w.max(1.0));
                    paper(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(70.0);
                        ui.vertical_centered(|ui| {
                            if fd.src.trim().is_empty() {
                                ui.label(RichText::new("Hier erscheint die Formel.").color(Color32::GRAY));
                            } else {
                                let (r, _) = ui.allocate_exact_size(Vec2::new(w * scale, h * scale), egui::Sense::hover());
                                crate::formula::paint(&d, ui.painter(), r, Some(markup::INK));
                            }
                        });
                    });
                    let errs = crate::math::errors(&node);
                    if !errs.is_empty() {
                        ui.label(RichText::new(format!("Nicht erkannt (rot dargestellt): {}", errs.join("  "))).color(WARN));
                    }
                    ui.label(
                        RichText::new(
                            "Schreibweise wie LaTeX: x^2, x_1, \\frac{a}{b}, \\sqrt{x}, \\int_a^b, \\sum_{i=1}^n, \\lim_{x \\to 0}, \\alpha, \\cdot, \\le, \\Rightarrow. \
                             Mehrere Zeilen: \\\\ trennt Zeilen, & richtet sie aus (x &= 2+3 \\\\ &= 5). \
                             Leerzeichen werden ignoriert; Abstand mit \\, oder \\quad. sin, cos, ln, lim werden automatisch aufrecht gesetzt.",
                        )
                        .small()
                        .weak(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let label = if fd.edit.is_some() { "✔  Übernehmen" } else { "✔  Einfügen" };
                        let ok = ui.add_enabled(!fd.src.trim().is_empty(), egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(ACCENT)).clicked();
                        let ctrl_enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
                        if (ok || ctrl_enter) && !fd.src.trim().is_empty() {
                            apply = true;
                        }
                        if ui.button("Abbrechen").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            close_formula = true;
                        }
                        ui.label(RichText::new("Strg+Enter = Einfügen").small().weak());
                    });
                });
            if apply {
                // the formula goes into the text as $…$ (one line; spaces do not matter in formulas)
                let src = fd.src.split_whitespace().collect::<Vec<_>>().join(" ");
                let which = fd.which.min(1);
                let edit = fd.edit;
                let doc = if which == 0 { &mut ed.task.task } else { &mut ed.task.solution };
                let code = format!("${src}$");
                match edit {
                    Some((a, b)) => {
                        let (ba, bb) = (char_to_byte(&doc.text, a), char_to_byte(&doc.text, b));
                        doc.text.replace_range(ba..bb, &code);
                        let end = a + code.chars().count();
                        ed.cur[which] = (end, end);
                    }
                    None => {
                        // a space between the formula and a word right before it
                        let n = doc.text.chars().count();
                        let at = ed.cur[which].0.min(n);
                        let before = doc.text.chars().nth(at.wrapping_sub(1)).filter(|_| at > 0);
                        let pre = if before.map(|c| c.is_alphanumeric()).unwrap_or(false) { " " } else { "" };
                        insert_wrap(&mut doc.text, &mut ed.cur[which], &format!("{pre}{code}"), "");
                    }
                }
                close_formula = true;
            }
        }
        if close_formula {
            ed.formula = None;
        }
        if let Some(w) = add_word {
            if !self.db.settings.user_words.contains(&w) {
                self.db.settings.user_words.push(w);
                self.changed();
            }
            if let Some(ed) = &mut self.editor {
                ed.menu_word = [None, None];
                ed.menu_syn = [None, None];
            }
        }
        if cancel {
            self.editor = None;
        }
        if save {
            let Some(mut ed) = self.editor.take() else { return };
            ed.task.tags = ed.tags_text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            ed.task.updated = Some(now());
            ed.task.author_upd = self.db.settings.teacher.clone();
            ed.task.changed = true;
            let task = ed.finished();
            let id = task.id;
            if let Some(t) = self.db.task_mut(id) {
                *t = task;
            } else {
                self.db.tasks.push(task);
            }
            self.selected = Some(id);
            self.changed();
            self.toast("Aufgabe gespeichert – die Änderung gilt auch in allen Klausuren, die sie enthalten.");
        }
    }
}
