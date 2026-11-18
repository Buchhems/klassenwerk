//! Formulas from the old formula editor: they are stored as embedded objects
//! whose picture is a Windows metafile. This module reads that picture into
//! simple drawing commands that can be painted on screen or written as SVG.

use crate::delphi::cp1252;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use std::io::Read;

#[derive(Clone, Debug)]
pub enum Prim {
    Lines {
        pts: Vec<Pos2>,
        width: f32,
        color: Color32,
    },
    Polygon {
        pts: Vec<Pos2>,
        fill: Option<Color32>,
        stroke: Option<(f32, Color32)>,
    },
    Text {
        /// x positions of each char (logical units), y = reference line
        xs: Vec<f32>,
        y: f32,
        chars: Vec<char>,
        size: f32,
        italic: bool,
        bold: bool,
        serif: bool,
        color: Color32,
        /// 0 = top, 1 = baseline, 2 = bottom
        valign: u8,
        /// 0 = left, 1 = center, 2 = right
        halign: u8,
        has_dx: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Drawing {
    pub org: Pos2,
    pub ext: Vec2,
    pub prims: Vec<Prim>,
}

/// Extracts the metafile picture from an embedded object (compound file).
pub fn wmf_from_ole(data: &[u8]) -> Option<Vec<u8>> {
    let sig = [0xD0u8, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    let start = crate::delphi::find_bytes(data, &sig)?;
    let cur = std::io::Cursor::new(data[start..].to_vec());
    let mut comp = cfb::CompoundFile::open(cur).ok()?;
    let mut names = Vec::new();
    for e in comp.walk() {
        if e.is_stream() {
            names.push(e.path().to_path_buf());
        }
    }
    let pres = names.iter().find(|p| p.file_name().map(|n| n.to_string_lossy().contains("OlePres")).unwrap_or(false))?;
    let mut buf = Vec::new();
    comp.open_stream(pres).ok()?.read_to_end(&mut buf).ok()?;
    pres_to_wmf(&buf)
}

fn rd_u32(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(p..p + 4)?.try_into().ok()?))
}
fn rd_i16(b: &[u8], p: usize) -> i16 {
    b.get(p..p + 2).map(|s| i16::from_le_bytes([s[0], s[1]])).unwrap_or(0)
}
fn rd_u16(b: &[u8], p: usize) -> u16 {
    rd_i16(b, p) as u16
}

fn pres_to_wmf(b: &[u8]) -> Option<Vec<u8>> {
    let mut p = 0;
    let marker = rd_u32(b, p)?;
    p += 4;
    if marker == 0xFFFF_FFFF || marker == 0xFFFF_FFFE {
        p += 4;
    } else if marker != 0 {
        p += marker as usize;
    }
    let td = rd_u32(b, p)? as usize;
    p += td.max(4);
    p += 4 * 4; // aspect, lindex, advf, reserved
    p += 8; // width, height
    let size = rd_u32(b, p)? as usize;
    p += 4;
    let data = b.get(p..(p + size).min(b.len()))?;
    // optional placeable header
    if data.len() > 22 && rd_u32(data, 0)? == 0x9AC6_CDD7 {
        return Some(data[22..].to_vec());
    }
    Some(data.to_vec())
}

#[derive(Clone)]
enum Obj {
    Pen { width: f32, color: Color32, null: bool },
    Brush { color: Color32, null: bool },
    Font { height: f32, italic: bool, bold: bool, face: String, symbol: bool },
    Other,
}

#[derive(Clone)]
struct DcState {
    pen: (f32, Color32, bool),
    brush: (Color32, bool),
    font: (f32, bool, bool, String, bool),
    text_color: Color32,
    align: u16,
    org: Pos2,
    ext: Vec2,
}

fn colorref(b: &[u8], p: usize) -> Color32 {
    Color32::from_rgb(*b.get(p).unwrap_or(&0), *b.get(p + 1).unwrap_or(&0), *b.get(p + 2).unwrap_or(&0))
}

pub fn parse_wmf(w: &[u8]) -> Option<Drawing> {
    if w.len() < 18 {
        return None;
    }
    let header_words = rd_u16(w, 2) as usize;
    let mut p = header_words.max(9) * 2;
    let mut objs: Vec<Option<Obj>> = Vec::new();
    let mut st = DcState {
        pen: (1.0, Color32::BLACK, false),
        brush: (Color32::WHITE, false),
        font: (12.0, false, false, "Times New Roman".into(), false),
        text_color: Color32::BLACK,
        align: 0,
        org: Pos2::ZERO,
        ext: Vec2::new(1000.0, 1000.0),
    };
    let mut have_ext = false;
    let mut stack: Vec<DcState> = Vec::new();
    let mut cur = Pos2::ZERO;
    let mut prims = Vec::new();
    let mut minmax: Option<Rect> = None;
    let grow = |r: &mut Option<Rect>, p: Pos2| {
        *r = Some(match r {
            Some(rr) => rr.union(Rect::from_min_max(p, p)),
            None => Rect::from_min_max(p, p),
        })
    };

    let add_obj = |objs: &mut Vec<Option<Obj>>, o: Obj| {
        if let Some(slot) = objs.iter_mut().find(|s| s.is_none()) {
            *slot = Some(o);
        } else {
            objs.push(Some(o));
        }
    };

    while p + 6 <= w.len() {
        let size_rec = rd_u32(w, p)? as usize * 2;
        let func = rd_u16(w, p + 4);
        if size_rec < 6 {
            break;
        }
        let a = p + 6; // params
        let end = (p + size_rec).min(w.len());
        match func {
            0x0000 => break,
            0x020B => {
                st.org = Pos2::new(rd_i16(w, a + 2) as f32, rd_i16(w, a) as f32);
            }
            0x020C => {
                st.ext = Vec2::new(rd_i16(w, a + 2) as f32, rd_i16(w, a) as f32);
                have_ext = true;
            }
            0x02FB => {
                let h = rd_i16(w, a) as f32;
                let weight = rd_i16(w, a + 8);
                let italic = w.get(a + 10).copied().unwrap_or(0) != 0;
                let charset = w.get(a + 13).copied().unwrap_or(0);
                let face_bytes = &w[(a + 18).min(end)..end];
                let face_end = face_bytes.iter().position(|&c| c == 0).unwrap_or(face_bytes.len());
                let face = cp1252(&face_bytes[..face_end]);
                let symbol = charset == 2 || face.eq_ignore_ascii_case("Symbol");
                if std::env::var("KLASSENWERK_FONTS").is_ok() {
                    println!("      font h={h} it={italic} w={weight} cs={charset} face={face}");
                }
                add_obj(&mut objs, Obj::Font { height: h, italic, bold: weight >= 600, face, symbol });
            }
            0x02FA => {
                let style = rd_u16(w, a);
                let width = rd_i16(w, a + 2) as f32;
                add_obj(&mut objs, Obj::Pen { width, color: colorref(w, a + 6), null: style & 0xF == 5 });
            }
            0x02FC => {
                let style = rd_u16(w, a);
                add_obj(&mut objs, Obj::Brush { color: colorref(w, a + 2), null: style == 1 });
            }
            0x00F7 | 0x0142 | 0x01F9 | 0x06FF => add_obj(&mut objs, Obj::Other),
            0x012D => {
                let idx = rd_u16(w, a) as usize;
                if let Some(Some(o)) = objs.get(idx) {
                    match o.clone() {
                        Obj::Pen { width, color, null } => st.pen = (width, color, null),
                        Obj::Brush { color, null } => st.brush = (color, null),
                        Obj::Font { height, italic, bold, face, symbol } => st.font = (height, italic, bold, face, symbol),
                        Obj::Other => {}
                    }
                }
            }
            0x01F0 => {
                let idx = rd_u16(w, a) as usize;
                if let Some(s) = objs.get_mut(idx) {
                    *s = None;
                }
            }
            0x001E => stack.push(st.clone()),
            0x0127 => {
                if let Some(s) = stack.pop() {
                    st = s;
                }
            }
            0x0209 => st.text_color = colorref(w, a),
            0x012E => st.align = rd_u16(w, a),
            0x0214 => cur = Pos2::new(rd_i16(w, a + 2) as f32, rd_i16(w, a) as f32),
            0x0213 => {
                let np = Pos2::new(rd_i16(w, a + 2) as f32, rd_i16(w, a) as f32);
                if !st.pen.2 {
                    prims.push(Prim::Lines { pts: vec![cur, np], width: st.pen.0.max(1.0), color: st.pen.1 });
                    grow(&mut minmax, cur);
                    grow(&mut minmax, np);
                }
                cur = np;
            }
            0x0325 | 0x0324 => {
                let n = rd_i16(w, a).max(0) as usize;
                let mut pts = Vec::with_capacity(n);
                for i in 0..n {
                    let q = a + 2 + i * 4;
                    if q + 4 > end {
                        break;
                    }
                    let pt = Pos2::new(rd_i16(w, q) as f32, rd_i16(w, q + 2) as f32);
                    grow(&mut minmax, pt);
                    pts.push(pt);
                }
                if func == 0x0325 {
                    if !st.pen.2 {
                        prims.push(Prim::Lines { pts, width: st.pen.0.max(1.0), color: st.pen.1 });
                    }
                } else {
                    prims.push(Prim::Polygon {
                        pts,
                        fill: if st.brush.1 { None } else { Some(st.brush.0) },
                        stroke: if st.pen.2 { None } else { Some((st.pen.0.max(1.0), st.pen.1)) },
                    });
                }
            }
            0x0538 => {
                let np = rd_i16(w, a).max(0) as usize;
                let counts: Vec<usize> = (0..np).map(|i| rd_i16(w, a + 2 + i * 2).max(0) as usize).collect();
                let mut q = a + 2 + np * 2;
                for c in counts {
                    let mut pts = Vec::new();
                    for _ in 0..c {
                        if q + 4 > end {
                            break;
                        }
                        let pt = Pos2::new(rd_i16(w, q) as f32, rd_i16(w, q + 2) as f32);
                        grow(&mut minmax, pt);
                        pts.push(pt);
                        q += 4;
                    }
                    prims.push(Prim::Polygon {
                        pts,
                        fill: if st.brush.1 { None } else { Some(st.brush.0) },
                        stroke: if st.pen.2 { None } else { Some((st.pen.0.max(1.0), st.pen.1)) },
                    });
                }
            }
            0x041B | 0x0418 => {
                let b = rd_i16(w, a) as f32;
                let r = rd_i16(w, a + 2) as f32;
                let t = rd_i16(w, a + 4) as f32;
                let l = rd_i16(w, a + 6) as f32;
                let pts = vec![Pos2::new(l, t), Pos2::new(r, t), Pos2::new(r, b), Pos2::new(l, b)];
                prims.push(Prim::Polygon {
                    pts,
                    fill: if st.brush.1 { None } else { Some(st.brush.0) },
                    stroke: if st.pen.2 { None } else { Some((st.pen.0.max(1.0), st.pen.1)) },
                });
            }
            0x0521 | 0x0A32 => {
                let (x, y, bytes, dx): (f32, f32, Vec<u8>, Option<Vec<f32>>);
                if func == 0x0521 {
                    let n = rd_i16(w, a).max(0) as usize;
                    let sb = w.get(a + 2..(a + 2 + n).min(end)).unwrap_or(&[]).to_vec();
                    let q = a + 2 + ((n + 1) & !1);
                    y = rd_i16(w, q) as f32;
                    x = rd_i16(w, q + 2) as f32;
                    bytes = sb;
                    dx = None;
                } else {
                    y = rd_i16(w, a) as f32;
                    x = rd_i16(w, a + 2) as f32;
                    let n = rd_i16(w, a + 4).max(0) as usize;
                    let opts = rd_u16(w, a + 6);
                    let mut q = a + 8;
                    if opts & 0x0006 != 0 {
                        q += 8;
                    }
                    bytes = w.get(q..(q + n).min(end)).unwrap_or(&[]).to_vec();
                    q += (n + 1) & !1;
                    if q + n * 2 <= end && n > 0 {
                        dx = Some((0..n).map(|i| rd_i16(w, q + i * 2) as f32).collect());
                    } else {
                        dx = None;
                    }
                }
                let (h, italic, bold, face, symbol) = st.font.clone();
                let chars: Vec<char> = if symbol {
                    bytes.iter().map(|&b| symbol_char(b)).collect()
                } else if face.to_ascii_lowercase().contains("mt extra") {
                    bytes.iter().map(|&b| mt_extra_char(b)).collect()
                } else {
                    cp1252(&bytes).chars().collect()
                };
                let size = if h < 0.0 { -h } else { h * 0.82 };
                let has_dx = dx.is_some();
                let mut xs = Vec::with_capacity(chars.len());
                let mut cx = x;
                for i in 0..chars.len() {
                    xs.push(cx);
                    cx += match &dx {
                        Some(d) => d.get(i).copied().unwrap_or(size * 0.5),
                        None => size * 0.5,
                    };
                }
                let valign = if st.align & 24 == 24 {
                    1
                } else if st.align & 8 != 0 {
                    2
                } else {
                    0
                };
                let halign = if st.align & 6 == 6 {
                    1
                } else if st.align & 2 != 0 {
                    2
                } else {
                    0
                };
                grow(&mut minmax, Pos2::new(x, y - size));
                grow(&mut minmax, Pos2::new(cx, y + size * 0.3));
                let serif = !face.to_ascii_lowercase().contains("arial");
                // Big operators and stretchy delimiters are drawn from pieces of the
                // symbol font; system fonts render those pieces poorly, so draw them as shapes.
                let mut chars = chars;
                if symbol {
                    let baseline = match valign {
                        1 => y,
                        2 => y - size * 0.22,
                        _ => y + size * 0.80,
                    };
                    for (i, &b) in bytes.iter().enumerate() {
                        if i >= chars.len() {
                            break;
                        }
                        let shapes = vector_glyph(b, xs[i], baseline, size, st.text_color);
                        if !shapes.is_empty() {
                            grow(&mut minmax, Pos2::new(xs[i], baseline - size * 0.95));
                            grow(&mut minmax, Pos2::new(xs[i] + size * 0.4, baseline + size * 0.25));
                            prims.extend(shapes);
                            chars[i] = ' ';
                        }
                    }
                    if chars.iter().all(|c| *c == ' ') {
                        p += size_rec;
                        continue;
                    }
                }
                prims.push(Prim::Text { xs, y, chars, size, italic, bold, serif, color: st.text_color, valign, halign, has_dx });
            }
            _ => {}
        }
        p += size_rec;
    }
    let (org, ext) = if have_ext && st.ext.x != 0.0 && st.ext.y != 0.0 {
        (st.org, st.ext)
    } else if let Some(r) = minmax {
        (r.min, r.size().max(Vec2::splat(1.0)))
    } else {
        (Pos2::ZERO, Vec2::splat(1.0))
    };
    Some(Drawing { org, ext, prims })
}

fn quad(p0: (f32, f32), c: (f32, f32), p2: (f32, f32), n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            (u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p2.0, u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p2.1)
        })
        .collect()
}

