//! Reader for the binary property streams used inside the old task files
//! (style tables, tables and embedded objects).

#[derive(Debug, Clone)]
pub enum DVal {
    Null,
    /// list values are read to stay in step with the stream; their content is not used
    #[allow(dead_code)]
    List(Vec<DVal>),
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Bool(bool),
    Binary(Vec<u8>),
    Set(Vec<String>),
    Nil,
    Collection(Vec<Vec<(String, DVal)>>),
}

impl DVal {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            DVal::Int(i) => Some(*i),
            DVal::Float(f) => Some(*f as i64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DVal::Str(s) | DVal::Ident(s) => Some(s),
            _ => None,
        }
    }
}

pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

pub fn cp1252(bytes: &[u8]) -> String {
    const HI: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™',
        'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    bytes.iter().map(|&b| if (0x80..0xA0).contains(&b) { HI[(b - 0x80) as usize] } else { b as char }).collect()
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    pub fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }
    pub fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return None;
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }
    pub fn u32(&mut self) -> Option<u32> {
        let b = self.bytes(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn shortstr(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(cp1252(self.bytes(n)?))
    }

    pub fn value(&mut self) -> Option<DVal> {
        let t = self.u8()?;
        Some(match t {
            0 => DVal::Null,
            1 => {
                let mut v = Vec::new();
                while self.peek()? != 0 {
                    v.push(self.value()?);
                }
                self.pos += 1;
                DVal::List(v)
            }
            2 => DVal::Int(self.u8()? as i8 as i64),
            3 => {
                let b = self.bytes(2)?;
                DVal::Int(i16::from_le_bytes([b[0], b[1]]) as i64)
            }
            4 => DVal::Int(self.u32()? as i32 as i64),
            5 => {
                let b = self.bytes(10)?;
                DVal::Float(extended_to_f64(b))
            }
            6 => DVal::Str(self.shortstr()?),
            7 => DVal::Ident(self.shortstr()?),
            8 => DVal::Bool(false),
            9 => DVal::Bool(true),
            10 => {
                let n = self.u32()? as usize;
                DVal::Binary(self.bytes(n)?.to_vec())
            }
            11 => {
                let mut v = Vec::new();
                loop {
                    let s = self.shortstr()?;
                    if s.is_empty() {
                        break;
                    }
                    v.push(s);
                }
                DVal::Set(v)
            }
            12 | 20 => {
                let n = self.u32()? as usize;
                DVal::Str(String::from_utf8_lossy(self.bytes(n)?).into_owned())
            }
            13 => DVal::Nil,
            14 => {
                let mut items = Vec::new();
                loop {
                    let p = self.peek()?;
                    if p == 0 {
                        self.pos += 1;
                        break;
                    }
                    if matches!(p, 2..=4) {
                        self.value()?;
                    }
                    if self.u8()? != 1 {
                        return None;
                    }
                    items.push(self.props()?);
                }
                DVal::Collection(items)
            }
            15 => {
                let b = self.bytes(4)?;
                DVal::Float(f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64)
            }
            16 | 17 | 21 => {
                let b = self.bytes(8)?;
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                DVal::Float(f64::from_le_bytes(a))
            }
            18 => {
                let n = self.u32()? as usize;
                let b = self.bytes(n * 2)?;
                let u: Vec<u16> = b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                DVal::Str(String::from_utf16_lossy(&u))
            }
            19 => {
                let b = self.bytes(8)?;
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                DVal::Int(i64::from_le_bytes(a))
            }
            _ => return None,
        })
    }

    /// Properties until the end-of-list marker.
    pub fn props(&mut self) -> Option<Vec<(String, DVal)>> {
        let mut v = Vec::new();
        loop {
            if self.peek()? == 0 {
                self.pos += 1;
                return Some(v);
            }
            let name = self.shortstr()?;
            let val = self.value()?;
            v.push((name, val));
        }
    }

    /// Optional filer flags before a component header.
    pub fn skip_flags(&mut self) -> Option<()> {
        if let Some(p) = self.peek() {
            if p & 0xF0 == 0xF0 {
                self.pos += 1;
                if p & 2 != 0 {
                    self.value()?;
                }
            }
        }
        Some(())
    }
}

fn extended_to_f64(b: &[u8]) -> f64 {
    let mut m = 0u64;
    for (i, x) in b.iter().take(8).enumerate() {
        m |= (*x as u64) << (8 * i);
    }
    let se = u16::from_le_bytes([b[8], b[9]]);
    let sign = if se & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = (se & 0x7FFF) as i32;
    if e == 0 && m == 0 {
        return 0.0;
    }
    sign * (m as f64) * 2f64.powi(e - 16383 - 63)
}

pub fn find<'b>(props: &'b [(String, DVal)], name: &str) -> Option<&'b DVal> {
    props.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

/// Reads a simple saved component: "TPF0", class, name, properties.
pub fn read_component(data: &[u8]) -> Option<(String, Vec<(String, DVal)>)> {
    let start = find_bytes(data, b"TPF0")?;
    let mut r = Reader::new(&data[start + 4..]);
    r.skip_flags()?;
    let class = r.shortstr()?;
    let _name = r.shortstr()?;
    let props = r.props()?;
    Some((class, props))
}

pub fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}
