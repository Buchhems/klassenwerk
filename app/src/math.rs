//! New formulas written by the teacher (formula editor). Not in the original.
//!
//! The source is a small LaTeX-like language, e.g. `\frac{a}{b}`, `x^{2}`,
//! `\sqrt{x}`, `\int_{0}^{1} x^2 \, dx`. It is parsed into a tree, laid out and
//! turned into the same drawing commands as the old formula pictures
//! (`formula::Drawing`), so screen, HTML and Word output all work the same way.
//! The tree is also used for Word's own formula format (OMML) in DOCX files.

use crate::formula::{Drawing, Prim};
use egui::{Color32, Pos2, Vec2};

/// Drawing units per em.
pub const U: f32 = 100.0;
/// Screen pixels per em at the normal text size (12 pt at 96 dpi).
pub const PX_PER_EM: f32 = 16.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Row(Vec<Node>),
    /// a single character: variable (italic), number, operator, symbol
    Sym {
        c: char,
        italic: bool,
        class: Class,
    },
    /// upright word: function name (sin, lim) or \text{…}
    Word(String),
    Frac(Box<Node>, Box<Node>),
    Sqrt {
        body: Box<Node>,
        index: Option<Box<Node>>,
    },
    Scripts {
        base: Box<Node>,
        sub: Option<Box<Node>>,
        sup: Option<Box<Node>>,
    },
    /// ∑ ∏ ∫ ∮ and lim with their limits
    BigOp {
        op: char,
        sub: Option<Box<Node>>,
        sup: Option<Box<Node>>,
    },
    Fenced {
        open: char,
        close: char,
        body: Box<Node>,
    },
    Accent {
        kind: Accent,
        body: Box<Node>,
    },
    Space(f32),
    /// unknown command or syntax error, shown in red
    Error(String),
    /// matrix, case distinction or several aligned lines: rows of cells
    Table {
        kind: TableKind,
        rows: Vec<Vec<Node>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableKind {
    /// matrix with its brackets ('.' = none): pmatrix ( ), bmatrix [ ], vmatrix | |
    Matrix(char, char),
    /// case distinction with a left brace, cells left-aligned
    Cases,
    /// lines aligned at "&" (align/aligned, or "\\" at the top level)
    Aligned,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Class {
    Ord,
    /// binary operator: + − · ×
    Bin,
    /// relation: = < ≤ ⇒
    Rel,
    /// , ;
    Punct,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accent {
    Vec,
    Bar,
    Hat,
    Dot,
}

/// Named symbols: (name, char, class)
const SYMBOLS: &[(&str, char, Class)] = &[
    ("alpha", 'α', Class::Ord),
    ("beta", 'β', Class::Ord),
    ("gamma", 'γ', Class::Ord),
    ("delta", 'δ', Class::Ord),
    ("epsilon", 'ε', Class::Ord),
    ("varepsilon", 'ε', Class::Ord),
    ("zeta", 'ζ', Class::Ord),
    ("eta", 'η', Class::Ord),
    ("theta", 'θ', Class::Ord),
    ("vartheta", 'ϑ', Class::Ord),
    ("iota", 'ι', Class::Ord),
    ("kappa", 'κ', Class::Ord),
    ("lambda", 'λ', Class::Ord),
    ("mu", 'μ', Class::Ord),
    ("nu", 'ν', Class::Ord),
    ("xi", 'ξ', Class::Ord),
    ("pi", 'π', Class::Ord),
    ("rho", 'ρ', Class::Ord),
    ("sigma", 'σ', Class::Ord),
    ("tau", 'τ', Class::Ord),
    ("upsilon", 'υ', Class::Ord),
    ("phi", 'φ', Class::Ord),
    ("varphi", 'ϕ', Class::Ord),
    ("chi", 'χ', Class::Ord),
    ("psi", 'ψ', Class::Ord),
    ("omega", 'ω', Class::Ord),
    ("Gamma", 'Γ', Class::Ord),
    ("Delta", 'Δ', Class::Ord),
    ("Theta", 'Θ', Class::Ord),
    ("Lambda", 'Λ', Class::Ord),
    ("Xi", 'Ξ', Class::Ord),
    ("Pi", 'Π', Class::Ord),
    ("Sigma", 'Σ', Class::Ord),
    ("Phi", 'Φ', Class::Ord),
    ("Psi", 'Ψ', Class::Ord),
    ("Omega", 'Ω', Class::Ord),
    ("cdot", '·', Class::Bin),
    ("times", '×', Class::Bin),
    ("div", '÷', Class::Bin),
    ("pm", '±', Class::Bin),
    ("mp", '∓', Class::Bin),
    ("cup", '∪', Class::Bin),
    ("cap", '∩', Class::Bin),
    ("setminus", '\\', Class::Bin),
    ("circ", '∘', Class::Bin),
    ("le", '≤', Class::Rel),
    ("leq", '≤', Class::Rel),
    ("ge", '≥', Class::Rel),
    ("geq", '≥', Class::Rel),
    ("ne", '≠', Class::Rel),
    ("neq", '≠', Class::Rel),
    ("approx", '≈', Class::Rel),
    ("equiv", '≡', Class::Rel),
    ("sim", '∼', Class::Rel),
    ("to", '→', Class::Rel),
    ("rightarrow", '→', Class::Rel),
    ("leftarrow", '←', Class::Rel),
    ("Rightarrow", '⇒', Class::Rel),
    ("Leftarrow", '⇐', Class::Rel),
    ("Leftrightarrow", '⇔', Class::Rel),
    ("iff", '⇔', Class::Rel),
    ("implies", '⇒', Class::Rel),
    ("in", '∈', Class::Rel),
    ("notin", '∉', Class::Rel),
    ("subset", '⊂', Class::Rel),
    ("subseteq", '⊆', Class::Rel),
    ("perp", '⊥', Class::Rel),
    ("parallel", '∥', Class::Rel),
    ("mapsto", '↦', Class::Rel),
    ("infty", '∞', Class::Ord),
    ("partial", '∂', Class::Ord),
    ("nabla", '∇', Class::Ord),
    ("emptyset", '∅', Class::Ord),
    ("forall", '∀', Class::Ord),
    ("exists", '∃', Class::Ord),
    ("angle", '∠', Class::Ord),
    ("degree", '°', Class::Ord),
    ("triangle", '△', Class::Ord),
    ("ldots", '…', Class::Ord),
    ("dots", '…', Class::Ord),
    ("cdots", '⋯', Class::Ord),
    ("neg", '¬', Class::Ord),
    ("land", '∧', Class::Bin),
    ("lor", '∨', Class::Bin),
    ("prime", '′', Class::Ord),
    ("percent", '%', Class::Ord),
    ("euro", '€', Class::Ord),
    ("{", '{', Class::Ord),
    ("}", '}', Class::Ord),
    ("%", '%', Class::Ord),
    ("#", '#', Class::Ord),
    ("&", '&', Class::Ord),
    ("_", '_', Class::Ord),
    ("$", '$', Class::Ord),
    ("|", '‖', Class::Ord),
];

const FUNCTIONS: &[&str] = &[
    "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "sin", "cos", "tan", "cot", "ln", "log", "lg", "exp", "lim", "max", "min", "det", "sgn", "mod",
    "ggT", "kgV",
];

const BLACKBOARD: &[(char, char)] = &[('R', 'ℝ'), ('N', 'ℕ'), ('Z', 'ℤ'), ('Q', 'ℚ'), ('C', 'ℂ')];

// ------------------------------------------------------------------ parser

struct Parser {
    s: Vec<char>,
    i: usize,
    /// > 0 inside a table: "&" and "\\" end a cell
    env: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    /// "\\" (new line of a table)
    fn at_newline(&self) -> bool {
        self.peek() == Some('\\') && self.s.get(self.i + 1) == Some(&'\\')
    }
    fn at_word(&self, w: &str) -> bool {
        self.peek() == Some('\\') && {
            let n = w.chars().count();
            self.s[self.i + 1..].iter().take(n).collect::<String>() == w && !self.s.get(self.i + 1 + n).map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        }
    }
    /// at the end of a table cell
    fn at_cell_end(&self) -> bool {
        self.env > 0 && (self.peek() == Some('&') || self.at_newline() || self.at_word("end"))
    }
    /// a group (braces, brackets): "&" and "\\" inside belong to it, not to a table around it
    fn group(&mut self, until: Option<char>) -> Node {
        let e = self.env;
        self.env = 0;
        let r = self.row(until);
        self.env = e;
        r
    }
    /// Cells and rows up to "\end" or the end of the source.
    fn table_body(&mut self) -> Vec<Vec<Node>> {
        self.env += 1;
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        loop {
            cells.push(self.row(None));
            self.skip_ws();
            if self.peek() == Some('&') {
                self.i += 1;
            } else if self.at_newline() {
                self.i += 2;
                rows.push(std::mem::take(&mut cells));
            } else {
                break;
            }
        }
        rows.push(cells);
        self.env -= 1;
        // "a \\ b \\" ends with an empty line
        let empty = |r: &Vec<Node>| r.iter().all(|c| matches!(c, Node::Row(v) if v.is_empty()));
        while rows.len() > 1 && rows.last().map(empty).unwrap_or(false) {
            rows.pop();
        }
        rows
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.i += 1;
        }
    }
    fn command(&mut self) -> String {
        // after the backslash
        let start = self.i;
        match self.peek() {
            Some(c) if c.is_ascii_alphabetic() => {
                while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
                    self.i += 1;
                }
            }
            Some(_) => self.i += 1,
            None => {}
        }
        self.s[start..self.i].iter().collect()
    }
    /// one argument: {group} or a single token
    fn arg(&mut self) -> Node {
        self.skip_ws();
        match self.peek() {
            Some('{') => {
                self.i += 1;
                let r = self.group(Some('}'));
                if self.peek() == Some('}') {
                    self.i += 1;
                }
                r
            }
            Some(_) => self.atom().unwrap_or(Node::Row(vec![])),
            None => Node::Error("□".into()),
        }
    }
    fn raw_group(&mut self) -> String {
        self.skip_ws();
        if self.peek() != Some('{') {
            return self
                .peek()
                .map(|c| {
                    self.i += 1;
                    c.to_string()
                })
                .unwrap_or_default();
        }
        self.i += 1;
        let mut depth = 1;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            out.push(c);
        }
        out
    }
    fn row(&mut self, until: Option<char>) -> Node {
        let mut v = Vec::new();
        loop {
            self.skip_ws_keep();
            match self.peek() {
                None => break,
                Some(c) if Some(c) == until => break,
                Some('}') if until.is_none() || until == Some(')') || until == Some(']') => {
                    // stray closing brace
                    self.i += 1;
                    v.push(Node::Error("}".into()));
                }
                _ => {}
            }
            if self.at_right() || self.at_cell_end() {
                break;
            }
            match self.atom() {
                Some(n) => {
                    let n = self.scripts(n);
                    v.push(n);
                }
                None => break,
            }
        }
        if v.len() == 1 {
            v.pop().unwrap()
        } else {
            Node::Row(v)
        }
    }
    /// spaces in the source are ignored (as in LaTeX)
    fn skip_ws_keep(&mut self) {
        self.skip_ws();
    }
    /// "\\right" (but not "\\rightarrow")
    fn at_right(&self) -> bool {
        self.at_word("right")
    }
    fn scripts(&mut self, base: Node) -> Node {
        let (mut sub, mut sup) = (None, None);
        loop {
            self.skip_ws();
            match self.peek() {
                Some('^') if sup.is_none() => {
                    self.i += 1;
                    sup = Some(Box::new(self.arg()));
                }
                Some('_') if sub.is_none() => {
                    self.i += 1;
                    sub = Some(Box::new(self.arg()));
                }
                Some('\'') => {
                    // f' -> prime as superscript
                    self.i += 1;
                    let p = Node::Sym { c: '′', italic: false, class: Class::Ord };
                    sup = Some(Box::new(match sup.take() {
                        Some(s) => Node::Row(vec![*s, p]),
                        None => p,
                    }));
                }
                _ => break,
            }
        }
        if sub.is_none() && sup.is_none() {
            return base;
        }
        match base {
            Node::BigOp { op, sub: s0, sup: p0 } => Node::BigOp { op, sub: sub.or(s0), sup: sup.or(p0) },
            b => Node::Scripts { base: Box::new(b), sub, sup },
        }
    }
    fn fenced(&mut self, open: char, close: char) -> Node {
        let body = self.group(Some(close));
        if self.peek() == Some(close) {
            self.i += 1;
            Node::Fenced { open, close, body: Box::new(body) }
        } else {
            // no closing bracket: keep the opening one as a plain symbol
            Node::Row(vec![Node::Sym { c: open, italic: false, class: Class::Ord }, body])
        }
    }
    fn delim(&mut self) -> char {
        self.skip_ws();
        match self.peek() {
            Some('\\') => {
                self.i += 1;
                match self.command().as_str() {
                    "{" | "lbrace" => '{',
                    "}" | "rbrace" => '}',
                    "|" => '‖',
                    "langle" => '⟨',
                    "rangle" => '⟩',
                    _ => '.',
                }
            }
            Some(c) => {
                self.i += 1;
                c
            }
            None => '.',
        }
    }
    fn atom(&mut self) -> Option<Node> {
        self.skip_ws();
        let c = self.peek()?;
        self.i += 1;
        Some(match c {
            '{' => {
                let r = self.group(Some('}'));
                if self.peek() == Some('}') {
                    self.i += 1;
                }
                r
            }
            '(' => self.fenced('(', ')'),
            '[' => self.fenced('[', ']'),
            '\\' => {
                let name = self.command();
                self.command_node(&name)
            }
            '^' | '_' => Node::Error(c.to_string()),
            '0'..='9' | '.' => {
                // a number; a comma between digits is the German decimal comma
                let mut s = String::from(c);
                while let Some(d) = self.peek() {
                    let next_digit = self.s.get(self.i + 1).map(|x| x.is_ascii_digit()).unwrap_or(false);
                    if d.is_ascii_digit() || ((d == ',' || d == '.') && next_digit) {
                        s.push(d);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                let v: Vec<Node> = s.chars().map(|c| Node::Sym { c, italic: false, class: Class::Ord }).collect();
                if v.len() == 1 {
                    v.into_iter().next().unwrap()
                } else {
                    Node::Row(v)
                }
            }
            c if c.is_alphabetic() => {
                // function names written without backslash (sin x, ln x, lim)
                let rest: String = std::iter::once(c).chain(self.s[self.i..].iter().copied().take_while(|x| x.is_alphabetic())).collect();
                if let Some(f) = FUNCTIONS.iter().find(|f| rest.starts_with(*f) && rest.len() == f.len()) {
                    self.i += f.chars().count() - 1;
                    if *f == "lim" {
                        Node::BigOp { op: 'l', sub: None, sup: None }
                    } else {
                        Node::Word(f.to_string())
                    }
                } else {
                    Node::Sym { c, italic: c.is_ascii_alphabetic() || ('α'..='ω').contains(&c), class: Class::Ord }
                }
            }
            '+' | '±' | '∓' | '×' | '÷' | '∪' | '∩' => Node::Sym { c, italic: false, class: Class::Bin },
            '-' | '−' => Node::Sym { c: '−', italic: false, class: Class::Bin },
            '*' | '·' => Node::Sym { c: '·', italic: false, class: Class::Bin },
            '=' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | '→' | '⇒' | '⇔' | '∈' => Node::Sym { c, italic: false, class: Class::Rel },
            ',' | ';' => Node::Sym { c, italic: false, class: Class::Punct },
            ':' => Node::Sym { c, italic: false, class: Class::Rel },
            '~' => Node::Space(0.28),
            ')' | ']' | '}' => Node::Error(c.to_string()),
            c => Node::Sym { c, italic: false, class: Class::Ord },
        })
    }
    fn command_node(&mut self, name: &str) -> Node {
        match name {
            "frac" | "dfrac" | "tfrac" => {
                let a = self.arg();
                let b = self.arg();
                Node::Frac(Box::new(a), Box::new(b))
            }
            "sqrt" => {
                self.skip_ws();
                let index = if self.peek() == Some('[') {
                    self.i += 1;
                    let r = self.group(Some(']'));
                    if self.peek() == Some(']') {
                        self.i += 1;
                    }
                    Some(Box::new(r))
                } else {
                    None
                };
                Node::Sqrt { body: Box::new(self.arg()), index }
            }
            "int" => Node::BigOp { op: '∫', sub: None, sup: None },
            "iint" => Node::BigOp { op: '∬', sub: None, sup: None },
            "oint" => Node::BigOp { op: '∮', sub: None, sup: None },
            "sum" => Node::BigOp { op: '∑', sub: None, sup: None },
            "prod" => Node::BigOp { op: '∏', sub: None, sup: None },
            "lim" => Node::BigOp { op: 'l', sub: None, sup: None },
            "left" => {
                let open = self.delim();
                let body = self.group(None);
                let close = if self.at_right() {
                    self.i += 6;
                    self.delim()
                } else {
                    '.'
                };
                Node::Fenced { open, close, body: Box::new(body) }
            }
            "right" => Node::Error("\\right".into()),
            "begin" => {
                let env = self.raw_group();
                let kind = match env.trim() {
                    "matrix" | "smallmatrix" => Some(TableKind::Matrix('.', '.')),
                    "pmatrix" => Some(TableKind::Matrix('(', ')')),
                    "bmatrix" => Some(TableKind::Matrix('[', ']')),
                    "Bmatrix" => Some(TableKind::Matrix('{', '}')),
                    "vmatrix" => Some(TableKind::Matrix('|', '|')),
                    "Vmatrix" => Some(TableKind::Matrix('‖', '‖')),
                    "cases" => Some(TableKind::Cases),
                    "aligned" | "align" | "align*" | "alignat" | "gathered" | "gather" | "gather*" | "split" | "eqnarray" | "eqnarray*" => {
                        Some(TableKind::Aligned)
                    }
                    "array" => {
                        // column format such as {cc|c}: not needed for the layout
                        self.raw_group();
                        Some(TableKind::Matrix('.', '.'))
                    }
                    _ => None,
                };
                let rows = self.table_body();
                if self.at_word("end") {
                    self.i += 4;
                    self.raw_group();
                }
                let table = Node::Table { kind: kind.unwrap_or(TableKind::Matrix('.', '.')), rows };
                match kind {
                    Some(_) => table,
                    None => Node::Row(vec![Node::Error(format!("\\begin{{{}}}", env.trim())), table]),
                }
            }
            "end" => {
                let env = self.raw_group();
                Node::Error(format!("\\end{{{env}}}"))
            }
            "vec" => Node::Accent { kind: Accent::Vec, body: Box::new(self.arg()) },
            "overline" | "bar" => Node::Accent { kind: Accent::Bar, body: Box::new(self.arg()) },
            "hat" => Node::Accent { kind: Accent::Hat, body: Box::new(self.arg()) },
            "dot" => Node::Accent { kind: Accent::Dot, body: Box::new(self.arg()) },
            "text" | "mathrm" | "textrm" | "operatorname" => Node::Word(self.raw_group()),
            "mathbb" => {
                let g = self.raw_group();
                let c = g.chars().next().unwrap_or('R');
                let m = BLACKBOARD.iter().find(|(a, _)| *a == c).map(|(_, b)| *b).unwrap_or(c);
                Node::Sym { c: m, italic: false, class: Class::Ord }
            }
            "," => Node::Space(0.17),
            ":" | ">" => Node::Space(0.22),
            ";" => Node::Space(0.28),
            " " => Node::Space(0.3),
            "!" => Node::Space(-0.17),
            "quad" => Node::Space(1.0),
            "qquad" => Node::Space(2.0),
            "\\" => Node::Space(1.0),
            n if FUNCTIONS.contains(&n) => Node::Word(n.to_string()),
            n => match SYMBOLS.iter().find(|(s, _, _)| *s == n) {
                Some((_, c, class)) => {
                    let italic = ('α'..='ω').contains(c);
                    Node::Sym { c: *c, italic, class: *class }
                }
                None => Node::Error(format!("\\{n}")),
            },
        }
    }
}

/// Parses formula source. Never fails: unknown parts become `Node::Error`.
/// Several lines ("\\") and alignment marks ("&") at the top level make an
/// aligned block, e.g. `x &= 2 + 3 \\ &= 5`.
pub fn parse(src: &str) -> Node {
    let mut p = Parser { s: src.chars().collect(), i: 0, env: 1 };
    let mut rows: Vec<Vec<Node>> = Vec::new();
    let mut cells: Vec<Node> = Vec::new();
    loop {
        let mut v = Vec::new();
        while p.i < p.s.len() {
            let n = p.row(None);
            v.push(n);
            p.skip_ws();
            if p.i >= p.s.len() || p.peek() == Some('&') || p.at_newline() {
                break;
            }
            // stray "\right", "\end" or "}"
            let c = p.s[p.i];
            p.i += 1;
            if c == '\\' {
                let name = p.command();
                v.push(Node::Error(format!("\\{name}")));
            } else {
                v.push(Node::Error(c.to_string()));
            }
        }
        v.retain(|n| !matches!(n, Node::Row(x) if x.is_empty()));
        cells.push(if v.len() == 1 { v.pop().unwrap() } else { Node::Row(v) });
        if p.peek() == Some('&') {
            p.i += 1;
        } else if p.at_newline() {
            p.i += 2;
            rows.push(std::mem::take(&mut cells));
        } else {
            break;
        }
    }
    rows.push(cells);
    let empty = |r: &Vec<Node>| r.iter().all(|c| matches!(c, Node::Row(v) if v.is_empty()));
    while rows.len() > 1 && rows.last().map(empty).unwrap_or(false) {
        rows.pop();
    }
    if rows.len() == 1 && rows[0].len() == 1 {
        rows.pop().unwrap().pop().unwrap()
    } else {
        Node::Table { kind: TableKind::Aligned, rows }
    }
}

/// Parts of the source the parser did not understand (for the editor).
pub fn errors(n: &Node) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(n: &Node, out: &mut Vec<String>) {
        match n {
            Node::Error(e) => out.push(e.clone()),
            Node::Row(v) => v.iter().for_each(|x| walk(x, out)),
            Node::Frac(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Node::Sqrt { body, index } => {
                walk(body, out);
                if let Some(i) = index {
                    walk(i, out);
                }
            }
            Node::Scripts { base, sub, sup } => {
                walk(base, out);
                for x in [sub, sup].into_iter().flatten() {
                    walk(x, out);
                }
            }
            Node::BigOp { sub, sup, .. } => {
                for x in [sub, sup].into_iter().flatten() {
                    walk(x, out);
                }
            }
            Node::Fenced { body, .. } | Node::Accent { body, .. } => walk(body, out),
            Node::Table { rows, .. } => rows.iter().flatten().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(n, &mut out);
    out
}

// ------------------------------------------------------------------ layout

/// Approximate advance width (em) of a serif font such as Times New Roman.
/// Every character is placed at its own x position, so the result does not
/// depend on the font the viewer actually has.
fn adv(c: char) -> f32 {
    match c {
        'i' | 'j' | 'l' => 0.28,
        'f' | 't' | 'r' => 0.36,
        's' => 0.39,
        'm' => 0.78,
        'w' => 0.72,
        'a'..='z' => 0.5,
        'I' => 0.36,
        'J' => 0.42,
        'M' => 0.89,
        'W' => 0.94,
        'A'..='Z' => 0.68,
        '0'..='9' => 0.5,
        '+' | '=' | '<' | '>' | '−' | '±' | '∓' | '×' | '÷' | '≤' | '≥' | '≠' | '≈' | '≡' | '∼' => 0.56,
        '·' | ',' | '.' | ';' | ':' | '′' => 0.25,
        '(' | ')' | '[' | ']' => 0.33,
        '{' | '}' => 0.48,
        '|' => 0.2,
        '‖' => 0.36,
        '∞' => 0.72,
        '→' | '←' | '↦' => 0.9,
        '⇒' | '⇐' | '⇔' => 1.0,
        '∈' | '∉' | '⊂' | '⊆' | '∪' | '∩' | '∂' | '∇' | '∅' | '∀' | '∃' | '∠' | '△' => 0.62,
        '…' | '⋯' => 0.9,
        'ℝ' | 'ℕ' | 'ℤ' | 'ℚ' | 'ℂ' => 0.72,
        'α'..='ω' => 0.54,
        'Α'..='Ω' => 0.66,
        '°' => 0.4,
        '%' => 0.83,
        '€' => 0.6,
        _ => 0.6,
    }
}

#[derive(Clone, Default)]
struct Bx {
    w: f32,
    asc: f32,
    desc: f32,
    prims: Vec<Prim>,
}

const ASC: f32 = 0.70;
const DESC: f32 = 0.22;
const AXIS: f32 = 0.25;

impl Bx {
    fn place(&mut self, other: &Bx, dx: f32, dy: f32) {
        for p in &other.prims {
            self.prims.push(shift(p, dx, dy));
        }
    }
}

fn shift(p: &Prim, dx: f32, dy: f32) -> Prim {
    let mv = |v: &Vec<Pos2>| v.iter().map(|q| Pos2::new(q.x + dx, q.y + dy)).collect::<Vec<_>>();
    match p {
        Prim::Lines { pts, width, color } => Prim::Lines { pts: mv(pts), width: *width, color: *color },
        Prim::Polygon { pts, fill, stroke } => Prim::Polygon { pts: mv(pts), fill: *fill, stroke: *stroke },
        Prim::Text { xs, y, chars, size, italic, bold, serif, color, valign, halign, has_dx } => Prim::Text {
            xs: xs.iter().map(|x| x + dx).collect(),
            y: y + dy,
            chars: chars.clone(),
            size: *size,
            italic: *italic,
            bold: *bold,
            serif: *serif,
            color: *color,
            valign: *valign,
            halign: *halign,
            has_dx: *has_dx,
        },
    }
}

struct Lay {
    color: Color32,
}

impl Lay {
    /// Characters on the baseline. Each one is centered in its own cell, so the
    /// spacing stays even when the viewer's font is a little wider or narrower.
    fn glyphs(&self, s: &str, italic: bool, size: f32, color: Color32) -> Bx {
        let mut prims = Vec::new();
        let mut x = 0.0;
        for c in s.chars() {
            let w = adv(c) * size;
            if c != ' ' {
                prims.push(Prim::Text {
                    xs: vec![(x + w / 2.0) * U],
                    y: 0.0,
                    chars: vec![c],
                    size: size * U,
                    italic,
                    bold: false,
                    serif: true,
                    color,
                    valign: 1,
                    halign: 1,
                    has_dx: false,
                });
            }
            x += w;
        }
        if italic {
            x += 0.04 * size;
        }
        Bx { w: x * U, asc: ASC * size * U, desc: DESC * size * U, prims }
    }

    fn line(&self, pts: &[(f32, f32)], w: f32) -> Prim {
        Prim::Lines { pts: pts.iter().map(|&(x, y)| Pos2::new(x, y)).collect(), width: w, color: self.color }
    }

    fn hcat(&self, items: Vec<Bx>) -> Bx {
        let mut out = Bx::default();
        for b in items {
            out.place(&b, out.w, 0.0);
            out.w += b.w;
            out.asc = out.asc.max(b.asc);
            out.desc = out.desc.max(b.desc);
        }
        out
    }

    fn node(&self, n: &Node, size: f32) -> Bx {
        let u = size * U;
        match n {
            Node::Row(v) => {
                let mut items = Vec::new();
                let mut prev: Option<Class> = None;
                for (k, x) in v.iter().enumerate() {
                    let class = match x {
                        Node::Sym { class, .. } => *class,
                        _ => Class::Ord,
                    };
                    // a minus/plus at the start or after a relation is a sign, not an operator
                    let unary = class == Class::Bin && (k == 0 || matches!(prev, Some(Class::Rel) | Some(Class::Bin) | Some(Class::Punct)));
                    let pad = match class {
                        Class::Bin if !unary => 0.22,
                        Class::Rel => 0.28,
                        _ => 0.0,
                    } * u;
                    if pad > 0.0 && k > 0 {
                        items.push(Bx { w: pad, ..Default::default() });
                    }
                    items.push(self.node(x, size));
                    if pad > 0.0 && k + 1 < v.len() {
                        items.push(Bx { w: pad, ..Default::default() });
                    }
                    if class == Class::Punct && k + 1 < v.len() {
                        items.push(Bx { w: 0.17 * u, ..Default::default() });
                    }
                    // a little room after a function name: "sin x"
                    if matches!(x, Node::Word(_)) && k + 1 < v.len() && !matches!(v[k + 1], Node::Fenced { .. }) {
                        items.push(Bx { w: 0.17 * u, ..Default::default() });
                    }
                    prev = Some(class);
                }
                let mut b = self.hcat(items);
                if v.is_empty() {
                    b.asc = ASC * u;
                    b.desc = DESC * u;
                }
                b
            }
            Node::Sym { c, .. } if BLACKBOARD.iter().any(|(_, b)| b == c) => {
                // ℝ ℕ ℤ ℚ ℂ: the letter with a second stroke, so no special font is needed
                let letter = BLACKBOARD.iter().find(|(_, b)| b == c).map(|(a, _)| *a).unwrap();
                let mut b = self.glyphs(&letter.to_string(), false, size, self.color);
                let w = adv(letter) * u;
                let lw = 0.045 * u;
                match letter {
                    // second diagonal next to the one of the Z
                    'Z' => b.prims.push(self.line(&[(w * 0.78 - 0.13 * u, -0.64 * u), (w * 0.14 - 0.02 * u, -0.03 * u)], lw)),
                    'C' | 'Q' => b.prims.push(self.line(&[(0.21 * u, -0.56 * u), (0.21 * u, -0.1 * u)], lw)),
                    // inner stem right of the letter's main stem
                    _ => b.prims.push(self.line(&[(0.25 * u, -0.64 * u), (0.25 * u, -0.01 * u)], lw)),
                }
                b
            }
            Node::Sym { c, italic, .. } => self.glyphs(&c.to_string(), *italic, size, self.color),
            Node::Word(s) => self.glyphs(s, false, size, self.color),
            Node::Error(s) => self.glyphs(s, false, size, Color32::from_rgb(210, 0, 0)),
            Node::Space(w) => Bx { w: w * u, asc: 0.0, desc: 0.0, prims: vec![] },
            Node::Frac(a, b) => {
                let s2 = if size > 0.85 { size } else { (size * 0.85).max(0.55) };
                let na = self.node(a, s2);
                let nb = self.node(b, s2);
                let pad = 0.12 * u;
                let w = na.w.max(nb.w) + 2.0 * pad;
                let gap = 0.14 * u;
                let axis = AXIS * u;
                let mut out = Bx { w, ..Default::default() };
                let ya = -axis - gap - na.desc;
                let yb = -axis + gap + nb.asc;
                out.place(&na, (w - na.w) / 2.0, ya);
                out.place(&nb, (w - nb.w) / 2.0, yb);
                out.prims.push(self.line(&[(0.04 * u, -axis), (w - 0.04 * u, -axis)], 0.05 * u));
                out.asc = -ya + na.asc;
                out.desc = yb + nb.desc;
                out
            }
            Node::Sqrt { body, index } => {
                let b = self.node(body, size);
                let gap = 0.12 * u;
                let top = -(b.asc + gap);
                let bottom = b.desc.max(0.1 * u);
                let hook = 0.6 * u;
                let mut out = Bx::default();
                let (mut ix, mut ib) = (0.0, None);
                if let Some(i) = index {
                    let bi = self.node(i, (size * 0.55).max(0.4));
                    ix = (bi.w - 0.3 * u).max(0.0);
                    ib = Some(bi);
                }
                let x0 = ix;
                let mid = (top + bottom) * 0.45;
                out.prims.push(self.line(
                    &[(x0, mid + 0.05 * u), (x0 + 0.12 * u, mid - 0.03 * u), (x0 + 0.3 * u, bottom), (x0 + 0.52 * u, top), (x0 + hook + b.w + 0.08 * u, top)],
                    0.05 * u,
                ));
                if let Some(bi) = ib {
                    out.place(&bi, 0.0, mid - 0.12 * u - bi.desc);
                    out.asc = out.asc.max(-(mid - 0.12 * u - bi.desc) + bi.asc);
                }
                out.place(&b, x0 + hook, 0.0);
                out.w = x0 + hook + b.w + 0.12 * u;
                out.asc = out.asc.max(-top + 0.05 * u);
                out.desc = bottom;
                out
            }
            Node::Scripts { base, sub, sup } => {
                let b = self.node(base, size);
                let ss = (size * 0.7).max(0.45);
                let mut out = Bx::default();
                out.place(&b, 0.0, 0.0);
                let italic_base = matches!(**base, Node::Sym { italic: true, .. });
                let mut w = b.w;
                let mut asc = b.asc;
                let mut desc = b.desc;
                let mut sw: f32 = 0.0;
                if let Some(p) = sup {
                    let bp = self.node(p, ss);
                    let y = -(0.42 * u).max(b.asc - 0.3 * u);
                    out.place(&bp, b.w + if italic_base { 0.03 * u } else { 0.0 }, y);
                    asc = asc.max(-y + bp.asc);
                    sw = sw.max(bp.w);
                }
                if let Some(s) = sub {
                    let bs = self.node(s, ss);
                    let y = (0.2 * u).max(b.desc - 0.05 * u + if sup.is_some() { 0.08 * u } else { 0.0 });
                    out.place(&bs, b.w - if italic_base { 0.03 * u } else { 0.0 }, y);
                    desc = desc.max(y + bs.desc);
                    sw = sw.max(bs.w);
                }
                w += sw + 0.02 * u;
                out.w = w;
                out.asc = asc;
                out.desc = desc;
                out
            }
            Node::BigOp { op, sub, sup } => {
                let ss = (size * 0.7).max(0.45);
                let bsub = sub.as_ref().map(|s| self.node(s, ss));
                let bsup = sup.as_ref().map(|s| self.node(s, ss));
                match op {
                    '∫' | '∬' | '∮' => {
                        // integral sign drawn as a shape; limits at the right
                        let h = 2.05 * size; // em
                        let n = match op {
                            '∬' => 2,
                            _ => 1,
                        };
                        let mut out = Bx::default();
                        let base = 0.5 * u; // baseline of the sign
                        let gw = 0.273 * h * U;
                        for k in 0..n {
                            for p in crate::formula::vector_glyph(0xF2, k as f32 * gw * 0.7, base, h * U, self.color) {
                                out.prims.push(p);
                            }
                        }
                        if *op == '∮' {
                            let cx = gw * 0.5;
                            let r = 0.16 * u;
                            let pts: Vec<(f32, f32)> =
                                (0..=16).map(|i| i as f32 / 16.0 * std::f32::consts::TAU).map(|a| (cx + r * a.cos(), -AXIS * u + r * a.sin())).collect();
                            out.prims.push(self.line(&pts, 0.04 * u));
                        }
                        let sign_w = gw * (1.0 + 0.7 * (n as f32 - 1.0));
                        let top = base - 0.91 * h * U;
                        let bottom = base + 0.107 * h * U;
                        let mut w = sign_w;
                        let mut asc = -top;
                        let mut desc = bottom;
                        let mut lw: f32 = 0.0;
                        if let Some(p) = &bsup {
                            let y = top + p.asc;
                            out.place(p, sign_w + 0.02 * u, y);
                            asc = asc.max(-(y - p.asc));
                            lw = lw.max(p.w);
                        }
                        if let Some(s) = &bsub {
                            let y = bottom - s.desc;
                            out.place(s, sign_w - 0.18 * u, y);
                            desc = desc.max(y + s.desc);
                            lw = lw.max(s.w - 0.2 * u);
                        }
                        w += lw + 0.12 * u;
                        out.w = w;
                        out.asc = asc;
                        out.desc = desc;
                        out
                    }
                    _ => {
                        // ∑ ∏ lim: limits centered below and above
                        let core =
                            if *op == 'l' { self.glyphs("lim", false, size, self.color) } else { self.glyphs(&op.to_string(), false, size * 1.45, self.color) };
                        // center a big symbol on the math axis
                        let dy = if *op == 'l' { 0.0 } else { 0.2 * u };
                        let w = core.w.max(bsub.as_ref().map(|b| b.w).unwrap_or(0.0)).max(bsup.as_ref().map(|b| b.w).unwrap_or(0.0));
                        let mut out = Bx { w: w + 0.1 * u, ..Default::default() };
                        out.place(&core, (w - core.w) / 2.0, dy);
                        let mut asc = core.asc - dy;
                        let mut desc = core.desc + dy;
                        if let Some(p) = &bsup {
                            let y = -asc - 0.08 * u - p.desc;
                            out.place(p, (w - p.w) / 2.0, y);
                            asc = -y + p.asc;
                        }
                        if let Some(s) = &bsub {
                            let y = desc + 0.06 * u + s.asc;
                            out.place(s, (w - s.w) / 2.0, y);
                            desc = y + s.desc;
                        }
                        out.asc = asc;
                        out.desc = desc;
                        if *op == 'l' {
                            out.w += 0.12 * u;
                        }
                        out
                    }
                }
            }
            Node::Fenced { open, close, body } => {
                let b = self.node(body, size);
                let a = b.asc.max(ASC * u);
                let d = b.desc.max(DESC * u);
                let tall = a + d > 1.25 * u;
                let mut items = Vec::new();
                for (k, ch) in [*open, *close].iter().enumerate() {
                    let dl = if *ch == '.' {
                        Bx { w: 0.0, ..Default::default() }
                    } else if tall {
                        self.delimiter(*ch, a + 0.05 * u, d + 0.05 * u, size, k == 1)
                    } else {
                        self.glyphs(&ch.to_string(), false, size, self.color)
                    };
                    if k == 0 {
                        items.push(dl);
                        items.push(b.clone());
                    } else {
                        items.push(dl);
                    }
                }
                self.hcat(items)
            }
            Node::Accent { kind, body } => {
                let b = self.node(body, size);
                let mut out = b.clone();
                let y = -(b.asc + 0.08 * u);
                let (x0, x1) = (0.05 * u, b.w.max(0.4 * u) - 0.02 * u);
                match kind {
                    Accent::Bar => out.prims.push(self.line(&[(x0, y), (x1, y)], 0.045 * u)),
                    Accent::Vec => {
                        out.prims.push(self.line(&[(x0, y), (x1, y)], 0.045 * u));
                        out.prims.push(self.line(&[(x1 - 0.14 * u, y - 0.08 * u), (x1, y), (x1 - 0.14 * u, y + 0.08 * u)], 0.045 * u));
                    }
                    Accent::Hat => {
                        let cx = b.w / 2.0;
                        out.prims.push(self.line(&[(cx - 0.16 * u, y + 0.04 * u), (cx, y - 0.1 * u), (cx + 0.16 * u, y + 0.04 * u)], 0.045 * u));
                    }
                    Accent::Dot => {
                        let cx = b.w / 2.0;
                        let r = 0.05 * u;
                        let pts: Vec<Pos2> =
                            (0..10).map(|i| i as f32 / 10.0 * std::f32::consts::TAU).map(|t| Pos2::new(cx + r * t.cos(), y + r * t.sin())).collect();
                        out.prims.push(Prim::Polygon { pts, fill: Some(self.color), stroke: None });
                    }
                }
                out.asc = -y + 0.14 * u;
                out.w = out.w.max(x1 + 0.04 * u);
                out
            }
            Node::Table { kind, rows } => {
                let t = self.table(*kind, rows, size);
                let (open, close) = match kind {
                    TableKind::Matrix(o, c) => (*o, *c),
                    TableKind::Cases => ('{', '.'),
                    TableKind::Aligned => ('.', '.'),
                };
                if open == '.' && close == '.' {
                    return t;
                }
                let (a, d) = (t.asc + 0.08 * u, t.desc + 0.08 * u);
                let mut items = Vec::new();
                if open != '.' {
                    items.push(self.delimiter(open, a, d, size, false));
                    items.push(Bx { w: 0.08 * u, ..Default::default() });
                }
                items.push(t);
                if close != '.' {
                    items.push(Bx { w: 0.08 * u, ..Default::default() });
                    items.push(self.delimiter(close, a, d, size, true));
                }
                self.hcat(items)
            }
        }
    }

    /// Cells in rows and columns, centered on the math axis. Matrices center their
    /// cells; case distinctions are left-aligned; aligned lines put the "&" marks
    /// under each other (columns alternate right / left aligned).
    fn table(&self, kind: TableKind, rows: &[Vec<Node>], size: f32) -> Bx {
        let u = size * U;
        let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let boxes: Vec<Vec<Bx>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(j, c)| {
                        let mut b = self.node(c, size);
                        // "&= …": the relation after the mark keeps its space in front
                        if kind == TableKind::Aligned && j % 2 == 1 && starts_with_rel(c) {
                            let mut p = Bx { w: 0.28 * u, asc: b.asc, desc: b.desc, prims: vec![] };
                            p.place(&b, 0.28 * u, 0.0);
                            p.w += b.w;
                            b = p;
                        }
                        b
                    })
                    .collect()
            })
            .collect();
        let mut colw = vec![0.0f32; ncols];
        for r in &boxes {
            for (j, b) in r.iter().enumerate() {
                colw[j] = colw[j].max(b.w);
            }
        }
        let (row_gap, col_gap) = match kind {
            TableKind::Matrix(..) => (0.3 * u, 0.8 * u),
            TableKind::Cases => (0.25 * u, 1.0 * u),
            TableKind::Aligned => (0.3 * u, 1.5 * u),
        };
        // space in front of each column
        let gap_before = |j: usize| -> f32 {
            // no gap at the start, nor between the two halves of an aligned pair ("x" | "= 2")
            if j == 0 || (kind == TableKind::Aligned && j % 2 == 1) {
                0.0
            } else {
                col_gap
            }
        };
        let mut xs = Vec::with_capacity(ncols);
        let mut x = 0.0;
        for (j, w) in colw.iter().enumerate() {
            x += gap_before(j);
            xs.push(x);
            x += w;
        }
        let width = x;
        let metrics: Vec<(f32, f32)> =
            boxes.iter().map(|r| (r.iter().map(|b| b.asc).fold(ASC * u, f32::max), r.iter().map(|b| b.desc).fold(DESC * u, f32::max))).collect();
        let height: f32 = metrics.iter().map(|(a, d)| a + d).sum::<f32>() + row_gap * (metrics.len().saturating_sub(1)) as f32;
        let top = -AXIS * u - height / 2.0;
        let mut out = Bx { w: width, asc: -top, desc: height + top, prims: vec![] };
        let mut y = top;
        for (r, (a, d)) in boxes.iter().zip(&metrics) {
            let base = y + a;
            for (j, b) in r.iter().enumerate() {
                let dx = match kind {
                    TableKind::Matrix(..) => (colw[j] - b.w) / 2.0,
                    TableKind::Cases => 0.0,
                    TableKind::Aligned => {
                        if j.is_multiple_of(2) {
                            colw[j] - b.w
                        } else {
                            0.0
                        }
                    }
                };
                out.place(b, xs[j] + dx, base);
            }
            y = base + d + row_gap;
        }
        out
    }

    /// A stretched bracket drawn as lines, from `a` above to `d` below the baseline.
    fn delimiter(&self, ch: char, a: f32, d: f32, size: f32, right: bool) -> Bx {
        let u = size * U;
        let lw = 0.055 * u;
        let (top, bot) = (-a, d);
        let h = a + d;
        let w = match ch {
            '|' | '‖' => 0.25 * u,
            '{' | '}' => 0.5 * u,
            _ => (0.32 + 0.04 * (h / u)).min(0.5) * u,
        };
        let mirror = |pts: Vec<(f32, f32)>| -> Vec<(f32, f32)> {
            if right {
                pts.into_iter().map(|(x, y)| (w - x, y)).collect()
            } else {
                pts
            }
        };
        let q = |p0: (f32, f32), c: (f32, f32), p2: (f32, f32)| -> Vec<(f32, f32)> {
            (0..=10)
                .map(|i| {
                    let t = i as f32 / 10.0;
                    let s = 1.0 - t;
                    (s * s * p0.0 + 2.0 * s * t * c.0 + t * t * p2.0, s * s * p0.1 + 2.0 * s * t * c.1 + t * t * p2.1)
                })
                .collect()
        };
        let mut prims = Vec::new();
        match ch {
            '(' | ')' => {
                let (xo, xi) = (w * 0.85, w * 0.2);
                let pts = q((xo, top), (xi - 0.05 * u, (top + bot) / 2.0), (xo, bot));
                prims.push(self.line(&mirror(pts), lw));
            }
            '[' | ']' => {
                let (xo, xi) = (w * 0.8, w * 0.25);
                prims.push(self.line(&mirror(vec![(xo, top), (xi, top), (xi, bot), (xo, bot)]), lw));
            }
            '{' | '}' => {
                let m = (top + bot) / 2.0;
                let (xo, xc, xi) = (w * 0.85, w * 0.45, w * 0.1);
                let mut pts = q((xo, top), (xc, top), (xc, top + 0.18 * h));
                pts.extend(q((xc, m - 0.18 * h), (xc, m), (xi, m)));
                let mut lower = q((xi, m), (xc, m), (xc, m + 0.18 * h));
                lower.extend(q((xc, bot - 0.18 * h), (xc, bot), (xo, bot)));
                prims.push(self.line(&mirror(pts), lw));
                prims.push(self.line(&mirror(lower), lw));
            }
            '|' => prims.push(self.line(&[(w / 2.0, top), (w / 2.0, bot)], lw)),
            '‖' => {
                prims.push(self.line(&[(w * 0.3, top), (w * 0.3, bot)], lw));
                prims.push(self.line(&[(w * 0.7, top), (w * 0.7, bot)], lw));
            }
            '⟨' | '⟩' => prims.push(self.line(&mirror(vec![(w * 0.8, top), (w * 0.2, (top + bot) / 2.0), (w * 0.8, bot)]), lw)),
            _ => return self.glyphs(&ch.to_string(), false, size, self.color),
        }
        Bx { w, asc: a, desc: d, prims }
    }
}

fn starts_with_rel(n: &Node) -> bool {
    match n {
        Node::Sym { class: Class::Rel, .. } => true,
        Node::Row(v) => v.first().map(starts_with_rel).unwrap_or(false),
        _ => false,
    }
}

/// Lays out formula source as a drawing (units: `U` per em, baseline at y = 0).
pub fn layout(src: &str) -> Drawing {
    layout_node(&parse(src))
}

pub fn layout_node(n: &Node) -> Drawing {
    let l = Lay { color: Color32::BLACK };
    let b = l.node(n, 1.0);
    let pad = 0.06 * U;
    let asc = b.asc.max(ASC * U);
    let desc = b.desc.max(DESC * U);
    Drawing { org: Pos2::new(-pad, -asc - pad), ext: Vec2::new(b.w.max(0.2 * U) + 2.0 * pad, asc + desc + 2.0 * pad), prims: b.prims }
}

/// How far the formula reaches below its baseline, in pixels (for aligning it with text).
pub fn depth_px(d: &Drawing) -> f32 {
    (d.org.y + d.ext.y) / U * PX_PER_EM
}

/// Display size in pixels at the normal text size.
pub fn size_px(d: &Drawing) -> (f32, f32) {
    (d.ext.x / U * PX_PER_EM, d.ext.y / U * PX_PER_EM)
}

// ------------------------------------------------------------------ OMML (Word)

fn x(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn run(text: &str, plain: bool) -> String {
    if plain {
        format!("<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t xml:space=\"preserve\">{}</m:t></m:r>", x(text))
    } else {
        format!("<m:r><m:t xml:space=\"preserve\">{}</m:t></m:r>", x(text))
    }
}

fn omml_nary(op: char, sub: &Option<Box<Node>>, sup: &Option<Box<Node>>, body: &str) -> String {
    let loc = if matches!(op, '∫' | '∬' | '∮') { "subSup" } else { "undOvr" };
    let mut pr = format!("<m:naryPr><m:chr m:val=\"{op}\"/><m:limLoc m:val=\"{loc}\"/>");
    if sub.is_none() {
        pr += "<m:subHide m:val=\"1\"/>";
    }
    if sup.is_none() {
        pr += "<m:supHide m:val=\"1\"/>";
    }
    pr += "</m:naryPr>";
    format!(
        "<m:nary>{pr}<m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{body}</m:e></m:nary>",
        sub.as_ref().map(|s| omml_node(s)).unwrap_or_default(),
        sup.as_ref().map(|s| omml_node(s)).unwrap_or_default()
    )
}

fn omml_node(n: &Node) -> String {
    let e = |n: &Node| format!("<m:e>{}</m:e>", omml_node(n));
    match n {
        Node::Row(v) => {
            // ∫ ∑ ∏: what follows (up to the next relation such as "=") is the operand
            let mut out = String::new();
            let mut k = 0;
            while k < v.len() {
                if let Node::BigOp { op, sub, sup } = &v[k] {
                    if *op != 'l' {
                        let mut j = k + 1;
                        while j < v.len() && !matches!(v[j], Node::Sym { class: Class::Rel, .. }) {
                            j += 1;
                        }
                        let body: String = v[k + 1..j].iter().map(omml_node).collect();
                        out += &omml_nary(*op, sub, sup, &body);
                        k = j;
                        continue;
                    }
                }
                out += &omml_node(&v[k]);
                k += 1;
            }
            out
        }
        Node::Sym { c, italic, .. } => run(&c.to_string(), !italic),
        Node::Word(s) => run(s, true),
        Node::Error(s) => run(s, true),
        Node::Space(w) => {
            if *w >= 0.9 {
                run("\u{2003}", true)
            } else if *w > 0.0 {
                run("\u{2009}", true)
            } else {
                String::new()
            }
        }
        Node::Frac(a, b) => format!("<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>", omml_node(a), omml_node(b)),
        Node::Sqrt { body, index } => match index {
            Some(i) => format!("<m:rad><m:deg>{}</m:deg>{}</m:rad>", omml_node(i), e(body)),
            None => format!("<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/>{}</m:rad>", e(body)),
        },
        Node::Scripts { base, sub, sup } => match (sub, sup) {
            (Some(s), Some(p)) => format!("<m:sSubSup>{}<m:sub>{}</m:sub><m:sup>{}</m:sup></m:sSubSup>", e(base), omml_node(s), omml_node(p)),
            (Some(s), None) => format!("<m:sSub>{}<m:sub>{}</m:sub></m:sSub>", e(base), omml_node(s)),
            (None, Some(p)) => format!("<m:sSup>{}<m:sup>{}</m:sup></m:sSup>", e(base), omml_node(p)),
            (None, None) => omml_node(base),
        },
        Node::BigOp { op, sub, sup } => {
            if *op == 'l' {
                let lim = run("lim", true);
                return match sub {
                    Some(s) => format!("<m:limLow><m:e>{lim}</m:e><m:lim>{}</m:lim></m:limLow>", omml_node(s)),
                    None => lim,
                };
            }
            // an operator standing alone (its operand is handled in Node::Row)
            omml_nary(*op, sub, sup, "")
        }
        Node::Fenced { open, close, body } => {
            let o = if *open == '.' { String::new() } else { open.to_string() };
            let c = if *close == '.' { String::new() } else { close.to_string() };
            format!("<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/></m:dPr>{}</m:d>", x(&o), x(&c), e(body))
        }
        Node::Table { kind, rows } => {
            let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
            let jc = |j: usize| match kind {
                TableKind::Matrix(..) => "center",
                TableKind::Cases => "left",
                TableKind::Aligned => {
                    if j.is_multiple_of(2) {
                        "right"
                    } else {
                        "left"
                    }
                }
            };
            let mut m = String::from("<m:m><m:mPr><m:mcs>");
            for j in 0..ncols {
                m += &format!("<m:mc><m:mcPr><m:count m:val=\"1\"/><m:mcJc m:val=\"{}\"/></m:mcPr></m:mc>", jc(j));
            }
            m += "</m:mcs></m:mPr>";
            for r in rows {
                m += "<m:mr>";
                for j in 0..ncols {
                    match r.get(j) {
                        Some(c) => m += &e(c),
                        None => m += "<m:e/>",
                    }
                }
                m += "</m:mr>";
            }
            m += "</m:m>";
            let (open, close) = match kind {
                TableKind::Matrix(o, c) => (*o, *c),
                TableKind::Cases => ('{', '.'),
                TableKind::Aligned => ('.', '.'),
            };
            if open == '.' && close == '.' {
                m
            } else {
                let o = if open == '.' { String::new() } else { open.to_string() };
                let c = if close == '.' { String::new() } else { close.to_string() };
                format!("<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/></m:dPr><m:e>{m}</m:e></m:d>", x(&o), x(&c))
            }
        }
        Node::Accent { kind, body } => match kind {
            Accent::Bar => format!("<m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr>{}</m:bar>", e(body)),
            Accent::Vec => format!("<m:acc><m:accPr><m:chr m:val=\"\u{20D7}\"/></m:accPr>{}</m:acc>", e(body)),
            Accent::Hat => format!("<m:acc><m:accPr><m:chr m:val=\"\u{0302}\"/></m:accPr>{}</m:acc>", e(body)),
            Accent::Dot => format!("<m:acc><m:accPr><m:chr m:val=\"\u{0307}\"/></m:accPr>{}</m:acc>", e(body)),
        },
    }
}

/// The formula in Word's own format (Office Math), editable in Word.
pub fn to_omml(src: &str) -> String {
    format!("<m:oMath>{}</m:oMath>", omml_node(&parse(src)))
}

/// A formula as one line of text for previews, e.g. `\text{CH}_4` → "CH₄",
/// `\frac{a}{b}` → "a/b", `x^2` → "x²".
pub fn to_text(src: &str) -> String {
    fn small(s: &str, sup: bool) -> Option<String> {
        let (from, to) = if sup {
            ("0123456789+-=()n", "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿ")
        } else {
            ("0123456789+-=()", "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎")
        };
        s.chars().map(|c| from.chars().position(|f| f == c).and_then(|i| to.chars().nth(i))).collect()
    }
    fn group(n: &Node) -> String {
        let t = text(n);
        if t.chars().count() > 1 && !matches!(n, Node::Fenced { .. }) {
            format!("({t})")
        } else {
            t
        }
    }
    fn text(n: &Node) -> String {
        match n {
            Node::Row(v) => {
                let mut out = String::new();
                for x in v {
                    let t = text(x);
                    let spaced = matches!(x, Node::Sym { class: Class::Rel | Class::Bin, .. });
                    if spaced && !out.is_empty() {
                        out.push(' ');
                        out += &t;
                        out.push(' ');
                    } else {
                        out += &t;
                    }
                }
                out.split_whitespace().collect::<Vec<_>>().join(" ")
            }
            Node::Sym { c, .. } => c.to_string(),
            Node::Word(w) => w.clone(),
            Node::Error(e) => e.clone(),
            Node::Space(w) => {
                if *w >= 0.15 {
                    " ".into()
                } else {
                    String::new()
                }
            }
            Node::Frac(a, b) => format!("{}/{}", group(a), group(b)),
            Node::Sqrt { body, index } => match index {
                Some(i) => format!("{}√{}", text(i), group(body)),
                None => format!("√{}", group(body)),
            },
            Node::Scripts { base, sub, sup } => {
                let mut s = text(base);
                if let Some(x) = sub {
                    let t = text(x);
                    s += &small(&t, false).unwrap_or(format!("_{}", group(x)));
                }
                if let Some(x) = sup {
                    let t = text(x);
                    s += &small(&t, true).unwrap_or(format!("^{}", group(x)));
                }
                s
            }
            Node::BigOp { op, sub, sup } => {
                let mut s = if *op == 'l' { "lim".to_string() } else { op.to_string() };
                if let Some(x) = sub {
                    s += &format!("_{}", group(x));
                }
                if let Some(x) = sup {
                    s += &format!("^{}", group(x));
                }
                s + " "
            }
            Node::Fenced { open, close, body } => {
                let o = if *open == '.' { String::new() } else { open.to_string() };
                let c = if *close == '.' { String::new() } else { close.to_string() };
                format!("{o}{}{c}", text(body))
            }
            Node::Accent { body, .. } => text(body),
            Node::Table { rows, .. } => rows.iter().map(|r| r.iter().map(text).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("; "),
        }
    }
    text(&parse(src)).trim().to_string()
}

/// Plain-text form for places that cannot show formulas (old XML format).
pub fn to_plain(src: &str) -> String {
    src.replace("\\\\", "; ").replace('&', "").replace("\\frac", "").replace("\\cdot", "·").replace('\\', "")
}
