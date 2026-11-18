//! The program window: task pool, editor, exam composer with archive,
//! grade overview, statistics and settings.

mod classes;
mod misc;
mod pool;
mod sheets;

use crate::export;
use crate::markup::{ObjCache, PAPER};
use crate::model::*;
use egui::{Align, Color32, CornerRadius, Layout, RichText, Stroke, Vec2};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

pub(crate) const ACCENT: Color32 = Color32::from_rgb(79, 70, 229);
pub(crate) const GOOD: Color32 = Color32::from_rgb(22, 150, 90);
pub(crate) const WARN: Color32 = Color32::from_rgb(214, 120, 0);

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Pool,
    Sheets,
    Stats,
    Classes,
}

#[derive(PartialEq, Clone, Copy)]
enum SheetView {
    Edit,
    Preview,
    Solution,
    Results,
}

#[derive(PartialEq, Clone, Copy)]
enum TaskSort {
    Subject,
    Title,
    Points,
    LastUsed,
    Newest,
}

struct Editor {
    task: Task,
    is_new: bool,
    cur: [(usize, usize); 2],
    tags_text: String,
    /// editor that had the keyboard focus last (0 = Aufgabe, 1 = Lösung);
    /// dropped image files go there
    focus: usize,
    /// open formula editor
    formula: Option<FormulaDlg>,
    /// preview of task and solution: (what it was made from, document with formula objects)
    preview: [Option<(String, Doc)>; 2],
    /// example numbers for the random variables in the preview
    seed: u64,
    /// word under the right-click menu: (start, end, word, suggestions)
    menu_word: [Option<MenuWord>; 2],
    /// word under the right-click menu with its synonyms: (start, end, word, meanings)
    menu_syn: [Option<SynWord>; 2],
}

/// An unknown word in the editor: (start, end) in chars, the word, suggestions.
type MenuWord = (usize, usize, String, Vec<String>);
type SynWord = (usize, usize, String, Vec<crate::thesaurus::Meaning>);

impl Editor {
    /// Opens a task for editing. Formulas of the formula editor appear in the text
    /// as `$…$`; pictures and old formulas stay `{{n}}` (see `Doc::to_dollar_text`).
    fn new(mut task: Task, is_new: bool) -> Self {
        for d in [&mut task.task, &mut task.solution] {
            let (text, objects) = d.to_dollar_text();
            *d = Doc { text, objects };
        }
        Editor {
            tags_text: task.tags.join(", "),
            task,
            is_new,
            cur: [(0, 0); 2],
            focus: 0,
            formula: None,
            preview: [None, None],
            seed: 0,
            menu_word: [None, None],
            menu_syn: [None, None],
        }
    }
    /// Task and solution with their `$…$` turned into formula objects again.
    fn finished(mut self) -> Task {
        for d in [&mut self.task.task, &mut self.task.solution] {
            *d = Doc::from_dollar_text(&d.text, &d.objects);
            d.prune();
        }
        self.task.sync_points();
        self.task
    }
    /// The document shown as preview (rebuilt when the text, the objects or the random numbers changed).
    fn preview(&mut self, which: usize) -> &Doc {
        let doc = if which == 0 { &self.task.task } else { &self.task.solution };
        let key = format!("{}\u{0}{}\u{0}{:?}\u{0}{}", doc.text, doc.objects.len(), self.task.vars, self.seed);
        let fresh = matches!(&self.preview[which], Some((k, _)) if *k == key);
        if !fresh {
            let mut d = Doc::from_dollar_text(&doc.text, &doc.objects);
            if !self.task.vars.is_empty() || doc.text.contains("{=") {
                d = crate::vary::apply_doc(&d, &crate::vary::values(&self.task, self.seed));
            }
            self.preview[which] = Some((key, d));
        }
        &self.preview[which].as_ref().unwrap().1
    }
}

/// The formula editor dialog: which text it belongs to and the formula source.
struct FormulaDlg {
    which: usize,
    src: String,
    /// the `$…$` being changed, as char range in the editor text (None = new formula)
    edit: Option<(usize, usize)>,
    cur: (usize, usize),
}

/// "📄  Word" button with a small menu: DOCX or RTF. Returns Some(rtf?) when chosen.
pub(crate) fn word_menu(ui: &mut egui::Ui, label: &str) -> Option<bool> {
    let mut out = None;
    ui.menu_button(label, |ui| {
        if ui
            .button("Word-Dokument (.docx)")
            .on_hover_text("Für Word, LibreOffice, Google Docs – Formeln aus dem Formeleditor bleiben in Word bearbeitbar")
            .clicked()
        {
            out = Some(false);
            ui.close_menu();
        }
        if ui.button("RTF (ältere Programme)").on_hover_text("Rich Text Format – Formeln als Bilder").clicked() {
            out = Some(true);
            ui.close_menu();
        }
    });
    out
}

enum Modal {
    None,
    DeleteTasks(Vec<u64>),
    DeleteSheet(u64),
    DeleteCourse(u64),
    Rename { field: usize, from: String, to: String },
    Settings,
    Help,
    Update,
    Restore,
    Exchange,
    WordImport,
}

/// A Word or RTF document being imported: the file, the options and what was found.
pub struct WordImportState {
    pub path: PathBuf,
    pub parsed: Result<crate::wordimport::Parsed, String>,
    pub opt: crate::wordimport::Options,
    pub found: Vec<crate::wordimport::Found>,
    pub notes: Vec<String>,
    /// numbers of the tasks to take over
    pub take: HashSet<usize>,
    pub preview: Option<usize>,
}

/// An opened backup, from which single entries can be taken over.
pub struct RestoreState {
    pub src: Database,
    /// where it comes from, e.g. "Automatische Sicherung vom 17.11.2026, 14:05"
    pub label: String,
    pub tasks: HashSet<u64>,
    pub sheets: HashSet<u64>,
    pub courses: HashSet<u64>,
    /// show only entries that are missing now or differ
    pub only_diff: bool,
    pub tab: u8,
}

struct Shot {
    path: PathBuf,
    frames: u32,
    requested: bool,
}

enum Act {
    Select(u64),
    Edit(u64),
    NewTask,
    Duplicate(u64),
    AskDelete(Vec<u64>),
    ToggleFav(u64),
    AddToSheet(u64, u64),
    AddToNewSheet(u64),
    PrintTask(u64),
    /// task as Word file; true = RTF, false = DOCX
    WordTask(u64, bool),
    SelectSheet(u64),
    OpenSheet(u64, SheetView),
    DuplicateSheet(u64),
    /// parallel version (Gruppe A/B): sheet, replace tasks, shuffle order
    ParallelSheet(u64, ParallelOptions),
    ArchiveSheet(u64),
    PrintSheet(u64, bool),
    NewSheet(String),
}

pub struct App {
    db: Database,
    persist: bool,
    dirty: bool,
    dirty_at: f64,
    status: String,
    tab: Tab,
    search: String,
    f_subject: Option<String>,
    f_topic: Option<String>,
    f_tag: Option<String>,
    only_fav: bool,
    only_unused: bool,
    sort: TaskSort,
    selected: Option<u64>,
    marked: HashSet<u64>,
    editor: Option<Editor>,
    cache: ObjCache,
    sheet_sel: Option<u64>,
    sheet_view: SheetView,
    arch_search: String,
    arch_subject: String,
    arch_type: String,
    arch_archived: bool,
    arch_sort: u8,
    arch_desc: bool,
    picker_search: String,
    picker_subject: String,
    rnd_target: i32,
    par_replace: bool,
    par_shuffle: bool,
    par_tolerance: i32,
    course_sel: Option<u64>,
    course_archived: bool,
    /// Notenbuch: 0 = school year, 1 / 2 = Halbjahr
    course_term: u8,
    /// student whose grade development is shown
    course_chart: Option<u64>,
    /// spell checking: the loaded dictionary, the loader running in the background, a status line
    spell: Option<std::sync::Arc<crate::spell::Dict>>,
    spell_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Result<crate::spell::Dict, String>)>>,
    spell_status: String,
    spell_cache: crate::spell::Cache,
    /// thesaurus: loaded file, loader in the background, status line
    thes: Option<std::sync::Arc<crate::thesaurus::Thesaurus>>,
    thes_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Result<crate::thesaurus::Thesaurus, String>)>>,
    thes_status: String,
    /// updater: a check or download running in the background, the newer version found, a status line
    update_rx: Option<std::sync::mpsc::Receiver<(bool, crate::update::Check)>>,
    download_rx: Option<std::sync::mpsc::Receiver<Result<PathBuf, String>>>,
    update: Option<(crate::update::Release, Option<crate::update::Asset>)>,
    update_status: String,
    /// opened backup for taking over single entries
    restore: Option<RestoreState>,
    word_import: Option<WordImportState>,
    /// packages in the exchange folder: (read at, folder, list)
    exchange_list: Option<(f64, String, Vec<misc::ExchangeFile>)>,
    names_paste: String,
    /// data file time of our last load/save; a different time means the MCP server wrote it
    file_stamp: Option<std::time::SystemTime>,
    last_poll: f64,
    known_external: HashSet<u64>,
    rnd_seed: u64,
    quick_points: String,
    modal: Modal,
    settings_tab: u8,
    scheme_sel: usize,
    template_sel: usize,
    toast: Option<(String, f64)>,
    shot: Option<Shot>,
    logo_tex: Option<(usize, egui::TextureHandle)>,
    /// the program's own symbol (same as the .exe and the taskbar), shown next to the name
    app_logo: Option<egui::TextureHandle>,
    preview_zoom: f32,
}