/// Shapes for the symbol-font characters that are pieces of big operators or
/// stretchy delimiters (integral, bracket and parenthesis pieces). Coordinates
/// are in em relative to the character origin on the baseline (y grows downward).
/// Returns nothing for ordinary characters.
pub(crate) fn vector_glyph(b: u8, x: f32, base: f32, size: f32, color: Color32) -> Vec<Prim> {
    // Proportions follow the symbol font's glyph boxes: pieces span from 0.926 em
    // above the baseline to 0.081 em below it, strokes are about 0.047 em wide.
    const TOP: f32 = -0.926;
    const BOT: f32 = 0.081;
    let stroke = 0.047;
    let map = |pts: &[(f32, f32)]| -> Vec<Pos2> { pts.iter().map(|&(px, py)| Pos2::new(x + px * size, base + py * size)).collect() };
    let line = |pts: &[(f32, f32)], wd: f32| Prim::Lines { pts: map(pts), width: wd * size, color };
    let dot = |cx: f32, cy: f32, r: f32| {
        let pts: Vec<(f32, f32)> = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        Prim::Polygon { pts: map(&pts), fill: Some(color), stroke: None }
    };
    // a stroke that is thin at both ends and thick in the middle
    let tapered = |c: &[(f32, f32)], thin: f32, thick: f32| -> Vec<Prim> {
        let n = c.len() - 1;
        (0..n)
            .map(|i| {
                let t = (i as f32 + 0.5) / n as f32;
                line(&[c[i], c[i + 1]], thin + (thick - thin) * (1.0 - (2.0 * t - 1.0).powi(2)))
            })
            .collect()
    };
    match b {
        // ∫ : a whole integral sign, 0.273 em wide, from 0.91 em above to 0.107 em below the baseline
        0xF2 => {
            let mut c = quad((0.255, -0.855), (0.205, -0.955), (0.157, -0.72), 6);
            c.extend(quad((0.157, -0.72), (0.138, -0.31), (0.118, 0.0), 10).into_iter().skip(1));
            c.extend(quad((0.118, 0.0), (0.100, 0.115), (0.018, 0.055), 6).into_iter().skip(1));
            let mut v = tapered(&c, 0.028, 0.078);
            v.push(dot(0.243, -0.855, 0.04));
            v.push(dot(0.030, 0.055, 0.04));
            v
        }
        // ⌠ ⎮ ⌡ : integral pieces for tall integrals (stems aligned)
        0xF3 => {
            let mut c = quad((0.36, -0.86), (0.24, -0.99), (0.20, -0.70), 8);
            c.push((0.20, BOT));
            vec![line(&c, 0.07), dot(0.345, -0.86, 0.04)]
        }
        0xF4 => vec![line(&[(0.20, TOP), (0.20, BOT)], 0.07)],
        0xF5 => {
            let mut c = vec![(0.20, TOP)];
            c.extend(quad((0.20, -0.10), (0.16, 0.14), (0.05, 0.0), 8));
            vec![line(&c, 0.07), dot(0.06, 0.0, 0.04)]
        }
        // ⎡ ⎢ ⎣  /  ⎤ ⎥ ⎦
        0xE9 => vec![line(&[(0.343, TOP + 0.024), (0.024, TOP + 0.024), (0.024, BOT)], stroke)],
        0xEA => vec![line(&[(0.024, TOP), (0.024, BOT)], stroke)],
        0xEB => vec![line(&[(0.024, TOP), (0.024, BOT - 0.024), (0.343, BOT - 0.024)], stroke)],
        0xF9 => vec![line(&[(0.017, TOP + 0.024), (0.336, TOP + 0.024), (0.336, BOT)], stroke)],
        0xFA => vec![line(&[(0.336, TOP), (0.336, BOT)], stroke)],
        0xFB => vec![line(&[(0.336, TOP), (0.336, BOT - 0.024), (0.017, BOT - 0.024)], stroke)],
        // ⎛ ⎜ ⎝  /  ⎞ ⎟ ⎠
        0xE6 => vec![line(&quad((0.437, TOP), (0.065, TOP + 0.30), (0.065, BOT), 12), stroke)],
        0xE7 => vec![line(&[(0.065, TOP), (0.065, BOT)], stroke)],
        0xE8 => vec![line(&quad((0.065, TOP), (0.065, BOT - 0.30), (0.437, BOT), 12), stroke)],
        0xF6 => vec![line(&quad((0.054, TOP), (0.427, TOP + 0.30), (0.427, BOT), 12), stroke)],
        0xF7 => vec![line(&[(0.427, TOP), (0.427, BOT)], stroke)],
        0xF8 => vec![line(&quad((0.427, TOP), (0.427, BOT - 0.30), (0.054, BOT), 12), stroke)],
        _ => Vec::new(),
    }
}

