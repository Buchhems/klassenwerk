//! Spell checking with Hunspell dictionaries (.aff/.dic), the format used by
//! LibreOffice, Firefox and most Linux systems. Not in the original program.
//!
//! The dictionary is not shipped: Klassenwerk looks for an installed German
//! dictionary (LibreOffice, Linux) or uses the file chosen in the settings.
//!
//! The checker reads the common parts of the format: prefixes and suffixes with
//! their conditions (also a second suffix through continuation classes), flag
//! types (single letters, "long", "num", UTF-8), flag aliases (AF), forbidden
//! words, NEEDAFFIX and ONLYINCOMPOUND, and compound words through
//! COMPOUNDFLAG / COMPOUNDBEGIN / COMPOUNDMIDDLE / COMPOUNDEND. Compound rules
//! (COMPOUNDRULE), REP tables and morphology are not used. To avoid false
//! alarms on German compounds, a long word made of known words (with a joining
//! "s") is also accepted. When in doubt, a word counts as correct.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

type Flag = u32;

#[derive(Clone, Copy, PartialEq)]
enum FlagMode {
    Char,
    Long,
    Num,
}

#[derive(Clone, Debug)]
enum CondPart {
    Any,
    Char(char),
    Set(Vec<char>, bool),
}

#[derive(Clone, Debug)]
struct Affix {
    flag: Flag,
    cross: bool,
    strip: String,
    cond: Vec<CondPart>,
    /// continuation flags: what may follow this affix (a second suffix, compound flags)
    cont: Vec<Flag>,
}

fn cond_parse(s: &str) -> Vec<CondPart> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '.' => out.push(CondPart::Any),
            '[' => {
                let mut j = i + 1;
                let neg = c.get(j) == Some(&'^');
                if neg {
                    j += 1;
                }
                let mut set = Vec::new();
                while j < c.len() && c[j] != ']' {
                    set.push(c[j]);
                    j += 1;
                }
                out.push(CondPart::Set(set, neg));
                i = j;
            }
            ch => out.push(CondPart::Char(ch)),
        }
        i += 1;
    }
    if out.len() == 1 && matches!(out[0], CondPart::Any) {
        out.clear();
    }
    out
}

fn cond_match_char(p: &CondPart, ch: char) -> bool {
    match p {
        CondPart::Any => true,
        CondPart::Char(c) => *c == ch,
        CondPart::Set(set, neg) => set.contains(&ch) != *neg,
    }
}

/// Does the condition match the start (prefix) or the end (suffix) of the stem?
fn cond_ok(cond: &[CondPart], stem: &[char], suffix: bool) -> bool {
    if cond.is_empty() {
        return true;
    }
    if cond.len() > stem.len() {
        return false;
    }
    let part = if suffix { &stem[stem.len() - cond.len()..] } else { &stem[..cond.len()] };
    cond.iter().zip(part).all(|(p, c)| cond_match_char(p, *c))
}

pub struct Dict {
    words: HashMap<String, Vec<Vec<Flag>>>,
    /// suffix rules by the text they add (reversed lookup from the word end)
    sfx: HashMap<String, Vec<Affix>>,
    pfx: HashMap<String, Vec<Affix>>,
    max_sfx: usize,
    max_pfx: usize,
    forbidden: Option<Flag>,
    needaffix: Option<Flag>,
    onlyincompound: Option<Flag>,
    compound: Option<Flag>,
    compound_begin: Option<Flag>,
    compound_middle: Option<Flag>,
    compound_end: Option<Flag>,
    compound_min: usize,
    try_chars: Vec<char>,
    /// lower-case forms of all words, for the compound fallback
    lower: HashSet<String>,
    pub name: String,
}

fn decode(bytes: &[u8], latin: bool) -> String {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if !latin {
        if let Ok(s) = std::str::from_utf8(b) {
            return s.to_string();
        }
    }
    crate::delphi::cp1252(b)
}