fn now() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.selection.bg_fill = if dark { Color32::from_rgb(70, 64, 190) } else { Color32::from_rgb(214, 212, 252) };
    v.selection.stroke = Stroke::new(1.0_f32, if dark { Color32::WHITE } else { ACCENT });
    v.hyperlink_color = if dark { Color32::from_rgb(150, 160, 255) } else { ACCENT };
    v.panel_fill = if dark { Color32::from_rgb(24, 25, 31) } else { Color32::from_rgb(242, 243, 247) };
    v.window_fill = if dark { Color32::from_rgb(33, 34, 42) } else { Color32::WHITE };
    v.extreme_bg_color = if dark { Color32::from_rgb(17, 18, 22) } else { Color32::WHITE };
    v.faint_bg_color = if dark { Color32::from_rgb(38, 39, 48) } else { Color32::from_rgb(247, 247, 251) };
    let r = CornerRadius::same(7);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    if !dark {
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(232, 233, 240);
        v.widgets.inactive.bg_fill = Color32::from_rgb(232, 233, 240);
        // a thin frame, so input fields are visible on white windows and cards
        v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(214, 216, 226));
    }
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::new(21.0, crate::fonts::family(false, false, true)));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
}

fn card_fill(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(33, 34, 42)
    } else {
        Color32::WHITE
    }
}

fn border(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(52, 54, 66)
    } else {
        Color32::from_rgb(222, 224, 232)
    }
}

fn sel_fill(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(48, 46, 92)
    } else {
        Color32::from_rgb(236, 235, 255)
    }
}

fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new().fill(card_fill(ui)).stroke(Stroke::new(1.0_f32, border(ui))).corner_radius(10).inner_margin(12).show(ui, add)
}

fn paper<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(PAPER)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(214, 216, 224)))
        .corner_radius(6)
        .shadow(egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(22) })
        .inner_margin(egui::Margin::symmetric(22, 16))
        .show(ui, add)
}

fn chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    if text.is_empty() {
        return;
    }
    let bg = color.gamma_multiply(if ui.visuals().dark_mode { 0.35 } else { 0.14 });
    let fg = if ui.visuals().dark_mode { color.gamma_multiply(1.6) } else { color };
    egui::Frame::new().fill(bg).corner_radius(10).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
        ui.label(RichText::new(text).size(12.0).color(fg));
    });
}

fn section_label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).size(11.5).strong().color(color).extra_letter_spacing(1.2));
}

fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

fn insert_wrap(text: &mut String, cur: &mut (usize, usize), pre: &str, post: &str) -> usize {
    let n = text.chars().count();
    let a = cur.0.min(n);
    let b = cur.1.min(n).max(a);
    let ba = char_to_byte(text, a);
    let bb = char_to_byte(text, b);
    let sel = text[ba..bb].to_string();
    let ins = format!("{pre}{sel}{post}");
    text.replace_range(ba..bb, &ins);
    let pos = a + pre.chars().count() + sel.chars().count();
    let end = pos + post.chars().count();
    *cur = (end, end);
    if sel.is_empty() {
        pos
    } else {
        end
    }
}

fn optimize_image(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width(), img.height());
    let is_jpeg = bytes.starts_with(b"\xFF\xD8");
    let is_png = bytes.starts_with(b"\x89PNG");
    let is_gif = bytes.starts_with(b"GIF8");
    if w <= 1200 && bytes.len() < 400_000 && (is_jpeg || is_png || is_gif) {
        return Some((bytes.to_vec(), w, h));
    }
    let img = if w > 1200 { img.resize(1200, (h as f32 * 1200.0 / w as f32) as u32, image::imageops::FilterType::Lanczos3) } else { img };
    let mut out = Vec::new();
    if is_jpeg {
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85);
        img.to_rgb8().write_with_encoder(enc).ok()?;
    } else {
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    }
    if out.len() >= bytes.len() && w <= 1200 {
        return Some((bytes.to_vec(), w, h));
    }
    Some((out, img.width(), img.height()))
}

/// Adds a picture to a document's objects and returns the markup that shows it
/// (`{{n}}`). Big pictures are scaled down for display (max. 560 px wide).
pub(crate) fn add_image_object(objects: &mut Vec<DocObject>, bytes: &[u8]) -> Option<String> {
    let (b, w, h) = optimize_image(bytes)?;
    let oid = objects.iter().map(|o| o.id).max().unwrap_or(0) + 1;
    let (dw, dh) = if w > 560 { (560.0, h as f32 * 560.0 / w as f32) } else { (w as f32, h as f32) };
    objects.push(DocObject { id: oid, kind: ObjKind::Image, data: b, width: dw, height: dh });
    Some(format!("{{{{{oid}}}}}"))
}

/// The picture on the clipboard (e.g. a screenshot or a copied sketch) as PNG bytes.
pub(crate) fn clipboard_png() -> Result<Vec<u8>, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Zwischenablage nicht verfügbar: {e}"))?;
    let img = cb.get_image().map_err(|_| "In der Zwischenablage ist kein Bild.".to_string())?;
    let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
        .ok_or_else(|| "Das Bild in der Zwischenablage hat ein unbekanntes Format.".to_string())?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(rgba).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

pub(crate) fn is_image_path(p: &std::path::Path) -> bool {
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp")
}