pub fn symbol_char(b: u8) -> char {
    const LOW: &str = " !∀#∃%&∋()∗+,−./0123456789:;<=>?≅ΑΒΧΔΕΦΓΗΙϑΚΛΜΝΟΠΘΡΣΤΥςΩΞΨΖ[∴]⊥_‾αβχδεφγηιϕκλμνοπθρστυϖωξψζ{|}∼";
    const HIGH: &str = "€ϒ′≤⁄∞ƒ♣♦♥♠↔←↑→↓°±″≥×∝∂•÷≠≡≈…⏐⎯↵ℵℑℜ℘⊗⊕∅∩∪⊃⊇⊄⊂⊆∈∉∠∇®©™∏√⋅¬∧∨⇔⇐⇑⇒⇓◊〈®©™∑⎛⎜⎝⎡⎢⎣⎧⎨⎩⎪ 〉∫⌠⎮⌡⎞⎟⎠⎤⎥⎦⎫⎬⎭ ";
    if (0x20..0x7F).contains(&b) {
        LOW.chars().nth((b - 0x20) as usize).unwrap_or('?')
    } else if b >= 0xA0 {
        HIGH.chars().nth((b - 0xA0) as usize).unwrap_or('?')
    } else {
        b as char
    }
}

fn mt_extra_char(b: u8) -> char {
    match b {
        0x4C => '…',
        0x4D => '⋯',
        0x4E => '⋮',
        0x4F => '⋱',
        0x5C => '∥',
        0x6C => 'ℓ',
        0x3B => ' ',
        0x31 => '↼',
        0x22 => '▷',
        0x60 => '‵',
        0xA3 => '←',
        0xA4 => '→',
        _ => cp1252(&[b]).chars().next().unwrap_or(' '),
    }
}