impl Dict {
    /// Reads a dictionary: the .dic file with the .aff file next to it.
    pub fn load(dic: &Path) -> Result<Dict, String> {
        let aff_path = dic.with_extension("aff");
        let aff_bytes = std::fs::read(&aff_path).map_err(|e| format!("{}: {e}", aff_path.display()))?;
        let dic_bytes = std::fs::read(dic).map_err(|e| format!("{}: {e}", dic.display()))?;
        // the character set comes from the SET line of the .aff file
        let head = String::from_utf8_lossy(&aff_bytes[..aff_bytes.len().min(4000)]).to_string();
        let set = head.lines().find_map(|l| l.trim().strip_prefix("SET ")).map(|s| s.trim().to_uppercase()).unwrap_or_default();
        let latin = !set.is_empty() && set != "UTF-8";
        let name = dic.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        Ok(Dict::parse(&decode(&aff_bytes, latin), &decode(&dic_bytes, latin), name))
    }

    pub fn parse(aff: &str, dic: &str, name: String) -> Dict {
        let mut d = Dict {
            words: HashMap::new(),
            sfx: HashMap::new(),
            pfx: HashMap::new(),
            max_sfx: 0,
            max_pfx: 0,
            forbidden: None,
            needaffix: None,
            onlyincompound: None,
            compound: None,
            compound_begin: None,
            compound_middle: None,
            compound_end: None,
            compound_min: 3,
            try_chars: vec![],
            lower: HashSet::new(),
            name,
        };
        let mut mode = FlagMode::Char;
        let mut aliases: Vec<Vec<Flag>> = Vec::new();
        // first pass: the flag type, so flags in the rules are read correctly
        for l in aff.lines() {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.first() == Some(&"FLAG") {
                mode = match t.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
                    Some("long") => FlagMode::Long,
                    Some("num") => FlagMode::Num,
                    _ => FlagMode::Char,
                };
            }
        }
        let flags = |s: &str| -> Vec<Flag> {
            match mode {
                FlagMode::Char => s.chars().map(|c| c as u32).collect(),
                FlagMode::Long => {
                    let c: Vec<char> = s.chars().collect();
                    c.chunks(2).map(|p| (p[0] as u32) << 16 | p.get(1).map(|x| *x as u32).unwrap_or(0)).collect()
                }
                FlagMode::Num => s.split(',').filter_map(|n| n.trim().parse().ok()).collect(),
            }
        };
        let one = |s: Option<&&str>| s.and_then(|x| flags(x).first().copied());
        let mut lines = aff.lines().peekable();
        let mut af_seen = false;
        while let Some(l) = lines.next() {
            let t: Vec<&str> = l.split_whitespace().collect();
            match t.first().copied() {
                Some("AF") if t.len() >= 2 => {
                    // the first AF line is the count
                    if !af_seen && t[1].parse::<usize>().is_ok() && t.len() == 2 {
                        af_seen = true;
                    } else {
                        aliases.push(flags(t[1]));
                    }
                }
                Some("FORBIDDENWORD") => d.forbidden = one(t.get(1)),
                Some("NEEDAFFIX") | Some("PSEUDOROOT") => d.needaffix = one(t.get(1)),
                Some("ONLYINCOMPOUND") => d.onlyincompound = one(t.get(1)),
                Some("COMPOUNDFLAG") => d.compound = one(t.get(1)),
                Some("COMPOUNDBEGIN") => d.compound_begin = one(t.get(1)),
                Some("COMPOUNDMIDDLE") => d.compound_middle = one(t.get(1)),
                Some("COMPOUNDEND") | Some("COMPOUNDLAST") => d.compound_end = one(t.get(1)),
                Some("COMPOUNDMIN") => d.compound_min = t.get(1).and_then(|n| n.parse().ok()).unwrap_or(3).max(1),
                Some("TRY") => d.try_chars = t.get(1).map(|s| s.chars().collect()).unwrap_or_default(),
                Some(kind @ ("SFX" | "PFX")) if t.len() >= 4 => {
                    // header "SFX X Y 4", then the rules
                    let Some(flag) = one(t.get(1)) else { continue };
                    let cross = t[2] == "Y";
                    let n: usize = t[3].parse().unwrap_or(0);
                    for _ in 0..n {
                        let Some(r) = lines.next() else { break };
                        let r: Vec<&str> = r.split_whitespace().collect();
                        if r.len() < 4 || r[0] != kind {
                            continue;
                        }
                        let strip = if r[2] == "0" { String::new() } else { r[2].to_string() };
                        let (add, cont) = match r[3].split_once('/') {
                            Some((a, c)) => {
                                let cont = if !aliases.is_empty() && c.chars().all(|x| x.is_ascii_digit()) {
                                    c.parse::<usize>().ok().and_then(|i| aliases.get(i.wrapping_sub(1)).cloned()).unwrap_or_default()
                                } else {
                                    flags(c)
                                };
                                (a, cont)
                            }
                            None => (r[3], vec![]),
                        };
                        let add = if add == "0" { String::new() } else { add.to_string() };
                        let cond = cond_parse(r.get(4).copied().unwrap_or("."));
                        let a = Affix { flag, cross, strip, cond, cont };
                        if kind == "SFX" {
                            d.max_sfx = d.max_sfx.max(add.chars().count());
                            d.sfx.entry(add).or_default().push(a);
                        } else {
                            d.max_pfx = d.max_pfx.max(add.chars().count());
                            d.pfx.entry(add).or_default().push(a);
                        }
                    }
                }
                _ => {}
            }
        }
        for (k, l) in dic.lines().enumerate() {
            if k == 0 && l.trim().chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let tok = l.split(['\t', ' ']).next().unwrap_or("").trim();
            if tok.is_empty() || tok.starts_with('#') {
                continue;
            }
            // "word/flags"; a "/" inside the word is written as "\/"
            let (word, fl) = match tok.rfind('/').filter(|p| *p > 0 && !tok[..*p].ends_with('\\')) {
                Some(p) => (&tok[..p], &tok[p + 1..]),
                None => (tok, ""),
            };
            let word = word.replace("\\/", "/");
            let f = if !aliases.is_empty() && !fl.is_empty() && fl.chars().all(|c| c.is_ascii_digit()) {
                fl.parse::<usize>().ok().and_then(|i| aliases.get(i.wrapping_sub(1)).cloned()).unwrap_or_default()
            } else {
                flags(fl)
            };
            d.lower.insert(word.to_lowercase());
            d.words.entry(word).or_default().push(f);
        }
        d
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    fn has(fl: &[Flag], f: Option<Flag>) -> bool {
        f.map(|f| fl.contains(&f)).unwrap_or(false)
    }

