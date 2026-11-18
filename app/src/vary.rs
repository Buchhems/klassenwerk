//! Task variants with random numbers (Aufgabenvarianten). Not in the original.
//!
//! A task defines variables with a range (e.g. a from 2 to 9, step 1). In the
//! task text, the solution and in formulas, `{=a}` stands for the value and
//! `{=2*a+b}` for a computed value; `{=a/3;2}` rounds to two decimals. Every
//! sheet has its own seed, so the printed sheet and its solution use the same
//! numbers, and Gruppe B gets other numbers than Gruppe A.

use crate::model::{Doc, ObjKind, Task};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Var {
    /// name used in the text: letters, digits, "_" (starting with a letter)
    pub name: String,
    pub min: f64,
    pub max: f64,
    /// distance between two possible values (1 = whole numbers, 0,5, 0,1 …)
    pub step: f64,
}

impl Default for Var {
    fn default() -> Self {
        Var { name: "a".into(), min: 1.0, max: 10.0, step: 1.0 }
    }
}

impl Var {
    /// How many values the variable can take.
    pub fn count(&self) -> u64 {
        if self.step <= 0.0 || self.max < self.min {
            1
        } else {
            ((self.max - self.min) / self.step + 1e-9).floor() as u64 + 1
        }
    }
}