impl App {
    pub fn new(ctx: &egui::Context, shot: Option<PathBuf>) -> App {
        let (db, persist) = if shot.is_some() {
            (Database::with_samples(), false)
        } else {
            match Database::load() {
                Some(mut db) => {
                    if db.settings.schemes.is_empty() {
                        db.settings.schemes = GradeScheme::defaults();
                    }
                    crate::model::migrate_schemes(&mut db);
                    if db.settings.templates.is_empty() {
                        db.settings.templates = Template::defaults();
                    }
                    (db, true)
                }
                None => {
                    let db = Database::with_samples();
                    let _ = db.save();
                    (db, true)
                }
            }
        };
        apply_theme(ctx, db.settings.dark);
        let mut app = App {
            db,
            persist,
            dirty: false,
            dirty_at: 0.0,
            status: String::new(),
            tab: Tab::Pool,
            search: String::new(),
            f_subject: None,
            f_topic: None,
            f_tag: None,
            only_fav: false,
            only_unused: false,
            sort: TaskSort::Subject,
            selected: None,
            marked: HashSet::new(),
            editor: None,
            cache: ObjCache::default(),
            sheet_sel: None,
            sheet_view: SheetView::Edit,
            arch_search: String::new(),
            arch_subject: String::new(),
            arch_type: String::new(),
            arch_archived: false,
            arch_sort: 0,
            arch_desc: true,
            picker_search: String::new(),
            picker_subject: String::new(),
            rnd_target: 60,
            par_replace: true,
            par_shuffle: true,
            par_tolerance: 2,
            course_sel: None,
            course_archived: false,
            course_term: 0,
            course_chart: None,
            spell: None,
            spell_rx: None,
            spell_status: String::new(),
            thes: None,
            thes_rx: None,
            thes_status: String::new(),
            spell_cache: crate::spell::Cache::default(),
            update_rx: None,
            download_rx: None,
            update: None,
            update_status: String::new(),
            restore: None,
            word_import: None,
            exchange_list: None,
            names_paste: String::new(),
            file_stamp: Database::file_stamp(),
            last_poll: 0.0,
            known_external: HashSet::new(),
            rnd_seed: (chrono::Local::now().timestamp_nanos_opt().unwrap_or(1) as u64) | 1,
            quick_points: String::new(),
            modal: Modal::None,
            settings_tab: 0,
            scheme_sel: 0,
            template_sel: 0,
            toast: None,
            shot: shot.map(|p| Shot { path: p, frames: 0, requested: false }),
            logo_tex: None,
            app_logo: None,
            preview_zoom: 1.0,
        };
        let list = app.filtered();
        app.selected = list
            .iter()
            .copied()
            .find(|id| app.db.task(*id).map(|t| t.subject == "Mathematik" && !t.task.objects.is_empty()).unwrap_or(false))
            .or_else(|| list.first().copied());
        // screenshot helpers (only without a data file, i.e. with --screenshot)
        if !app.persist && std::env::var("KLASSENWERK_DEMO").is_ok() {
            app.demo_class();
        }
        if !app.persist && std::env::var("KLASSENWERK_DARK").is_ok() {
            app.db.settings.dark = true;
            apply_theme(ctx, true);
        }
        if !app.persist {
            let open = std::env::var("KLASSENWERK_OPEN").unwrap_or_default();
            let (what, arg) = open.split_once(':').unwrap_or((open.as_str(), ""));
            let n: u64 = arg.parse().unwrap_or(0);
            match what {
                "settings" => {
                    app.settings_tab = n as u8;
                    app.modal = Modal::Settings;
                }
                "update" => {
                    // the dialog for a new version, with this version's notes from the changelog
                    let notes = include_str!("../../CHANGELOG.md").split("\n## ").nth(1).unwrap_or("").split_once('\n').map(|x| x.1).unwrap_or("").to_string();
                    let rel = crate::update::Release {
                        tag_name: format!("v{}", env!("CARGO_PKG_VERSION")),
                        body: notes,
                        assets: vec![crate::update::Asset { name: "Klassenwerk-windows-setup.exe".into(), size: 9_000_000, ..Default::default() }],
                        ..Default::default()
                    };
                    let asset = rel.assets.first().cloned();
                    app.update = Some((rel, asset));
                    app.modal = Modal::Update;
                }
                "chart" => {
                    // Notenbuch of a Halbjahr (0, 1, 2) with the grade chart of the first student
                    app.tab = Tab::Classes;
                    app.course_term = n as u8;
                    app.course_sel = app.db.courses.first().map(|c| c.id);
                    app.course_chart = app.db.courses.first().and_then(|c| c.students.first()).map(|s| s.id);
                }
                "restore" => {
                    // a backup that still has tasks deleted since, one task changed
                    let src = app.db.clone();
                    let gone: Vec<u64> = app.db.tasks.iter().rev().take(3).map(|t| t.id).collect();
                    app.db.tasks.retain(|t| !gone.contains(&t.id));
                    if let Some(t) = app.db.tasks.first_mut() {
                        t.points += 1;
                    }
                    app.restore = Some(RestoreState {
                        src,
                        label: "Automatische Sicherung vom 16.11.2026, 18:42".into(),
                        tasks: gone.into_iter().take(2).collect(),
                        sheets: HashSet::new(),
                        courses: HashSet::new(),
                        only_diff: true,
                        tab: 0,
                    });
                    app.modal = Modal::Restore;
                }
                "word" => {
                    if let Ok(p) = std::env::var("KLASSENWERK_WORD") {
                        app.open_word_import(PathBuf::from(p));
                        if let Some(st) = &mut app.word_import {
                            st.preview = Some(1.min(st.found.len().saturating_sub(1)));
                        }
                    }
                }
                "exchange" => {
                    app.db.settings.exchange_dir = std::env::var("KLASSENWERK_EXCHANGE").unwrap_or_default();
                    app.modal = Modal::Exchange;
                }
                "table" => {
                    // a task with merged cells and a single framed cell, open in the editor
                    let n = if n == 0 { app.db.tasks.first().map(|t| t.id).unwrap_or(0) } else { n };
                    if let Some(mut t) = app.db.task(n).cloned() {
                        t.task.text = "Ergänze die Wertetabelle der Funktion f(x) = 2x + 1.\n\n+| **Wertetabelle** | < | < | < |\n+| x | 0 | 1 | 2 |\n+| f(x) | | | |\n\n| Begründe kurz: | ! |\n| ^ | ^ |".into();
                        app.selected = Some(n);
                        app.editor = Some(Editor::new(t, false));
                    }
                }
                "edit" | "formula" => {
                    // "edit:0": the newest task
                    let n = if n == 0 { app.db.tasks.last().map(|t| t.id).unwrap_or(0) } else { n };
                    if let Some(t) = app.db.task(n).cloned() {
                        app.selected = Some(n);
                        let formula = (what == "formula").then(|| FormulaDlg {
                            which: 1,
                            src: std::env::var("KLASSENWERK_FORMULA")
                                .unwrap_or_else(|_| r"x_{1,2} = -\frac{p}{2} \pm \sqrt{\left(\frac{p}{2}\right)^2 - q}".into()),
                            edit: None,
                            cur: (0, 0),
                        });
                        let mut ed = Editor::new(t, false);
                        ed.formula = formula;
                        app.editor = Some(ed);
                    }
                }
                _ => {}
            }
        }
        if let Some(id) = std::env::var("KLASSENWERK_TASK").ok().and_then(|v| v.parse::<u64>().ok()) {
            if app.db.task(id).is_some() {
                app.selected = Some(id);
            }
        }
        app.sheet_sel = app.db.sheets.first().map(|s| s.id);
        match std::env::var("KLASSENWERK_TAB").unwrap_or_default().as_str() {
            "sheets" => app.tab = Tab::Sheets,
            "preview" => {
                app.tab = Tab::Sheets;
                app.sheet_view = SheetView::Preview;
            }
            "stats" => app.tab = Tab::Stats,
            "results" => {
                // the last sheet: in the demo the exam with points per Teilaufgabe
                app.tab = Tab::Sheets;
                app.sheet_view = SheetView::Results;
                app.sheet_sel = app.db.sheets.last().map(|s| s.id);
            }
            "classes" => {
                app.tab = Tab::Classes;
                app.course_sel = app.db.courses.first().map(|c| c.id);
            }
            "types" => {
                app.modal = Modal::Settings;
                app.settings_tab = 4;
            }
            "grades" => {
                app.modal = Modal::Settings;
                app.settings_tab = 2;
                app.scheme_sel = 1;
            }
            _ => {}
        }
        app.start_spell();
        app.start_thes();
        // updates: once a day, only for the real data (not for screenshots)
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if app.persist && app.db.settings.update_check && app.db.settings.update_last != today {
            app.check_update(true);
        }
        app
    }

