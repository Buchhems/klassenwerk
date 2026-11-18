//! Fonts: uses the system fonts on Windows (Segoe UI, Times New Roman,
//! Cambria Math, Segoe UI Symbol) and falls back to egui's built-in fonts.

use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// egui font size that gives a glyph em of 1.0 (epaint 0.31 already scales the
/// font size to the em, so this is 1; a larger value makes formulas overlap)
pub const EM_TO_SIZE: f32 = 1.0;
/// distance from the top of an egui text row to the baseline, in em
pub const ASCENT: f32 = 0.91;

pub fn family(serif: bool, italic: bool, bold: bool) -> FontFamily {
    let name = match (serif, italic, bold) {
        (true, true, _) => "serif_i",
        (true, false, true) => "serif_b",
        (true, false, false) => "serif",
        (false, true, _) => "sans_i",
        (false, false, true) => "sans_b",
        (false, false, false) => return FontFamily::Proportional,
    };
    FontFamily::Name(name.into())
}

/// Font for formula text: the old formula editor names Arial or Times New Roman;
/// metrics must match closely or characters overlap.
pub fn formula_family(serif: bool, italic: bool, bold: bool) -> FontFamily {
    if serif {
        return family(true, italic, bold);
    }
    FontFamily::Name(
        match (italic, bold) {
            (true, _) => "arial_i",
            (false, true) => "arial_b",
            _ => "arial",
        }
        .into(),
    )
}

