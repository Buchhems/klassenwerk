//! Synonyms from a MyThes thesaurus (th_de_DE_v2.dat), the format of the
//! OpenThesaurus files that come with LibreOffice and most Linux systems.
//! Like the "Thesaurus" of the original's editor; the file is not shipped.
//!
//! Format: first line the encoding (UTF-8 or ISO8859-1), then entries
//! `word|n` followed by n meaning lines `(part of speech)|synonym|synonym…`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One meaning of a word: part of speech (may be empty) and its synonyms.
pub type Meaning = (String, Vec<String>);

pub struct Thesaurus {
    pub name: String,
    map: HashMap<String, Vec<Meaning>>,
}

impl Thesaurus {
    pub fn load(p: &Path) -> Result<Thesaurus, String> {
        let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        Ok(Self::parse(&bytes, name))
    }

    pub fn parse(bytes: &[u8], name: String) -> Thesaurus {
        let first_end = bytes.iter().position(|b| *b == b'\n').unwrap_or(bytes.len());
        let enc = String::from_utf8_lossy(&bytes[..first_end]).trim().to_uppercase();
        let text: String = if enc.starts_with("UTF") {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            // ISO8859-1 / -15: bytes are code points (good enough for German)
            bytes.iter().map(|b| *b as char).collect()
        };
        let mut map: HashMap<String, Vec<Meaning>> = HashMap::new();
        let mut lines = text.lines().skip(1);
        while let Some(head) = lines.next() {
            let Some((word, n)) = head.rsplit_once('|') else { continue };
            let Ok(n) = n.trim().parse::<usize>() else { continue };
            let mut meanings = Vec::new();
            for _ in 0..n {
                let Some(l) = lines.next() else { break };
                let mut parts = l.split('|');
                let pos = parts.next().unwrap_or("").trim().trim_start_matches('(').trim_end_matches(')').to_string();
                let syns: Vec<String> = parts.map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != word).collect();
                if !syns.is_empty() {
                    meanings.push((pos, syns));
                }
            }
            if !meanings.is_empty() {
                map.entry(word.trim().to_lowercase()).or_default().extend(meanings);
            }
        }
        Thesaurus { name, map }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Meanings of a word: the word itself, otherwise a base form found by
    /// taking off a usual German ending (Häuser → Haus is not found, Hunden → Hund is).
    pub fn lookup(&self, word: &str) -> Vec<Meaning> {
        let w = word.trim().to_lowercase();
        if let Some(m) = self.map.get(&w) {
            return m.clone();
        }
        for end in ["en", "em", "er", "es", "e", "n", "s", "st", "t", "et", "te", "ten"] {
            if let Some(stem) = w.strip_suffix(end) {
                if stem.chars().count() >= 3 {
                    for base in [stem.to_string(), format!("{stem}en"), format!("{stem}n")] {
                        if let Some(m) = self.map.get(&base) {
                            return m.clone();
                        }
                    }
                }
            }
        }
        Vec::new()
    }
}

/// A synonym written like the word it replaces (capital first letter at the start of a sentence).
pub fn match_case(original: &str, syn: &str) -> String {
    let up = original.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
    let mut c = syn.chars();
    match c.next() {
        Some(f) if up && f.is_lowercase() => f.to_uppercase().collect::<String>() + c.as_str(),
        _ => syn.to_string(),
    }
}

/// Installed German thesauri (LibreOffice, Linux).
pub fn candidates() -> Vec<PathBuf> {
    let names = ["th_de_DE_v2.dat", "th_de_DE.dat", "th_de_AT_v2.dat", "th_de_CH_v2.dat", "th_de_DE_v2_utf8.dat"];
    let mut out = Vec::new();
    for d in crate::spell::dict_dirs() {
        for n in names {
            let p = d.join(n);
            if p.exists() {
                out.push(p);
            }
        }
    }
    out
}

/// The thesaurus to use: the chosen file, otherwise the first one found.
pub fn find(chosen: &str) -> Option<PathBuf> {
    let c = chosen.trim();
    if !c.is_empty() {
        let p = PathBuf::from(c);
        // the .idx file was chosen: the data is next to it
        return Some(if p.extension().map(|e| e == "idx").unwrap_or(false) { p.with_extension("dat") } else { p });
    }
    candidates().into_iter().next()
}