fn mix(mut z: u64) -> u64 {
    // splitmix64
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn hash_str(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// Decimals a step needs: 1 → 0, 0,5 → 1, 0,25 → 2.
fn decimals_of(step: f64) -> usize {
    (0..6)
        .find(|d| {
            let f = 10f64.powi(*d as i32);
            ((step * f).round() - step * f).abs() < 1e-9
        })
        .unwrap_or(6)
}

/// Values of the task's variables for a seed (same seed, same values).
pub fn values(task: &Task, seed: u64) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    for v in &task.vars {
        let n = v.count();
        let r = mix(seed ^ mix(task.id) ^ hash_str(&v.name));
        let k = r % n;
        let x = if n <= 1 { v.min } else { v.min + k as f64 * v.step };
        let f = 10f64.powi(decimals_of(v.step) as i32);
        out.insert(v.name.clone(), (x * f).round() / f);
    }
    out
}

// ---------------------------------------------------------------- expressions

struct P<'a> {
    s: Vec<char>,
    i: usize,
    vars: &'a HashMap<String, f64>,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).map(|c| c.is_whitespace()).unwrap_or(false) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: char) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') || self.eat('−') {
                v -= self.term()?;
            } else {
                return Ok(v);
            }
        }
    }
    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.power()?;
        loop {
            if self.eat('*') || self.eat('·') {
                v *= self.power()?;
            } else if self.eat('/') || self.eat(':') {
                let d = self.power()?;
                if d == 0.0 {
                    return Err("Division durch 0".into());
                }
                v /= d;
            } else {
                return Ok(v);
            }
        }
    }
    fn power(&mut self) -> Result<f64, String> {
        let b = self.unary()?;
        if self.eat('^') {
            let e = self.power()?;
            return Ok(b.powf(e));
        }
        Ok(b)
    }
    fn unary(&mut self) -> Result<f64, String> {
        if self.eat('-') || self.eat('−') {
            return Ok(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.atom()
    }
    fn atom(&mut self) -> Result<f64, String> {
        self.ws();
        if self.eat('(') {
            let v = self.expr()?;
            if !self.eat(')') {
                return Err("„)“ fehlt".into());
            }
            return Ok(v);
        }
        let start = self.i;
        let c = *self.s.get(self.i).ok_or("Ausdruck endet zu früh")?;
        if c.is_ascii_digit() || c == '.' || c == ',' {
            while self.s.get(self.i).map(|c| c.is_ascii_digit() || *c == '.' || *c == ',').unwrap_or(false) {
                self.i += 1;
            }
            let t: String = self.s[start..self.i].iter().collect();
            return t.replace(',', ".").parse::<f64>().map_err(|_| format!("Zahl „{t}“"));
        }
        if c.is_alphabetic() {
            while self.s.get(self.i).map(|c| c.is_alphanumeric() || *c == '_').unwrap_or(false) {
                self.i += 1;
            }
            let name: String = self.s[start..self.i].iter().collect();
            if self.eat('(') {
                let mut args = vec![self.expr()?];
                while self.eat(';') {
                    args.push(self.expr()?);
                }
                if !self.eat(')') {
                    return Err("„)“ fehlt".into());
                }
                return func(&name, &args);
            }
            return match name.as_str() {
                "pi" => Ok(std::f64::consts::PI),
                "e" if !self.vars.contains_key("e") => Ok(std::f64::consts::E),
                _ => self.vars.get(&name).copied().ok_or(format!("unbekannte Variable „{name}“")),
            };
        }
        Err(format!("unerwartetes Zeichen „{c}“"))
    }
}

fn func(name: &str, a: &[f64]) -> Result<f64, String> {
    let x = a[0];
    let deg = |v: f64| v.to_radians();
    Ok(match (name, a.len()) {
        ("sqrt" | "wurzel", 1) => {
            if x < 0.0 {
                return Err("Wurzel aus einer negativen Zahl".into());
            }
            x.sqrt()
        }
        ("abs" | "betrag", 1) => x.abs(),
        ("sin", 1) => deg(x).sin(),
        ("cos", 1) => deg(x).cos(),
        ("tan", 1) => deg(x).tan(),
        ("ln", 1) => x.ln(),
        ("lg" | "log", 1) => x.log10(),
        ("exp", 1) => x.exp(),
        ("round" | "runden", 1) => x.round(),
        ("round" | "runden", 2) => {
            let f = 10f64.powi(a[1] as i32);
            (x * f).round() / f
        }
        ("floor", 1) => x.floor(),
        ("ceil", 1) => x.ceil(),
        ("min", _) => a.iter().copied().fold(f64::INFINITY, f64::min),
        ("max", _) => a.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        _ => return Err(format!("unbekannte Funktion „{name}“")),
    })
}

/// Computes an expression such as `2*a + b^2` or `sqrt(a^2 + b^2)` (angles in degrees;
/// arguments separated by ";", e.g. `round(a/3; 2)`).
pub fn eval(expr: &str, vars: &HashMap<String, f64>) -> Result<f64, String> {
    let mut p = P { s: expr.chars().collect(), i: 0, vars };
    let v = p.expr()?;
    p.ws();
    if p.i < p.s.len() {
        return Err(format!("unerwartetes Zeichen „{}“", p.s[p.i]));
    }
    if !v.is_finite() {
        return Err("kein Zahlenwert".into());
    }
    Ok(v)
}

/// A number as printed: decimal comma, at most `max_dec` decimals, no trailing zeros.
pub fn fmt_num(v: f64, max_dec: usize) -> String {
    let s = format!("{:.*}", max_dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = if s == "-0" { "0".to_string() } else { s };
    s.replace('.', ",")
}

/// Replaces every `{=expression}` (or `{=expression;decimals}`) in a text.
/// Errors are shown in the text as „⟨?…⟩“, so they are noticed in the preview.
pub fn apply(text: &str, vars: &HashMap<String, f64>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(p) = rest.find("{=") {
        out += &rest[..p];
        let after = &rest[p + 2..];
        // the matching "}" (braces inside the expression are not allowed)
        match after.find('}') {
            Some(end) => {
                let inner = &after[..end];
                // "{=a/3;2}": decimals after the last ";" outside parentheses
                let balanced = |e: &str| e.matches('(').count() == e.matches(')').count();
                let (expr, dec) = match inner.rsplit_once(';') {
                    Some((e, d)) if balanced(e) && d.trim().parse::<usize>().is_ok() => (e, d.trim().parse::<usize>().unwrap_or(4).min(8)),
                    _ => (inner, 4),
                };
                match eval(expr, vars) {
                    Ok(v) => out += &fmt_num(v, dec),
                    Err(e) => out += &format!("⟨?{}: {e}⟩", expr.trim()),
                }
                rest = &after[end + 1..];
            }
            None => {
                out += "{=";
                rest = after;
            }
        }
    }
    out += rest;
    out
}

/// Problems in the `{=…}` parts of a text, for the editor (with example values).
pub fn problems(text: &str, vars: &HashMap<String, f64>) -> Vec<String> {
    let done = apply(text, vars);
    let mut out = Vec::new();
    let mut rest = done.as_str();
    while let Some(p) = rest.find("⟨?") {
        let after = &rest[p + "⟨?".len()..];
        let end = after.find('⟩').unwrap_or(after.len());
        out.push(after[..end].to_string());
        rest = &after[end..];
    }
    out
}

/// A document with its `{=…}` replaced, also inside formulas.
pub fn apply_doc(doc: &Doc, vars: &HashMap<String, f64>) -> Doc {
    let mut d = doc.clone();
    d.text = apply(&d.text, vars);
    for o in d.objects.iter_mut().filter(|o| o.kind == ObjKind::Math) {
        let src = String::from_utf8_lossy(&o.data).to_string();
        if src.contains("{=") {
            let new = apply(&src, vars);
            let lay = crate::math::layout(&new);
            let (w, h) = crate::math::size_px(&lay);
            o.data = new.into_bytes();
            o.width = w;
            o.height = h;
        }
    }
    d
}

impl Task {
    /// The task with numbers for a sheet (seed); unchanged without variables.
    pub fn varied(&self, seed: u64) -> Task {
        if self.vars.is_empty() && !self.task.text.contains("{=") && !self.solution.text.contains("{=") {
            return self.clone();
        }
        let v = values(self, seed);
        let mut t = self.clone();
        t.task = apply_doc(&self.task, &v);
        t.solution = apply_doc(&self.solution, &v);
        for p in &mut t.parts {
            p.expectation = apply(&p.expectation, &v);
        }
        t
    }
}