    /// Looks for a new version in the background; `quiet`: no message when there is none.
    fn check_update(&mut self, quiet: bool) {
        if self.update_rx.is_some() {
            return;
        }
        self.db.settings.update_last = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.changed();
        self.update_status = "Suche nach einer neuen Version …".into();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send((quiet, crate::update::check()));
        });
        self.update_rx = Some(rx);
    }

    fn poll_update(&mut self) {
        use crate::update::Check;
        if let Some(rx) = &self.update_rx {
            if let Ok((quiet, r)) = rx.try_recv() {
                self.update_rx = None;
                match r {
                    Check::Newer(rel, asset) => {
                        self.update_status = format!("Neue Version {} verfügbar.", rel.tag_name);
                        let skipped = quiet && rel.tag_name == self.db.settings.update_skip;
                        if !skipped {
                            if !quiet {
                                self.modal = Modal::Update;
                            }
                            self.update = Some((rel, asset));
                        }
                    }
                    Check::UpToDate => {
                        self.update_status = format!("Klassenwerk {} ist aktuell.", env!("CARGO_PKG_VERSION"));
                        if !quiet {
                            self.toast(self.update_status.clone());
                        }
                    }
                    Check::Failed(e) => {
                        self.update_status = format!("Prüfung nicht möglich: {e}");
                        if !quiet {
                            self.toast(self.update_status.clone());
                        }
                    }
                }
            }
        }
        if let Some(rx) = &self.download_rx {
            if let Ok(r) = rx.try_recv() {
                self.download_rx = None;
                let kind = crate::update::this_kind();
                match r {
                    Ok(path) if kind == crate::update::Kind::WindowsSetup => {
                        // save, start the installer, close: the installer replaces the program and starts it again
                        match self.save_now().and_then(|_| crate::update::start_installer(&path)) {
                            Ok(()) => std::process::exit(0),
                            Err(e) => self.toast(e),
                        }
                    }
                    Ok(path) => {
                        self.update_status = format!("Gespeichert: {}", path.display());
                        self.toast(format!(
                            "Neue Version gespeichert in {} – zum Installieren entpacken bzw. die Anleitung im Paket befolgen.",
                            path.display()
                        ));
                        if let Some(dir) = path.parent() {
                            export::open_path(dir);
                        }
                        self.modal = Modal::None;
                    }
                    Err(e) => {
                        self.update_status = e.clone();
                        self.toast(e);
                    }
                }
            }
        }
    }

    fn start_download(&mut self) {
        let Some((_, Some(asset))) = &self.update else { return };
        let asset = asset.clone();
        let kind = crate::update::this_kind();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::update::fetch(&asset, kind));
        });
        self.download_rx = Some(rx);
        self.update_status = "Wird heruntergeladen …".into();
    }

    /// Dialog for a new version: release notes, download / install.
    fn ui_update(&mut self, ctx: &egui::Context) {
        let Some((rel, asset)) = self.update.clone() else {
            self.modal = Modal::None;
            return;
        };
        let mut open = true;
        let mut close = false;
        let kind = crate::update::this_kind();
        egui::Window::new(format!("Neue Version: Klassenwerk {}", rel.tag_name.trim_start_matches('v')))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Installiert ist Version {}.", env!("CARGO_PKG_VERSION")));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let n = crate::update::notes(&rel);
                    ui.label(if n.is_empty() { "Keine Beschreibung.".to_string() } else { n });
                });
                ui.add_space(8.0);
                let busy = self.download_rx.is_some();
                ui.horizontal_wrapped(|ui| {
                    match &asset {
                        Some(a) => {
                            let label = if kind == crate::update::Kind::WindowsSetup { "⬇  Herunterladen und installieren" } else { "⬇  Herunterladen" };
                            let tip = match kind {
                                crate::update::Kind::WindowsSetup => "Klassenwerk speichert, schließt sich und startet nach dem Update neu.".to_string(),
                                _ => format!("{} ({:.1} MB) in den Ordner Downloads", a.name, a.size as f64 / 1_048_576.0),
                            };
                            if ui.add_enabled(!busy, egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(ACCENT)).on_hover_text(tip).clicked() {
                                self.start_download();
                            }
                        }
                        None => {
                            ui.label(RichText::new("Für dieses System liegt keine passende Datei bei – bitte auf GitHub herunterladen.").color(WARN));
                        }
                    }
                    if ui.button("Auf GitHub ansehen").clicked() {
                        export::open_path(std::path::Path::new(&rel.html_url));
                    }
                    if ui.button("Diese Version überspringen").clicked() {
                        self.db.settings.update_skip = rel.tag_name.clone();
                        self.changed();
                        self.update = None;
                        close = true;
                    }
                    if ui.button("Später").clicked() {
                        close = true;
                    }
                });
                if busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(&self.update_status);
                    });
                }
            });
        if !open || close {
            self.modal = Modal::None;
        }
    }

    /// Loads the spelling dictionary in the background (it takes a moment).
    fn start_spell(&mut self) {
        self.spell = None;
        self.spell_cache.clear();
        self.spell_rx = None;
        if !self.db.settings.spell_check {
            self.spell_status = "ausgeschaltet".into();
            return;
        }
        let chosen = match std::env::var("KLASSENWERK_SPELLDICT") {
            Ok(p) if !self.persist => p,
            _ => self.db.settings.spell_dict.clone(),
        };
        match crate::spell::find(&chosen) {
            None => {
                self.spell_status = "Kein deutsches Wörterbuch gefunden. Es gehört zu LibreOffice (oder unter Linux zum Paket hunspell-de-de); eine .dic-Datei lässt sich auch hier wählen.".into();
            }
            Some(p) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let path = p.clone();
                std::thread::spawn(move || {
                    let r = crate::spell::Dict::load(&path);
                    let _ = tx.send((path, r));
                });
                self.spell_rx = Some(rx);
                self.spell_status = format!("wird geladen: {}", p.display());
            }
        }
    }

    fn poll_spell(&mut self) {
        let Some(rx) = &self.spell_rx else { return };
        if let Ok((p, r)) = rx.try_recv() {
            self.spell_rx = None;
            match r {
                Ok(d) => {
                    self.spell_status = format!("{}: {} Wörter ({})", d.name, d.len(), p.display());
                    self.spell = Some(std::sync::Arc::new(d));
                    self.spell_cache.clear();
                }
                Err(e) => self.spell_status = format!("Wörterbuch nicht lesbar: {e}"),
            }
        }
    }

    /// Loads the thesaurus in the background.
    fn start_thes(&mut self) {
        self.thes = None;
        self.thes_rx = None;
        if !self.db.settings.thesaurus {
            self.thes_status = "ausgeschaltet".into();
            return;
        }
        let chosen = match std::env::var("KLASSENWERK_THESAURUS") {
            Ok(p) if !self.persist => p,
            _ => self.db.settings.thesaurus_file.clone(),
        };
        match crate::thesaurus::find(&chosen) {
            None => {
                self.thes_status = "Kein deutscher Thesaurus gefunden. Er gehört zu LibreOffice (oder unter Linux zum Paket mythes-de); eine th_de_DE_v2.dat lässt sich auch hier wählen.".into();
            }
            Some(p) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let path = p.clone();
                std::thread::spawn(move || {
                    let r = crate::thesaurus::Thesaurus::load(&path);
                    let _ = tx.send((path, r));
                });
                self.thes_rx = Some(rx);
                self.thes_status = format!("wird geladen: {}", p.display());
            }
        }
    }

    fn poll_thes(&mut self) {
        let Some(rx) = &self.thes_rx else { return };
        if let Ok((p, r)) = rx.try_recv() {
            self.thes_rx = None;
            match r {
                Ok(t) => {
                    self.thes_status = format!("{}: {} Stichwörter ({})", t.name, t.len(), p.display());
                    self.thes = Some(std::sync::Arc::new(t));
                }
                Err(e) => self.thes_status = format!("Thesaurus nicht lesbar: {e}"),
            }
        }
    }

    /// Example class with results for screenshots (--demo).
    fn demo_class(&mut self) {
        let mut c = crate::classes::Course { id: 9000, name: "12a".into(), subject: "Mathematik".into(), school_year: "2026/27".into(), ..Default::default() };
        let names: Vec<String> = ["Becker, Anna", "Dorn, Ben", "Ernst, Clara", "Fischer, David", "Graf, Emma", "Huber, Finn", "Ibrahim, Lea"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        c.add_students(&names, &mut self.db.next_id);
        let a = self.db.sheets[0].clone();
        let nid = self.db.new_id();
        let mut b = self.db.make_parallel(&a, ParallelOptions { replace: true, shuffle: true, tolerance: 2 }, 3, nid).sheet;
        self.db.sheets[0].group = "A".into();
        let total = self.db.sheet_total(&a) as f32;
        let pts = [0.95, 0.62, 0.78, 0.41, 0.88, 0.55, 0.7];
        self.db.sheets[0].course_id = Some(c.id);
        b.course_id = Some(c.id);
        for (i, st) in c.students.iter().enumerate() {
            let r =
                ResultRow { name: st.name.clone(), student: Some(st.id), points: (total * pts[i] * 2.0).round() / 2.0, absent: i == 6, ..Default::default() };
            if i % 2 == 0 {
                self.db.sheets[0].results.push(r);
            } else {
                b.results.push(r);
            }
        }
        // a second exam, counting double
        let mut t2 = self.db.sheets[0].clone();
        t2.id = self.db.new_id();
        t2.title = "Flächenberechnung".into();
        t2.date = "15.12.2026".into();
        t2.group.clear();
        t2.weight = 2.0;
        t2.results = c
            .students
            .iter()
            .enumerate()
            .map(|(i, st)| ResultRow {
                name: st.name.clone(),
                student: Some(st.id),
                points: (total * [0.8, 0.7, 0.92, 0.5, 0.97, 0.33, 0.6][i] * 2.0).round() / 2.0,
                absent: false,
                ..Default::default()
            })
            .collect();
        // Teilaufgaben for the tasks of the second exam, and points entered per Teilaufgabe
        for id in t2.task_ids() {
            if let Some(t) = self.db.task_mut(id) {
                if t.parts.is_empty() && t.points >= 4 {
                    let a = t.points / 2;
                    t.parts = vec![
                        Part { label: "a".into(), points: a, expectation: "Ansatz und Rechnung".into() },
                        Part { label: "b".into(), points: t.points - a, expectation: "Ergebnis mit Begründung".into() },
                    ];
                }
            }
        }
        let cols = self.db.score_columns(&t2);
        t2.detail_entry = true;
        for (i, r) in t2.results.iter_mut().enumerate() {
            for (k, col) in cols.iter().enumerate() {
                let f = [0.8, 0.7, 0.92, 0.5, 0.97, 0.33, 0.6][i] * if k % 2 == 0 { 1.05 } else { 0.92 };
                r.scores.insert(col.key.clone(), ((col.max * f).min(col.max) * 2.0).round() / 2.0);
            }
            r.sum_scores(&cols);
        }
        // oral grades and one report grade set by hand
        let grades = ["2", "3", "1-", "4", "2+", "3-", "2"];
        let mut oral = crate::classes::Assessment { id: self.db.new_id(), title: "Mündlich 1. Hj.".into(), term: 1, ..Default::default() };
        for (st, g) in c.students.iter().zip(grades) {
            crate::classes::set_grade(&mut oral.grades, st.id, g);
        }
        c.assessments.push(oral);
        let mut talk = crate::classes::Assessment { id: self.db.new_id(), title: "Referat".into(), date: "20.11.2026".into(), ..Default::default() };
        for st in c.students.iter().step_by(3) {
            crate::classes::set_grade(&mut talk.grades, st.id, "2");
        }
        c.assessments.push(talk);
        if let Some(st) = c.students.get(3) {
            crate::classes::set_grade(&mut c.report, st.id, "3");
        }
        // a task with random numbers (Aufgabenvariante)
        let id = self.db.new_id();
        let (task, _) = crate::mcp::doc_from_assistant(
            "Ein rechteckiges Grundstück ist {=a} m lang und {=b} m breit.\n\n| a) | Berechne den Flächeninhalt. | 2 P. |\n| b) | Wie lang ist die Diagonale? | 3 P. |",
        );
        let (solution, _) = crate::mcp::doc_from_assistant(
            "a) $A = {=a} \\cdot {=b} = {=a*b}\\,\\text{m}^2$\nb) $d = \\sqrt{{=a}^2 + {=b}^2} \\approx {=sqrt(a^2+b^2);2}\\,\\text{m}$",
        );
        self.db.tasks.push(Task {
            id,
            subject: "Mathematik".into(),
            topic: "Geometrie".into(),
            subtopic: "Rechteck mit Zufallszahlen".into(),
            points: 5,
            task,
            solution,
            parts: vec![
                Part { label: "a".into(), points: 2, expectation: "A = a · b mit Einheit".into() },
                Part { label: "b".into(), points: 3, expectation: "Satz des Pythagoras, gerundet".into() },
            ],
            vars: vec![
                crate::vary::Var { name: "a".into(), min: 12.0, max: 40.0, step: 1.0 },
                crate::vary::Var { name: "b".into(), min: 5.0, max: 25.0, step: 0.5 },
            ],
            created: Some(now()),
            ..Default::default()
        });
        // the second exam in the 2nd Halbjahr
        t2.date = "15.03.2027".into();
        self.db.sheets.push(b);
        self.db.sheets.push(t2);
        self.course_sel = Some(c.id);
        self.db.courses.push(c);
    }

    /// Takes over tasks and drafts the AI assistant (MCP server) wrote into the data file.
    fn sync_external(&mut self) {
        if !self.persist {
            return;
        }
        let stamp = Database::file_stamp();
        if stamp.is_none() || stamp == self.file_stamp {
            return;
        }
        self.file_stamp = stamp;
        let Some(file) = Database::load_plain(&Database::path()) else { return };
        let (t, s) = self.db.merge_external(&file, &mut self.known_external);
        if t + s > 0 {
            let mut parts = Vec::new();
            if t > 0 {
                parts.push(format!("{t} Aufgabe(n)"));
            }
            if s > 0 {
                parts.push(format!("{s} Klausur-Entwurf/-Entwürfe"));
            }
            self.toast(format!("✦ Vom KI-Assistenten: {} übernommen.", parts.join(" und ")));
            self.changed();
        }
    }

    fn save_now(&mut self) -> Result<(), String> {
        // never overwrite what the MCP server added in the meantime
        self.sync_external();
        let r = self.db.save();
        self.file_stamp = Database::file_stamp();
        r
    }

    fn changed(&mut self) {
        self.dirty = true;
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 0.0));
    }

    fn rand(&mut self) -> u64 {
        let mut x = self.rnd_seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rnd_seed = x;
        x
    }

    fn matches(&self, t: &Task) -> bool {
        if let Some(s) = &self.f_subject {
            if &t.subject != s {
                return false;
            }
        }
        if let Some(s) = &self.f_topic {
            if &t.topic != s {
                return false;
            }
        }
        if let Some(tag) = &self.f_tag {
            if !t.tags.iter().any(|x| x == tag) {
                return false;
            }
        }
        if self.only_fav && !t.favorite {
            return false;
        }
        if self.only_unused && t.last_any_use().is_some() {
            return false;
        }
        let q = self.search.trim().to_lowercase();
        if !q.is_empty() {
            let hay = format!("{} {} {} {} {} {} {}", t.subject, t.topic, t.subtopic, t.tags.join(" "), t.notes, t.task.text, t.solution.text).to_lowercase();
            if !q.split_whitespace().all(|w| hay.contains(w)) {
                return false;
            }
        }
        true
    }

    fn filtered(&self) -> Vec<u64> {
        let mut v: Vec<&Task> = self.db.tasks.iter().filter(|t| self.matches(t)).collect();
        match self.sort {
            TaskSort::Subject => v.sort_by(|a, b| {
                (a.subject.to_lowercase(), a.topic.to_lowercase(), a.subtopic.to_lowercase(), a.id).cmp(&(
                    b.subject.to_lowercase(),
                    b.topic.to_lowercase(),
                    b.subtopic.to_lowercase(),
                    b.id,
                ))
            }),
            TaskSort::Title => v.sort_by_key(|t| t.title().to_lowercase()),
            TaskSort::Points => v.sort_by_key(|t| -t.points),
            TaskSort::LastUsed => v.sort_by_key(|t| std::cmp::Reverse(t.last_any_use())),
            TaskSort::Newest => v.sort_by_key(|t| std::cmp::Reverse((t.created, t.id))),
        }
        v.into_iter().map(|t| t.id).collect()
    }

    fn tree(&self) -> BTreeMap<String, BTreeMap<String, usize>> {
        let mut m: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
        for t in &self.db.tasks {
            *m.entry(t.subject.clone()).or_default().entry(t.topic.clone()).or_default() += 1;
        }
        m
    }

    fn all_tags(&self) -> BTreeSet<String> {
        self.db.tasks.iter().flat_map(|t| t.tags.iter().cloned()).collect()
    }

    fn subjects(&self) -> BTreeSet<String> {
        let mut s: BTreeSet<String> = self.db.tasks.iter().map(|t| t.subject.clone()).filter(|s| !s.is_empty()).collect();
        s.extend(self.db.sheets.iter().map(|x| x.subject.clone()).filter(|s| !s.is_empty()));
        s
    }

    fn apply(&mut self, a: Act) {
        match a {
            Act::Select(id) => {
                self.selected = Some(id);
                self.editor = None;
            }
            Act::Edit(id) => {
                if let Some(t) = self.db.task(id) {
                    self.tab = Tab::Pool;
                    self.selected = Some(id);
                    self.editor = Some(Editor::new(t.clone(), false));
                }
            }
            Act::NewTask => {
                let id = self.db.new_id();
                let t = Task {
                    id,
                    subject: self.f_subject.clone().unwrap_or_default(),
                    topic: self.f_topic.clone().unwrap_or_default(),
                    points: 5,
                    author_new: self.db.settings.teacher.clone(),
                    author_upd: self.db.settings.teacher.clone(),
                    created: Some(now()),
                    updated: Some(now()),
                    indent: "L".into(),
                    changed: true,
                    task: Doc::plain("Beschreiben Sie …\n\n| a) | Teilaufgabe | 2 P. |\n| b) | Teilaufgabe | 3 P. |"),
                    solution: Doc::plain("a) …\nb) …"),
                    parts: vec![
                        Part { label: "a".into(), points: 2, expectation: String::new() },
                        Part { label: "b".into(), points: 3, expectation: String::new() },
                    ],
                    ..Default::default()
                };
                self.tab = Tab::Pool;
                self.editor = Some(Editor::new(t, true));
            }
            Act::Duplicate(id) => {
                if let Some(t) = self.db.task(id).cloned() {
                    let nid = self.db.new_id();
                    let mut c = t;
                    c.id = nid;
                    c.subtopic = format!("{} (Kopie)", c.subtopic);
                    c.created = Some(now());
                    c.updated = Some(now());
                    c.usage.clear();
                    c.origin.clear();
                    c.first_used = None;
                    c.last_used = None;
                    c.exam_first = None;
                    c.exam_last = None;
                    c.changed = true;
                    self.db.tasks.push(c);
                    self.selected = Some(nid);
                    self.changed();
                    self.toast("Aufgabe kopiert – die Kopie ist ausgewählt.");
                }
            }
            Act::AskDelete(ids) => self.modal = Modal::DeleteTasks(ids),
            Act::ToggleFav(id) => {
                if let Some(t) = self.db.task_mut(id) {
                    t.favorite = !t.favorite;
                }
                self.changed();
            }
            Act::AddToSheet(sid, tid) => self.add_to_sheet(sid, tid),
            Act::AddToNewSheet(tid) => {
                let sid = self.new_sheet("Klausur".into());
                self.add_to_sheet(sid, tid);
                self.tab = Tab::Sheets;
            }
            Act::PrintTask(id) => {
                if let Some(t) = self.db.task(id) {
                    let c = export::compose_task(&self.db, t);
                    let html = export::to_html(&self.db, &c, true);
                    let _ = export::open_html(&t.title(), &html);
                    self.toast("Druckvorschau im Browser geöffnet.");
                }
            }
            Act::WordTask(id, rtf) => {
                if let Some(t) = self.db.task(id) {
                    let c = export::compose_task(&self.db, t);
                    self.save_word(&c, true, rtf);
                }
            }
            Act::SelectSheet(id) => self.sheet_sel = Some(id),
            Act::OpenSheet(id, v) => {
                self.sheet_sel = Some(id);
                self.sheet_view = v;
                self.tab = Tab::Sheets;
            }
            Act::DuplicateSheet(id) => {
                if let Some(s) = self.db.sheets.iter().find(|s| s.id == id).cloned() {
                    let nid = self.db.new_id();
                    let mut c = s;
                    c.id = nid;
                    c.title = format!("{} (Kopie)", c.title);
                    c.results.clear();
                    c.group.clear();
                    c.parallel_of = None;
                    c.origin.clear();
                    c.used_marked = false;
                    c.archived = false;
                    c.created = Some(now());
                    c.date = chrono::Local::now().format("%d.%m.%Y").to_string();
                    self.db.sheets.push(c);
                    self.sheet_sel = Some(nid);
                    self.sheet_view = SheetView::Edit;
                    self.changed();
                    self.toast("Mappe als Grundlage kopiert.");
                }
            }
            Act::ParallelSheet(id, opt) => {
                let Some(src) = self.db.sheets.iter().find(|s| s.id == id).cloned() else { return };
                if src.group.trim().is_empty() {
                    if let Some(s) = self.db.sheets.iter_mut().find(|s| s.id == id) {
                        s.group = "A".into();
                    }
                }
                let src = self.db.sheets.iter().find(|s| s.id == id).cloned().unwrap();
                let nid = self.db.new_id();
                let seed = self.rand();
                let mut r = self.db.make_parallel(&src, opt, seed, nid);
                r.sheet.created = Some(now());
                let group = r.sheet.group.clone();
                self.db.sheets.push(r.sheet);
                self.sheet_sel = Some(nid);
                self.sheet_view = SheetView::Edit;
                self.changed();
                let mut msg = format!("Gruppe {group} angelegt.");
                if opt.replace {
                    msg += &format!(" {} Aufgabe(n) ersetzt", r.replaced);
                    if r.kept > 0 {
                        msg += &format!(", {} beibehalten (keine passende Aufgabe im Pool)", r.kept);
                    }
                    msg += ".";
                    match r.points_diff {
                        0 => {}
                        d if d > 0 => msg += &format!(" Achtung: {d} Punkt(e) mehr als die andere Gruppe."),
                        d => msg += &format!(" Achtung: {} Punkt(e) weniger als die andere Gruppe.", -d),
                    }
                }
                self.toast(msg);
            }
            Act::ArchiveSheet(id) => {
                if let Some(s) = self.db.sheets.iter_mut().find(|s| s.id == id) {
                    s.archived = !s.archived;
                }
                self.changed();
            }
            Act::PrintSheet(id, sol) => self.print_sheet(id, sol),
            Act::NewSheet(kind) => {
                self.new_sheet(kind);
            }
        }
    }

    fn new_sheet(&mut self, kind: String) -> u64 {
        let id = self.db.new_id();
        let tpl = if self.db.settings.templates.iter().any(|t| t.name == kind) {
            kind.clone()
        } else if self.db.settings.is_exam(&kind) {
            "Klausur".into()
        } else {
            "Übung".into()
        };
        let s = Sheet {
            id,
            title: format!("Neue {kind}"),
            kind,
            subject: self.f_subject.clone().unwrap_or_default(),
            date: chrono::Local::now().format("%d.%m.%Y").to_string(),
            template: tpl,
            scheme: self.db.settings.schemes.first().map(|s| s.name.clone()).unwrap_or_default(),
            created: Some(now()),
            ..Default::default()
        };
        self.db.sheets.push(s);
        self.sheet_sel = Some(id);
        self.sheet_view = SheetView::Edit;
        self.changed();
        id
    }

    fn add_to_sheet(&mut self, sid: u64, tid: u64) {
        let title = self.db.task(tid).map(|t| t.title()).unwrap_or_default();
        let Some(s) = self.db.sheets.iter_mut().find(|s| s.id == sid) else { return };
        let st = s.title.clone();
        if s.contains(tid) && !s.allow_duplicates {
            self.toast(format!("„{title}“ ist bereits in „{st}“ enthalten (doppelte Aufgaben sind ausgeschaltet)."));
            return;
        }
        s.items.push(SheetItem::Task(tid));
        self.changed();
        self.toast(format!("„{title}“ zu „{st}“ hinzugefügt."));
    }

    fn save_file(&mut self, name: &str, filter: &str, ext: &str, data: &[u8]) {
        if self.shot.is_some() {
            return;
        }
        if let Some(p) = rfd::FileDialog::new().set_file_name(name).add_filter(filter, &[ext]).save_file() {
            match std::fs::write(&p, data) {
                Ok(_) => {
                    self.toast(format!("Gespeichert: {}", p.display()));
                    if ext == "rtf" || ext == "docx" {
                        export::open_path(&p);
                    }
                }
                Err(e) => self.toast(format!("Fehler beim Speichern: {e}")),
            }
        }
    }

    /// Saves a composed document for Word: .docx (default) or .rtf (older programs).
    fn save_word(&mut self, c: &export::Composed, underline: bool, rtf: bool) {
        if rtf {
            let r = export::to_rtf(&self.db, c, underline);
            self.save_file(&format!("{}.rtf", export::safe_name(&c.title)), "Word / RTF", "rtf", r.as_bytes());
        } else {
            let d = crate::docx::to_docx(&self.db, c, underline);
            self.save_file(&format!("{}.docx", export::safe_name(&c.title)), "Word-Dokument", "docx", &d);
        }
    }

    fn print_sheet(&mut self, id: u64, solution: bool) {
        let Some(s) = self.db.sheets.iter().find(|s| s.id == id) else { return };
        let c = export::compose(&self.db, s, solution);
        let html = export::to_html(&self.db, &c, s.underline_lines);
        match export::open_html(&c.title, &html) {
            Ok(_) => self.toast("Druckvorschau im Browser geöffnet – dort drucken oder als PDF speichern."),
            Err(e) => self.toast(format!("Fehler: {e}")),
        }
    }

    fn mark_used(&mut self, sid: u64) {
        let Some(s) = self.db.sheets.iter().find(|s| s.id == sid).cloned() else { return };
        let exam = self.db.settings.is_exam(&s.kind);
        let when = chrono::NaiveDate::parse_from_str(&s.date, "%d.%m.%Y").ok().and_then(|d| d.and_hms_opt(8, 0, 0)).unwrap_or_else(now);
        for id in s.task_ids() {
            if let Some(t) = self.db.task_mut(id) {
                if exam {
                    t.exam_first = Some(t.exam_first.map(|x| x.min(when)).unwrap_or(when));
                    t.exam_last = Some(t.exam_last.map(|x| x.max(when)).unwrap_or(when));
                    t.exam = Some(when);
                } else {
                    t.first_used = Some(t.first_used.map(|x| x.min(when)).unwrap_or(when));
                    t.last_used = Some(t.last_used.map(|x| x.max(when)).unwrap_or(when));
                    t.lesson = Some(when);
                }
                t.usage.push(Usage { date: s.date.clone(), sheet: s.title.clone(), exam });
            }
        }
        if let Some(s) = self.db.sheets.iter_mut().find(|x| x.id == sid) {
            s.used_marked = true;
        }
        self.changed();
        self.toast("Die Aufgaben sind als verwendet vermerkt.");
    }

    fn random_fill(&mut self, sid: u64) {
        let Some(s) = self.db.sheets.iter().find(|s| s.id == sid).cloned() else { return };
        let mut have = self.db.sheet_total(&s);
        let subj = if self.picker_subject.is_empty() { s.subject.clone() } else { self.picker_subject.clone() };
        let mut cands: Vec<(u64, i32)> =
            self.db.tasks.iter().filter(|t| !s.contains(t.id) && (subj.is_empty() || t.subject == subj) && t.points > 0).map(|t| (t.id, t.points)).collect();
        for i in (1..cands.len()).rev() {
            let j = (self.rand() % (i as u64 + 1)) as usize;
            cands.swap(i, j);
        }
        cands.sort_by_key(|(id, _)| self.db.task(*id).map(|t| t.exam_last.is_some()).unwrap_or(false));
        let mut added = 0;
        for (id, p) in cands {
            if have + p <= self.rnd_target {
                if let Some(sh) = self.db.sheets.iter_mut().find(|x| x.id == sid) {
                    sh.items.push(SheetItem::Task(id));
                }
                have += p;
                added += 1;
            }
        }
        self.changed();
        self.toast(format!("{added} Aufgaben ergänzt – jetzt {have} Punkte."));
    }

    fn import_paths(&mut self, paths: Vec<PathBuf>) {
        let mut keys: HashSet<String> = self.db.tasks.iter().map(|t| t.dup_key()).collect();
        let (mut added, mut skipped, mut errors, mut new_sheets) = (0, 0, Vec::new(), 0);
        for p in paths {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let Ok(bytes) = std::fs::read(&p) else {
                errors.push(format!("{} kann nicht gelesen werden", p.display()));
                continue;
            };
            let text = String::from_utf8_lossy(&bytes).to_string();
            let mut tasks: Vec<Task> = Vec::new();
            if ext == "kwerk" || ext == "kldz" || ext == "json" {
                match serde_json::from_str::<Bundle>(&text) {
                    Ok(b) => {
                        let mut map = std::collections::HashMap::new();
                        for mut t in b.tasks {
                            let nid = self.db.new_id();
                            map.insert(t.id, nid);
                            t.id = nid;
                            tasks.push(t);
                        }
                        for mut s in b.sheets {
                            s.id = self.db.new_id();
                            s.items = s
                                .items
                                .into_iter()
                                .filter_map(|i| match i {
                                    SheetItem::Task(id) => map.get(&id).map(|n| SheetItem::Task(*n)),
                                    x => Some(x),
                                })
                                .collect();
                            self.db.sheets.push(s);
                            new_sheets += 1;
                        }
                    }
                    Err(e) => errors.push(format!("{}: {e}", p.display())),
                }
            } else {
                match import_xml(&text, &mut self.db) {
                    Ok(t) => tasks = t,
                    Err(e) => errors.push(format!("{}: {e}", p.display())),
                }
            }
            for t in tasks {
                let k = t.dup_key();
                if keys.contains(&k) {
                    skipped += 1;
                    continue;
                }
                keys.insert(k);
                self.db.tasks.push(t);
                added += 1;
            }
        }
        self.changed();
        let mut msg = format!("{added} Aufgaben importiert");
        if skipped > 0 {
            msg += &format!(", {skipped} doppelte übersprungen");
        }
        if new_sheets > 0 {
            msg += &format!(", {new_sheets} Mappen");
        }
        if !errors.is_empty() {
            msg += &format!(" – Fehler: {}", errors.join("; "));
        }
        self.toast(msg);
    }

    fn import_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("Aufgaben (XML, Klassenwerk-Paket, Word, RTF)", &["xml", "kwerk", "kldz", "json", "docx", "rtf"])
            .set_title("Aufgaben importieren")
            .pick_files()
        {
            let (word, rest): (Vec<PathBuf>, Vec<PathBuf>) = files.into_iter().partition(|p| {
                let e = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                e == "docx" || e == "rtf"
            });
            if !rest.is_empty() {
                self.import_paths(rest);
            }
            if let Some(p) = word.into_iter().next() {
                self.open_word_import(p);
            }
        }
    }

    fn export_bundle(&mut self, ids: &[u64], sheets: &[u64]) {
        let b = Bundle {
            format: "Klassenwerk-Paket 1".into(),
            author: self.db.settings.teacher.clone(),
            created: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
            tasks: ids.iter().filter_map(|id| self.db.task(*id).cloned()).collect(),
            sheets: self.db.sheets.iter().filter(|s| sheets.contains(&s.id)).cloned().collect(),
        };
        let json = serde_json::to_string_pretty(&b).unwrap_or_default();
        self.save_file("Aufgabenpaket.kwerk", "Klassenwerk-Paket", "kwerk", json.as_bytes());
    }

    /// Tasks as Moodle XML questions.
    fn export_moodle(&mut self, ids: &[u64], name: &str) {
        let tasks: Vec<&Task> = ids.iter().filter_map(|id| self.db.task(*id)).collect();
        let xml = crate::moodle::export(&tasks);
        let fname = format!("{}-Moodle.xml", export::safe_name(name));
        self.save_file(&fname, "Moodle-XML", "xml", xml.as_bytes());
    }

    fn export_xml_tasks(&mut self, ids: &[u64], name: &str) {
        let tasks: Vec<&Task> = ids.iter().filter_map(|id| self.db.task(*id)).collect();
        let xml = export_xml(&tasks);
        let fname = format!("{}.xml", export::safe_name(name));
        self.save_file(&fname, "XML", "xml", xml.as_bytes());
    }

    fn optimize_all_images(&mut self) {
        let (mut n, mut saved) = (0, 0usize);
        for t in &mut self.db.tasks {
            for d in [&mut t.task, &mut t.solution] {
                for o in &mut d.objects {
                    if o.kind != ObjKind::Image {
                        continue;
                    }
                    if let Some((b, w, h)) = optimize_image(&o.data) {
                        if b.len() < o.data.len() {
                            saved += o.data.len() - b.len();
                            if o.width <= 0.0 {
                                o.width = w as f32;
                                o.height = h as f32;
                            }
                            o.data = b;
                            n += 1;
                            t.changed = true;
                        }
                    }
                }
            }
        }
        self.cache = ObjCache::default();
        self.changed();
        self.toast(format!("{n} Grafiken optimiert, {} KB gespart.", saved / 1024));
    }

    fn ui_top(&mut self, ctx: &egui::Context) {
        let dark = self.db.settings.dark;
        let frame = egui::Frame::new()
            .fill(if dark { Color32::from_rgb(30, 31, 39) } else { Color32::WHITE })
            .inner_margin(egui::Margin::symmetric(14, 8))
            .stroke(Stroke::new(1.0_f32, if dark { Color32::from_rgb(45, 46, 58) } else { Color32::from_rgb(226, 228, 236) }));
        egui::TopBottomPanel::top("top").frame(frame).show(ctx, |ui| {
            ui.set_min_height(38.0);
            ui.horizontal(|ui| {
                if !self.db.settings.logo.is_empty() {
                    let key = self.db.settings.logo.len();
                    if self.logo_tex.as_ref().map(|(k, _)| *k != key).unwrap_or(true) {
                        if let Ok(img) = image::load_from_memory(&self.db.settings.logo) {
                            let rgba = img.to_rgba8();
                            let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                            self.logo_tex = Some((key, ctx.load_texture("logo", ci, egui::TextureOptions::LINEAR)));
                        }
                    }
                    if let Some((_, t)) = &self.logo_tex {
                        let s = t.size_vec2();
                        ui.add(egui::Image::new((t.id(), Vec2::new(34.0 * s.x / s.y.max(1.0), 34.0))));
                    }
                } else {
                    if self.app_logo.is_none() {
                        if let Ok(img) = image::load_from_memory(include_bytes!("../assets/logo-64.png")) {
                            let rgba = img.to_rgba8();
                            let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                            self.app_logo = Some(ctx.load_texture("app_logo", ci, egui::TextureOptions::LINEAR));
                        }
                    }
                    if let Some(t) = &self.app_logo {
                        ui.add(egui::Image::new((t.id(), Vec2::splat(34.0))));
                    }
                }
                ui.vertical(|ui| {
                    ui.add_space(1.0);
                    ui.label(RichText::new("Klassenwerk").size(17.0).strong());
                    let sub = if self.db.settings.school.is_empty() { "Aufgaben · Klausuren · Noten".to_string() } else { self.db.settings.school.clone() };
                    ui.label(RichText::new(sub).size(11.0).weak());
                });
                ui.add_space(24.0);
                for (t, label) in [
                    (Tab::Pool, "📚  Aufgabenpool"),
                    (Tab::Sheets, "📝  Klausuren & Übungen"),
                    (Tab::Classes, "👥  Klassen & Notenbuch"),
                    (Tab::Stats, "📊  Übersicht"),
                ] {
                    let sel = self.tab == t;
                    let txt = RichText::new(label).size(14.5);
                    let b = egui::Button::new(if sel { txt.color(Color32::WHITE) } else { txt })
                        .fill(if sel { ACCENT } else { Color32::TRANSPARENT })
                        .corner_radius(8)
                        .stroke(Stroke::NONE)
                        .min_size(Vec2::new(0.0, 32.0));
                    if ui.add(b).clicked() {
                        self.tab = t;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("⚙  Einstellungen").clicked() {
                        self.modal = Modal::Settings;
                    }
                    if let Some((rel, _)) = &self.update {
                        let b = egui::Button::new(RichText::new(format!("✨  Version {} ist da", rel.tag_name.trim_start_matches('v'))).color(Color32::WHITE))
                            .fill(GOOD);
                        if ui.add(b).on_hover_text("Was ist neu? Herunterladen und installieren").clicked() {
                            self.modal = Modal::Update;
                        }
                    }
                    if ui.button("❓").on_hover_text("Hilfe zur Formatierung").clicked() {
                        self.modal = Modal::Help;
                    }
                    let icon = if self.db.settings.dark { "☀" } else { "🌙" };
                    if ui.button(icon).on_hover_text("Hell/Dunkel umschalten").clicked() {
                        self.db.settings.dark = !self.db.settings.dark;
                        apply_theme(ctx, self.db.settings.dark);
                        self.changed();
                    }
                    ui.menu_button("📁  Datei", |ui| {
                        if ui.button("📥  Aufgaben importieren (XML / Paket) …").clicked() {
                            ui.close_menu();
                            self.import_dialog();
                        }
                        if ui
                            .button("📝  Aufgaben aus Word / RTF importieren …")
                            .on_hover_text(
                                "Ein Word- oder RTF-Dokument mit Markierungen wie „Aufgabe=1“ / „Lösung=1“ (auch A#1 / L#1) in einzelne Aufgaben aufteilen",
                            )
                            .clicked()
                        {
                            ui.close_menu();
                            if let Some(p) =
                                rfd::FileDialog::new().add_filter("Word / RTF", &["docx", "rtf"]).set_title("Aufgaben aus Word importieren").pick_file()
                            {
                                self.open_word_import(p);
                            }
                        }
                        ui.separator();
                        let ids = self.filtered();
                        if ui.button(format!("📦  Angezeigte Aufgaben als Paket exportieren ({}) …", ids.len())).clicked() {
                            ui.close_menu();
                            self.export_bundle(&ids, &[]);
                        }
                        if ui.button(format!("📄  Angezeigte Aufgaben als XML exportieren ({}) …", ids.len())).clicked() {
                            ui.close_menu();
                            self.export_xml_tasks(&ids, "Aufgaben");
                        }
                        if ui
                            .button(format!("🎓  Angezeigte Aufgaben für Moodle exportieren ({}) …", ids.len()))
                            .on_hover_text("Moodle-XML: in Moodle unter Fragensammlung → Importieren einlesen")
                            .clicked()
                        {
                            ui.close_menu();
                            self.export_moodle(&ids, "Aufgaben");
                        }
                        if ui
                            .button("🔁  Austauschordner …")
                            .on_hover_text("Aufgaben mit Kolleginnen und Kollegen über einen gemeinsamen Ordner teilen")
                            .clicked()
                        {
                            ui.close_menu();
                            self.modal = Modal::Exchange;
                        }
                        if ui.button("💾  Gesamte Sammlung archivieren (Paket) …").clicked() {
                            ui.close_menu();
                            let all: Vec<u64> = self.db.tasks.iter().map(|t| t.id).collect();
                            let sh: Vec<u64> = self.db.sheets.iter().map(|s| s.id).collect();
                            self.export_bundle(&all, &sh);
                        }
                        ui.separator();
                        if ui.button("🔤  Fächer / Schlagwörter global umbenennen …").clicked() {
                            ui.close_menu();
                            self.modal = Modal::Rename { field: 0, from: String::new(), to: String::new() };
                        }
                        if ui.button("🖼  Grafiken optimieren").clicked() {
                            ui.close_menu();
                            self.optimize_all_images();
                        }
                    });
                });
            });
        });
    }

    fn ui_status(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").exact_height(26.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(format!("{} Aufgaben · {} Mappen", self.db.tasks.len(), self.db.sheets.len())).small().weak());
                ui.separator();
                // screenshots for the documentation (--demo) look like normal use
                let st = if !self.persist && std::env::var("KLASSENWERK_DEMO").is_err() {
                    "Vorschau-Modus".to_string()
                } else if !self.persist {
                    "✔  Alles gespeichert".into()
                } else if self.dirty {
                    "● Änderungen werden gespeichert …".into()
                } else if !self.status.is_empty() {
                    self.status.clone()
                } else {
                    "✔  Alles gespeichert".into()
                };
                ui.label(RichText::new(st).small().weak());
                if let Some((msg, _)) = &self.toast {
                    let col = if self.db.settings.dark { Color32::from_rgb(160, 170, 255) } else { ACCENT };
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(msg).small().color(col));
                    });
                }
            });
        });
    }
}