    /// The word as it stands in the dictionary, without affixes.
    fn root(&self, w: &str, compound: bool) -> bool {
        self.words.get(w).map(|v| v.iter().any(|fl| self.root_ok(fl, compound))).unwrap_or(false)
    }

    fn root_ok(&self, fl: &[Flag], compound: bool) -> bool {
        !Dict::has(fl, self.forbidden) && !Dict::has(fl, self.needaffix) && (compound || !Dict::has(fl, self.onlyincompound))
    }

    fn forbidden(&self, w: &str) -> bool {
        self.words.get(w).map(|v| v.iter().any(|fl| Dict::has(fl, self.forbidden))).unwrap_or(false)
    }

    /// Stems for which `w` = stem + suffix, with the suffix rule.
    fn suffix_stems<'a>(&'a self, w: &[char]) -> Vec<(String, &'a Affix)> {
        let mut out = Vec::new();
        for k in 0..=self.max_sfx.min(w.len().saturating_sub(1)) {
            let add: String = w[w.len() - k..].iter().collect();
            let Some(rules) = self.sfx.get(&add) else { continue };
            let base: String = w[..w.len() - k].iter().collect();
            for r in rules {
                let stem = format!("{base}{}", r.strip);
                let sc: Vec<char> = stem.chars().collect();
                if cond_ok(&r.cond, &sc, true) {
                    out.push((stem, r));
                }
            }
        }
        out
    }

    fn prefix_stems<'a>(&'a self, w: &[char]) -> Vec<(String, &'a Affix)> {
        let mut out = Vec::new();
        for k in 0..=self.max_pfx.min(w.len().saturating_sub(1)) {
            let add: String = w[..k].iter().collect();
            let Some(rules) = self.pfx.get(&add) else { continue };
            let base: String = w[k..].iter().collect();
            for r in rules {
                let stem = format!("{}{base}", r.strip);
                let sc: Vec<char> = stem.chars().collect();
                if cond_ok(&r.cond, &sc, false) {
                    out.push((stem, r));
                }
            }
        }
        out
    }

    /// A dictionary entry `stem` carries `flag` (and, if given, `also`).
    fn stem_has(&self, stem: &str, flag: Flag, also: Option<Flag>, compound: bool) -> bool {
        self.words
            .get(stem)
            .map(|v| {
                v.iter().any(|fl| {
                    fl.contains(&flag)
                        && also.map(|a| fl.contains(&a)).unwrap_or(true)
                        && !Dict::has(fl, self.forbidden)
                        && (compound || !Dict::has(fl, self.onlyincompound))
                })
            })
            .unwrap_or(false)
    }

    /// The word with affixes: one suffix, two suffixes (continuation), a prefix,
    /// or a prefix and a suffix.
    fn affixed(&self, w: &str, compound: bool) -> bool {
        let wc: Vec<char> = w.chars().collect();
        for (stem, r) in self.suffix_stems(&wc) {
            if self.stem_has(&stem, r.flag, None, compound) {
                return true;
            }
            // a second suffix: stem = root + suffix1, suffix1 allows r (continuation)
            let sc: Vec<char> = stem.chars().collect();
            for (root, r1) in self.suffix_stems(&sc) {
                if r1.cont.contains(&r.flag) && self.stem_has(&root, r1.flag, None, compound) {
                    return true;
                }
            }
        }
        for (stem, p) in self.prefix_stems(&wc) {
            if self.stem_has(&stem, p.flag, None, compound) {
                return true;
            }
            if p.cross {
                let sc: Vec<char> = stem.chars().collect();
                for (root, s) in self.suffix_stems(&sc) {
                    if s.cross && self.stem_has(&root, s.flag, Some(p.flag), compound) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn simple(&self, w: &str) -> bool {
        !self.forbidden(w) && (self.root(w, false) || self.affixed(w, false))
    }

    /// A part of a compound at a position (0 = first, 1 = middle, 2 = last).
    fn compound_part(&self, w: &str, pos: u8) -> bool {
        let pos_flag = match pos {
            0 => self.compound_begin,
            1 => self.compound_middle,
            _ => self.compound_end,
        };
        let ok = |fl: &[Flag]| Dict::has(fl, self.compound) || Dict::has(fl, pos_flag);
        let direct = |x: &str| self.words.get(x).map(|v| v.iter().any(|fl| ok(fl) && !Dict::has(fl, self.forbidden))).unwrap_or(false);
        if direct(w) {
            return true;
        }
        // affixed forms whose affix or root carries the compound flag
        let wc: Vec<char> = w.chars().collect();
        for (stem, r) in self.suffix_stems(&wc).into_iter().chain(self.prefix_stems(&wc)) {
            let rule_ok = r.cont.iter().any(|f| Some(*f) == self.compound || Some(*f) == pos_flag);
            if let Some(v) = self.words.get(&stem) {
                if v.iter().any(|fl| fl.contains(&r.flag) && (rule_ok || ok(fl)) && !Dict::has(fl, self.forbidden)) {
                    return true;
                }
            }
        }
        false
    }

    fn has_compounding(&self) -> bool {
        self.compound.is_some() || self.compound_begin.is_some() || self.compound_end.is_some()
    }

    /// Compound words from parts marked for compounding (up to 4 parts).
    fn compound(&self, w: &[char], pos: u8, depth: u8) -> bool {
        let min = self.compound_min.max(2);
        if depth > 3 || w.len() < 2 * min {
            return false;
        }
        for i in min..=w.len() - min {
            let first: String = w[..i].iter().collect();
            let rest = &w[i..];
            let firsts = if pos == 0 { vec![first.clone()] } else { vec![first.clone(), capitalize(&first)] };
            if !firsts.iter().any(|f| self.compound_part(f, pos)) {
                continue;
            }
            let last: String = rest.iter().collect();
            if [last.clone(), capitalize(&last)].iter().any(|l| self.compound_part(l, 2)) {
                return true;
            }
            if self.compound(rest, 1, depth + 1) {
                return true;
            }
        }
        false
    }

    /// Fallback for long German compounds: every part (at least 3 letters) is a
    /// known word in any form, parts may be joined by "s".
    fn loose_compound(&self, w: &str) -> bool {
        let l: Vec<char> = w.to_lowercase().chars().collect();
        fn go(d: &Dict, l: &[char], depth: u8) -> bool {
            if depth > 3 {
                return false;
            }
            if l.len() >= 3 && (d.lower.contains(&l.iter().collect::<String>()) || d.simple_lower(l)) {
                return true;
            }
            for i in 3..l.len().saturating_sub(2) {
                let head: String = l[..i].iter().collect();
                if !d.lower.contains(&head) {
                    continue;
                }
                let mut rests = vec![&l[i..]];
                if l[i] == 's' && l.len() - i > 3 {
                    rests.push(&l[i + 1..]);
                }
                if rests.into_iter().any(|r| go(d, r, depth + 1)) {
                    return true;
                }
            }
            false
        }
        l.len() >= 8 && go(self, &l, 0)
    }

    fn simple_lower(&self, l: &[char]) -> bool {
        let s: String = l.iter().collect();
        self.simple(&s) || self.simple(&capitalize(&s))
    }

    fn check_form(&self, w: &str) -> bool {
        if self.simple(w) {
            return true;
        }
        if self.has_compounding() {
            let c: Vec<char> = w.chars().collect();
            if self.compound(&c, 0, 0) {
                return true;
            }
        }
        false
    }

    /// Is the word spelled correctly? Capitals at the start of a sentence and
    /// words in capitals are accepted when the normal form is known.
    pub fn check(&self, w: &str) -> bool {
        if w.chars().count() < 2 || w.chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
        if self.forbidden(w) {
            return false;
        }
        if self.check_form(w) {
            return true;
        }
        let lower = w.to_lowercase();
        let first_upper = w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        let all_upper = w.chars().all(|c| !c.is_lowercase());
        if first_upper && self.check_form(&decapitalize(w)) {
            return true;
        }
        if all_upper && (self.check_form(&lower) || self.check_form(&capitalize(&lower))) {
            return true;
        }
        self.loose_compound(w)
    }

    /// Up to `n` suggestions: one letter wrong, missing, too much or swapped,
    /// plus capitalisation.
    pub fn suggest(&self, w: &str, n: usize) -> Vec<String> {
        let c: Vec<char> = w.chars().collect();
        let mut alphabet: Vec<char> = self.try_chars.iter().copied().filter(|c| c.is_alphabetic()).collect();
        for ch in "enisratdhulcgmobwfkzpvüäößjyxq".chars() {
            if !alphabet.contains(&ch) {
                alphabet.push(ch);
            }
        }
        let mut out: Vec<String> = Vec::new();
        let push = |s: String, out: &mut Vec<String>| {
            if s != w && !out.contains(&s) && self.check_form(&s) {
                out.push(s);
            }
        };
        push(capitalize(w), &mut out);
        push(decapitalize(w), &mut out);
        for i in 0..c.len() {
            // swapped neighbours
            if i + 1 < c.len() {
                let mut x = c.clone();
                x.swap(i, i + 1);
                push(x.iter().collect(), &mut out);
            }
            // one letter too much
            let mut x = c.clone();
            x.remove(i);
            push(x.iter().collect(), &mut out);
        }
        for i in 0..=c.len() {
            for &a in &alphabet {
                // a wrong letter
                if i < c.len() && c[i] != a {
                    let mut x = c.clone();
                    x[i] = if i == 0 && c[0].is_uppercase() { a.to_uppercase().next().unwrap_or(a) } else { a };
                    push(x.iter().collect(), &mut out);
                }
                // a missing letter
                let mut x = c.clone();
                x.insert(i, a);
                push(x.iter().collect(), &mut out);
            }
            if out.len() >= n * 3 {
                break;
            }
        }
        // the most similar first: same first letter, same length
        out.sort_by_key(|s| (s.chars().next() != c.first().copied(), (s.chars().count() as i64 - c.len() as i64).abs()));
        out.truncate(n);
        out
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn decapitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Where German dictionaries are usually installed (LibreOffice, Linux, macOS).
pub fn candidates() -> Vec<PathBuf> {
    let names = ["de_DE_frami.dic", "de_DE.dic", "de-DE.dic", "de_AT_frami.dic", "de_CH_frami.dic", "de_AT.dic", "de_CH.dic"];
    let mut out = Vec::new();
    for d in dict_dirs() {
        for n in names {
            let p = d.join(n);
            if p.exists() && p.with_extension("aff").exists() {
                out.push(p);
            }
        }
    }
    out
}

/// Folders where LibreOffice and Linux keep dictionaries and thesauri.
pub fn dict_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(p) = std::env::var(var) {
            for lo in ["LibreOffice", "LibreOffice 7", "LibreOffice 24", "LibreOffice 25", "OpenOffice 4", "OpenOffice.org 3"] {
                dirs.push(PathBuf::from(&p).join(lo).join("share").join("extensions").join("dict-de"));
                dirs.push(PathBuf::from(&p).join(lo).join("share").join("extension").join("install"));
            }
        }
    }
    if let Ok(h) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&h).join("Library").join("Spelling"));
        dirs.push(PathBuf::from(&h).join(".local").join("share").join("hunspell"));
    }
    for d in [
        "/Applications/LibreOffice.app/Contents/Resources/extensions/dict-de",
        "/usr/share/hunspell",
        "/usr/share/myspell",
        "/usr/share/myspell/dicts",
        "/usr/share/mythes",
        "/usr/lib/libreoffice/share/extensions/dict-de",
        "/opt/libreoffice/share/extensions/dict-de",
        "/snap/libreoffice/current/lib/libreoffice/share/extensions/dict-de",
    ] {
        dirs.push(PathBuf::from(d));
    }
    dirs
}

