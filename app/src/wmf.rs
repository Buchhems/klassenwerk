//! Writes a drawing (`formula::Drawing`) as a Windows metafile, the picture
//! format Word and the old program use for formulas. Used for new formulas in
//! RTF and DOCX files. Text that the normal font cannot encode (Greek letters,
//! ∞, ≤ …) is written in the Symbol font, like the old formula editor did.

use crate::formula::{Drawing, Prim};
use egui::Color32;

struct W {
    recs: Vec<u8>,
    max_rec: u32,
    n_objects: u16,
}

impl W {
    fn rec(&mut self, func: u16, params: &[u16]) {
        let size = 3 + params.len() as u32;
        self.max_rec = self.max_rec.max(size);
        self.recs.extend_from_slice(&size.to_le_bytes());
        self.recs.extend_from_slice(&func.to_le_bytes());
        for p in params {
            self.recs.extend_from_slice(&p.to_le_bytes());
        }
    }
}

fn words(bytes: &[u8]) -> Vec<u16> {
    let mut b = bytes.to_vec();
    if b.len() % 2 == 1 {
        b.push(0);
    }
    b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn i(v: f32) -> u16 {
    (v.round().clamp(-32768.0, 32767.0) as i16) as u16
}

fn colorref(c: Color32) -> [u16; 2] {
    let v = c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16;
    [(v & 0xFFFF) as u16, (v >> 16) as u16]
}

/// cp1252 byte for a char, if it has one
fn ansi(c: char) -> Option<u8> {
    let u = c as u32;
    if u < 0x80 || (0xA0..=0xFF).contains(&u) {
        return Some(u as u8);
    }
    let hi = [
        ('€', 0x80),
        ('‚', 0x82),
        ('ƒ', 0x83),
        ('„', 0x84),
        ('…', 0x85),
        ('†', 0x86),
        ('‡', 0x87),
        ('ˆ', 0x88),
        ('‰', 0x89),
        ('‘', 0x91),
        ('’', 0x92),
        ('“', 0x93),
        ('”', 0x94),
        ('•', 0x95),
        ('–', 0x96),
        ('—', 0x97),
        ('™', 0x99),
    ];
    hi.iter().find(|(a, _)| *a == c).map(|(_, b)| *b as u8)
}

/// Symbol font byte for a char (inverse of `formula::symbol_char`)
fn symbol(c: char) -> Option<u8> {
    (0x20u8..=0xFE).find(|b| crate::formula::symbol_char(*b) == c)
}

/// A placeable metafile of the drawing; `units_per_inch` maps drawing units to paper.
pub fn from_drawing(d: &Drawing, units_per_inch: u16) -> Vec<u8> {
    let mut w = W { recs: Vec::new(), max_rec: 0, n_objects: 0 };
    let (ox, oy) = (d.org.x, d.org.y);
    let (ex, ey) = (d.ext.x, d.ext.y);
    w.rec(0x0103, &[8]); // SETMAPMODE MM_ANISOTROPIC
    w.rec(0x020B, &[i(oy), i(ox)]); // SETWINDOWORG y, x
    w.rec(0x020C, &[i(ey), i(ex)]); // SETWINDOWEXT y, x
    w.rec(0x0102, &[1]); // SETBKMODE TRANSPARENT
    w.rec(0x012E, &[24]); // SETTEXTALIGN TA_BASELINE | TA_LEFT
                          // object slots: each created object takes the lowest free slot; it is deleted right after use
    for p in &d.prims {
        match p {
            Prim::Lines { pts, width, color } => {
                let c = colorref(*color);
                w.rec(0x02FA, &[0, i(*width), 0, c[0], c[1]]); // CREATEPENINDIRECT
                w.rec(0x012D, &[0]); // SELECTOBJECT 0
                let mut params = vec![pts.len() as u16];
                for q in pts {
                    params.push(i(q.x));
                    params.push(i(q.y));
                }
                w.rec(0x0325, &params); // POLYLINE
                w.rec(0x01F0, &[0]); // DELETEOBJECT
                w.n_objects = w.n_objects.max(1);
            }
            Prim::Polygon { pts, fill, stroke } => {
                let fc = colorref(fill.unwrap_or(Color32::BLACK));
                // BS_SOLID or BS_NULL
                w.rec(0x02FC, &[if fill.is_some() { 0 } else { 1 }, fc[0], fc[1], 0]); // CREATEBRUSHINDIRECT
                w.rec(0x012D, &[0]);
                let (sw, sc) = stroke.unwrap_or((0.0, fill.unwrap_or(Color32::BLACK)));
                let c = colorref(sc);
                w.rec(0x02FA, &[if stroke.is_some() { 0 } else { 5 }, i(sw), 0, c[0], c[1]]); // PS_SOLID or PS_NULL
                w.rec(0x012D, &[1]);
                let mut params = vec![pts.len() as u16];
                for q in pts {
                    params.push(i(q.x));
                    params.push(i(q.y));
                }
                w.rec(0x0324, &params); // POLYGON
                w.rec(0x01F0, &[1]);
                w.rec(0x01F0, &[0]);
                w.n_objects = w.n_objects.max(2);
            }
            Prim::Text { xs, y, chars, size, italic, bold, serif, color, halign, .. } => {
                let c = colorref(*color);
                w.rec(0x0209, &[c[0], c[1]]); // SETTEXTCOLOR
                                              // TA_BASELINE plus TA_LEFT / TA_CENTER / TA_RIGHT
                w.rec(
                    0x012E,
                    &[24 | match halign {
                        1 => 6,
                        2 => 2,
                        _ => 0,
                    }],
                );
                for (k, ch) in chars.iter().enumerate() {
                    if *ch == ' ' {
                        continue;
                    }
                    let (face, cs, byte) = match ansi(*ch) {
                        Some(b) => (if *serif { "Times New Roman" } else { "Arial" }, 0u8, b),
                        None => match symbol(*ch) {
                            Some(b) => ("Symbol", 2u8, b),
                            None => (if *serif { "Times New Roman" } else { "Arial" }, 0u8, b'?'),
                        },
                    };
                    // LOGFONT: height (negative = character height), width, escapement, orientation, weight,
                    // italic/underline, strikeout/charset, out/clip precision, quality/pitch, face name
                    let it = if *italic && cs == 0 { 1u16 } else { 0 };
                    let mut lf = vec![i(-size), 0, 0, 0, if *bold { 700 } else { 400 }, it, (cs as u16) << 8, 0, 0];
                    let mut name = face.as_bytes().to_vec();
                    name.resize(32, 0);
                    lf.extend(words(&name));
                    w.rec(0x02FB, &lf); // CREATEFONTINDIRECT
                    w.rec(0x012D, &[0]);
                    let x = xs.get(k).copied().unwrap_or(xs.first().copied().unwrap_or(0.0));
                    let mut params = vec![1u16];
                    params.extend(words(&[byte]));
                    params.push(i(*y));
                    params.push(i(x));
                    w.rec(0x0521, &params); // TEXTOUT count, string, y, x
                    w.rec(0x01F0, &[0]);
                    w.n_objects = w.n_objects.max(1);
                }
            }
        }
    }
    w.rec(0x0000, &[]); // EOF

    let mut out = Vec::new();
    // placeable header
    let left = i(ox);
    let top = i(oy);
    let right = i(ox + ex);
    let bottom = i(oy + ey);
    let hdr: [u16; 10] = [0xCDD7, 0x9AC6, 0, left, top, right, bottom, units_per_inch, 0, 0];
    let checksum = hdr.iter().fold(0u16, |a, b| a ^ b);
    for v in hdr {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&checksum.to_le_bytes());
    // standard header
    let total_words = 9 + w.recs.len() / 2;
    out.extend_from_slice(&1u16.to_le_bytes()); // memory metafile
    out.extend_from_slice(&9u16.to_le_bytes());
    out.extend_from_slice(&0x0300u16.to_le_bytes());
    out.extend_from_slice(&(total_words as u32).to_le_bytes());
    out.extend_from_slice(&w.n_objects.max(1).to_le_bytes());
    out.extend_from_slice(&w.max_rec.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&w.recs);
    out
}

/// Metafile of a new formula (1 em = 12 pt).
#[cfg(test)]
pub fn from_math(src: &str) -> Vec<u8> {
    from_drawing(&crate::math::layout(src), (crate::math::U * 6.0) as u16)
}