impl eframe::App for App {
    /// Screenshots: no mouse, so no tooltips or hover marks appear by chance.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.shot.is_some() {
            raw.events.retain(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }));
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let t = ctx.input(|i| i.time);
        self.poll_spell();
        self.poll_thes();
        self.poll_update();
        if self.update_rx.is_some() || self.download_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        if self.spell_rx.is_some() || self.thes_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        let mut dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(ed) = &mut self.editor {
            // pictures dropped while a task is being edited go into that task
            let (imgs, rest): (Vec<PathBuf>, Vec<PathBuf>) = dropped.into_iter().partition(|p| is_image_path(p));
            dropped = rest;
            let which = ed.focus.min(1);
            for p in imgs {
                let doc = if which == 0 { &mut ed.task.task } else { &mut ed.task.solution };
                match std::fs::read(&p).ok().and_then(|b| add_image_object(&mut doc.objects, &b)) {
                    Some(m) => {
                        insert_wrap(&mut doc.text, &mut ed.cur[which], &m, "");
                    }
                    None => self.toast = Some((format!("{} ist kein lesbares Bild", p.display()), 0.0)),
                }
            }
        }
        let (word, dropped): (Vec<PathBuf>, Vec<PathBuf>) = dropped.into_iter().partition(|p| {
            let e = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            e == "docx" || e == "rtf" || e == "doc"
        });
        if let Some(p) = word.into_iter().next() {
            self.open_word_import(p);
        }
        if !dropped.is_empty() {
            self.import_paths(dropped);
        }
        if let Some((_, start)) = &mut self.toast {
            if *start == 0.0 {
                *start = t;
            }
            if t - *start > 6.0 {
                self.toast = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }

        self.ui_top(ctx);
        self.ui_status(ctx);
        match self.tab {
            Tab::Pool => self.ui_pool(ctx),
            Tab::Sheets => self.ui_sheets(ctx),
            Tab::Stats => self.ui_stats(ctx),
            Tab::Classes => self.ui_classes(ctx),
        }
        self.ui_modals(ctx);

        // look for entries from the AI assistant every 2 seconds
        if t - self.last_poll > 2.0 {
            self.last_poll = t;
            self.sync_external();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
        if self.dirty {
            if self.dirty_at == 0.0 {
                self.dirty_at = t;
            }
            if t - self.dirty_at > 1.0 {
                if self.persist {
                    self.status = match self.save_now() {
                        Ok(_) => String::new(),
                        Err(e) => format!("⚠ Speichern fehlgeschlagen: {e}"),
                    };
                }
                self.dirty = false;
                self.dirty_at = 0.0;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        if let Some(shot) = &mut self.shot {
            shot.frames += 1;
            ctx.request_repaint();
            if shot.frames >= 120 && !shot.requested {
                shot.requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let path = shot.path.clone();
            let img = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = img {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
                }
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = image::save_buffer(&path, &buf, w as u32, h as u32, image::ExtendedColorType::Rgba8);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if shot.frames > 900 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.persist {
            self.sync_external();
            if self.dirty {
                let _ = self.save_now();
            }
        }
    }
}