fn load(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Font files to try for one role, per system: Windows fonts, then the ones every
/// Mac has, then common Linux fonts (Liberation is metric-compatible with Arial/Times).
fn candidates(role: &str) -> Vec<String> {
    let win = std::env::var("WINDIR").map(|w| format!("{w}\\Fonts")).unwrap_or_else(|_| "C:\\Windows\\Fonts".into());
    let w = |f: &str| format!("{win}\\{f}");
    let mac = |f: &str| format!("/System/Library/Fonts/{f}");
    let macs = |f: &str| format!("/System/Library/Fonts/Supplemental/{f}");
    let lin = |subdirs: &[&str], f: &str| -> Vec<String> {
        let mut v = Vec::new();
        for base in ["/usr/share/fonts/truetype", "/usr/share/fonts/TTF", "/usr/share/fonts", "/usr/local/share/fonts", "/usr/share/fonts/opentype"] {
            for d in subdirs {
                v.push(format!("{base}/{d}/{f}"));
            }
            v.push(format!("{base}/{f}"));
        }
        v
    };
    let mut v: Vec<String> = match role {
        "sans" => vec![w("segoeui.ttf"), mac("Helvetica.ttc"), macs("Arial.ttf")],
        "sans_b" => vec![w("seguisb.ttf"), macs("Arial Bold.ttf")],
        "sans_i" => vec![w("segoeuii.ttf"), macs("Arial Italic.ttf")],
        "times" => vec![w("times.ttf"), macs("Times New Roman.ttf"), mac("Times.ttc")],
        "times_i" => vec![w("timesi.ttf"), macs("Times New Roman Italic.ttf")],
        "times_b" => vec![w("timesbd.ttf"), macs("Times New Roman Bold.ttf")],
        "math" => vec![w("cambria.ttc"), macs("STIXTwoMath.otf"), mac("Supplemental/STIXTwoMath.otf")],
        "sym" => vec![w("seguisym.ttf"), mac("Apple Symbols.ttf")],
        "arial" => vec![w("arial.ttf"), macs("Arial.ttf")],
        "arial_i" => vec![w("ariali.ttf"), macs("Arial Italic.ttf")],
        "arial_b" => vec![w("arialbd.ttf"), macs("Arial Bold.ttf")],
        _ => vec![],
    };
    let l: Vec<String> = match role {
        "sans" => [lin(&["noto", "noto-sans"], "NotoSans-Regular.ttf"), lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans.ttf")].concat(),
        "sans_b" => [lin(&["noto"], "NotoSans-SemiBold.ttf"), lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans-Bold.ttf")].concat(),
        "sans_i" => [lin(&["noto"], "NotoSans-Italic.ttf"), lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans-Oblique.ttf")].concat(),
        "times" => {
            [lin(&["liberation", "liberation2", "liberation-serif"], "LiberationSerif-Regular.ttf"), lin(&["dejavu", "dejavu-serif-fonts"], "DejaVuSerif.ttf")]
                .concat()
        }
        "times_i" => [
            lin(&["liberation", "liberation2", "liberation-serif"], "LiberationSerif-Italic.ttf"),
            lin(&["dejavu", "dejavu-serif-fonts"], "DejaVuSerif-Italic.ttf"),
        ]
        .concat(),
        "times_b" => [
            lin(&["liberation", "liberation2", "liberation-serif"], "LiberationSerif-Bold.ttf"),
            lin(&["dejavu", "dejavu-serif-fonts"], "DejaVuSerif-Bold.ttf"),
        ]
        .concat(),
        "math" | "sym" => lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans.ttf"),
        "arial" => {
            [lin(&["liberation", "liberation2", "liberation-sans"], "LiberationSans-Regular.ttf"), lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans.ttf")]
                .concat()
        }
        "arial_i" => [
            lin(&["liberation", "liberation2", "liberation-sans"], "LiberationSans-Italic.ttf"),
            lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans-Oblique.ttf"),
        ]
        .concat(),
        "arial_b" => {
            [lin(&["liberation", "liberation2", "liberation-sans"], "LiberationSans-Bold.ttf"), lin(&["dejavu", "dejavu-sans-fonts"], "DejaVuSans-Bold.ttf")]
                .concat()
        }
        _ => vec![],
    };
    v.extend(l);
    v
}

pub fn setup(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |key: &str, _file: &str, _index: u32| -> bool {
        for path in candidates(key) {
            if let Some(bytes) = load(&path) {
                let mut fd = FontData::from_owned(bytes);
                // Cambria Math is the second face in cambria.ttc
                fd.index = if path.ends_with("cambria.ttc") { 1 } else { 0 };
                fonts.font_data.insert(key.into(), Arc::new(fd));
                return true;
            }
        }
        false
    };
    let have = |b: bool, k: &str| if b { Some(k.to_string()) } else { None };
    let segoe = have(add("segoe", "segoeui.ttf", 0), "segoe");
    let segoe_b = have(add("segoe_b", "seguisb.ttf", 0), "segoe_b");
    let segoe_i = have(add("segoe_i", "segoeuii.ttf", 0), "segoe_i");
    let times = have(add("times", "times.ttf", 0), "times");
    let times_i = have(add("times_i", "timesi.ttf", 0), "times_i");
    let times_b = have(add("times_b", "timesbd.ttf", 0), "times_b");
    let math = have(add("math", "cambria.ttc", 1), "math");
    let sym = have(add("sym", "seguisym.ttf", 0), "sym");
    let arial = have(add("arial", "arial.ttf", 0), "arial");
    let arial_i = have(add("arial_i", "ariali.ttf", 0), "arial_i");
    let arial_b = have(add("arial_b", "arialbd.ttf", 0), "arial_b");

    let defaults: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let fallbacks: Vec<String> = [sym.clone(), math.clone()].into_iter().flatten().collect();
    let chain = |first: Option<String>| -> Vec<String> {
        let mut v: Vec<String> = first.into_iter().collect();
        v.extend(defaults.iter().cloned());
        v.extend(fallbacks.iter().cloned());
        v
    };
    fonts.families.insert(FontFamily::Proportional, chain(segoe.clone()));
    fonts.families.insert(FontFamily::Name("sans_b".into()), chain(segoe_b.or(segoe.clone())));
    fonts.families.insert(FontFamily::Name("sans_i".into()), chain(segoe_i.or(segoe.clone())));
    let serif_chain = |first: Option<String>| -> Vec<String> {
        let mut v: Vec<String> = first.into_iter().collect();
        v.extend(math.clone());
        v.extend(defaults.iter().cloned());
        v.extend(sym.clone());
        v
    };
    fonts.families.insert(FontFamily::Name("serif".into()), serif_chain(times.clone()));
    fonts.families.insert(FontFamily::Name("serif_i".into()), serif_chain(times_i.or(times.clone())));
    fonts.families.insert(FontFamily::Name("serif_b".into()), serif_chain(times_b.or(times)));
    fonts.families.insert(FontFamily::Name("arial".into()), chain(arial.clone().or(segoe.clone())));
    fonts.families.insert(FontFamily::Name("arial_i".into()), chain(arial_i.or(arial.clone()).or(segoe.clone())));
    fonts.families.insert(FontFamily::Name("arial_b".into()), chain(arial_b.or(arial).or(segoe.clone())));
    if let Some(m) = fonts.families.get_mut(&FontFamily::Monospace) {
        m.extend(fallbacks.iter().cloned());
    }
    ctx.set_fonts(fonts);
}