/// The dictionary to use: the chosen file, otherwise the first one found.
pub fn find(chosen: &str) -> Option<PathBuf> {
    let c = chosen.trim();
    if !c.is_empty() {
        let p = PathBuf::from(c);
        let p = if p.extension().map(|e| e == "aff").unwrap_or(false) { p.with_extension("dic") } else { p };
        return Some(p);
    }
    candidates().into_iter().next()
}

/// Words of a text to check, as (start, end) char positions. Formulas ($…$),
/// objects ({{n}}), computed values ({=…}), links and words with digits are skipped.
pub fn words(text: &str) -> Vec<(usize, usize)> {
    let c: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\\' {
            // an escaped character, or a word right after it (e.g. "\frac")
            i += 2;
            while i < c.len() && c[i].is_alphabetic() {
                i += 1;
            }
            continue;
        }
        if ch == '$' && c.get(i + 1).map(|n| !n.is_whitespace()).unwrap_or(false) {
            if let Some(j) = (i + 1..c.len()).find(|&j| c[j] == '$' && c[j - 1] != '\\') {
                i = j + 1;
                continue;
            }
        }
        if ch == '{' && matches!(c.get(i + 1), Some('{') | Some('=')) {
            if let Some(j) = (i + 2..c.len()).find(|&j| c[j] == '}') {
                i = j + 1;
                continue;
            }
        }
        if ch == '[' && c.get(i + 1) == Some(&'[') {
            if let Some(j) = (i + 2..c.len()).find(|&j| c[j] == ']') {
                i = j + 1;
                continue;
            }
        }
        if ch.is_alphabetic() {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || (c[i] == '\'' && c.get(i + 1).map(|n| n.is_alphabetic()).unwrap_or(false))) {
                i += 1;
            }
            let w: String = c[start..i].iter().collect();
            // links, mail addresses and file names
            let next = c.get(i).copied().unwrap_or(' ');
            let link = w.starts_with("http") || w == "www" || next == '@' || (next == '.' && c.get(i + 1).map(|n| n.is_alphanumeric()).unwrap_or(false));
            let prev = if start > 0 { c[start - 1] } else { ' ' };
            if !link && prev != '@' && prev != '/' && !w.chars().any(|x| x.is_ascii_digit()) && w.chars().count() > 1 {
                out.push((start, i));
            }
            if link {
                while i < c.len() && !c[i].is_whitespace() {
                    i += 1;
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

/// The word around a char position (start, end, word), if any.
pub fn word_at(text: &str, pos: usize) -> Option<(usize, usize, String)> {
    let c: Vec<char> = text.chars().collect();
    words(text).into_iter().find(|(a, b)| pos >= *a && pos <= *b).map(|(a, b)| (a, b, c[a..b].iter().collect()))
}

/// Checks words and remembers the results (the editor asks every frame).
#[derive(Default)]
pub struct Cache {
    known: HashMap<String, bool>,
}

impl Cache {
    pub fn ok(&mut self, d: &Dict, user: &HashSet<String>, w: &str) -> bool {
        if user.contains(w) || user.contains(&w.to_lowercase()) {
            return true;
        }
        if let Some(v) = self.known.get(w) {
            return *v;
        }
        let v = d.check(w);
        if self.known.len() > 50_000 {
            self.known.clear();
        }
        self.known.insert(w.to_string(), v);
        v
    }
    pub fn clear(&mut self) {
        self.known.clear();
    }
}