/// Paints a drawing into `rect`.
pub fn paint(d: &Drawing, painter: &egui::Painter, rect: Rect, ink: Option<Color32>) {
    let sx = rect.width() / d.ext.x;
    let sy = rect.height() / d.ext.y;
    let tr = |p: Pos2| Pos2::new(rect.min.x + (p.x - d.org.x) * sx, rect.min.y + (p.y - d.org.y) * sy);
    let recolor = |c: Color32| -> Color32 {
        match ink {
            Some(i) if c.r() < 60 && c.g() < 60 && c.b() < 60 => i,
            _ => c,
        }
    };
    let lw = |w: f32| (w * sx.abs()).max(1.0);
    for p in &d.prims {
        match p {
            Prim::Lines { pts, width, color } => {
                let pts: Vec<Pos2> = pts.iter().map(|&q| tr(q)).collect();
                painter.add(egui::Shape::line(pts, Stroke::new(lw(*width), recolor(*color))));
            }
            Prim::Polygon { pts, fill, stroke } => {
                let pts: Vec<Pos2> = pts.iter().map(|&q| tr(q)).collect();
                let is_white = matches!(fill, Some(c) if c.r() > 240 && c.g() > 240 && c.b() > 240);
                if is_white && stroke.is_none() {
                    continue; // background rectangle
                }
                let f = fill.filter(|_| !is_white).map(recolor).unwrap_or(Color32::TRANSPARENT);
                let s = stroke.map(|(w, c)| Stroke::new(lw(w), recolor(c))).unwrap_or(Stroke::NONE);
                painter.add(egui::Shape::convex_polygon(pts, f, s));
            }
            Prim::Text { xs, y, chars, size, italic, bold, serif, color, valign, halign, has_dx } => {
                let px = (size * sy.abs()).max(4.0);
                let fam = crate::fonts::formula_family(*serif, *italic, *bold);
                let font = egui::FontId::new(px * crate::fonts::EM_TO_SIZE, fam);
                let col = recolor(*color);
                let anchor_y = match valign {
                    1 => egui::Align::BOTTOM,
                    2 => egui::Align::BOTTOM,
                    _ => egui::Align::TOP,
                };
                // baseline handling: egui places the top of the row; shift by ascent
                let base = |yy: f32| -> Pos2 {
                    let p = tr(Pos2::new(0.0, yy));
                    match valign {
                        1 => Pos2::new(0.0, p.y - px * crate::fonts::ASCENT),
                        2 => Pos2::new(0.0, p.y - px * 1.1),
                        _ => Pos2::new(0.0, p.y),
                    }
                };
                let _ = anchor_y;
                let top = base(*y).y;
                if *has_dx || *halign == 0 {
                    if *has_dx {
                        for (i, c) in chars.iter().enumerate() {
                            if *c == ' ' {
                                continue;
                            }
                            let x = tr(Pos2::new(xs[i], 0.0)).x;
                            painter.text(Pos2::new(x, top), egui::Align2::LEFT_TOP, c.to_string(), font.clone(), col);
                        }
                    } else {
                        let s: String = chars.iter().collect();
                        let x = tr(Pos2::new(xs[0], 0.0)).x;
                        painter.text(Pos2::new(x, top), egui::Align2::LEFT_TOP, s, font, col);
                    }
                } else {
                    let s: String = chars.iter().collect();
                    let x = tr(Pos2::new(xs.first().copied().unwrap_or(0.0), 0.0)).x;
                    let al = if *halign == 1 { egui::Align2::CENTER_TOP } else { egui::Align2::RIGHT_TOP };
                    painter.text(Pos2::new(x, top), al, s, font, col);
                }
            }
        }
    }
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Writes the drawing as an inline SVG of the given pixel size.
pub fn to_svg(d: &Drawing, w: f32, h: f32) -> String {
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"none\" style=\"vertical-align:middle\">",
        d.org.x, d.org.y, d.ext.x, d.ext.y
    );
    for p in &d.prims {
        match p {
            Prim::Lines { pts, width, color } => {
                let pts: Vec<String> = pts.iter().map(|q| format!("{},{}", q.x, q.y)).collect();
                s += &format!("<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>", pts.join(" "), hex(*color), width.max(1.0));
            }
            Prim::Polygon { pts, fill, stroke } => {
                let is_white = matches!(fill, Some(c) if c.r() > 240 && c.g() > 240 && c.b() > 240);
                if is_white && stroke.is_none() {
                    continue;
                }
                let pts: Vec<String> = pts.iter().map(|q| format!("{},{}", q.x, q.y)).collect();
                let f = fill.filter(|_| !is_white).map(hex).unwrap_or_else(|| "none".into());
                let st = stroke.map(|(w, c)| format!("stroke=\"{}\" stroke-width=\"{}\"", hex(c), w)).unwrap_or_default();
                s += &format!("<polygon points=\"{}\" fill=\"{}\" {}/>", pts.join(" "), f, st);
            }
            Prim::Text { xs, y, chars, size, italic, bold, serif, color, valign, halign, has_dx } => {
                let fam = if *serif { "'Times New Roman',Cambria,serif" } else { "Arial,sans-serif" };
                let base = match valign {
                    1 => "alphabetic",
                    2 => "text-after-edge",
                    _ => "text-before-edge",
                };
                let anchor = match halign {
                    1 => "middle",
                    2 => "end",
                    _ => "start",
                };
                let xattr =
                    if *has_dx { xs.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(" ") } else { format!("{}", xs.first().copied().unwrap_or(0.0)) };
                let text: String = chars.iter().collect();
                s += &format!(
                    "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" fill=\"{}\" dominant-baseline=\"{}\" text-anchor=\"{}\"{}{} xml:space=\"preserve\">{}</text>",
                    xattr,
                    y,
                    fam,
                    size,
                    hex(*color),
                    base,
                    anchor,
                    if *italic { " font-style=\"italic\"" } else { "" },
                    if *bold { " font-weight=\"bold\"" } else { "" },
                    esc(&text)
                );
            }
        }
    }
    s += "</svg>";
    s
}
