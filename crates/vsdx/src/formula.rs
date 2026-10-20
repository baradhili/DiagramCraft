//! Minimal ShapeSheet formula evaluator.
//!
//! Supports the subset needed for geometry/size evaluation (Analysis doc 06):
//! numeric literals, cell references (`Width`, `Geometry1.X1`), `+ - * / ^`,
//! parentheses, unary minus, string literals with `&` concatenation, and
//! `&<n>` number-to-string coercion. Function calls (`IF`, `THEMEVAL`, …)
//! are not evaluated; callers should fall back to the cached `V` value.

use std::collections::HashMap;

pub type Env = HashMap<String, f64>;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Amp,
    LParen,
    RParen,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => s.trim().parse().ok(),
        }
    }
    fn coerce_str(self) -> String {
        match self {
            Value::Num(n) => format_num(n),
            Value::Str(s) => s,
        }
    }
}

fn format_num(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Evaluate a formula. Returns `Err` for anything unsupported so callers
/// can fall back to the cached value.
pub fn eval(expr: &str, env: &Env) -> Result<Value, String> {
    let toks = tokenize(expr)?;
    let mut p = Parser { toks: &toks, pos: 0, env };
    let v = p.expr()?;
    if p.pos != toks.len() {
        return Err(format!("trailing tokens at {}", p.pos));
    }
    Ok(v)
}

/// Multiplier converting a formula literal's unit suffix to internal units
/// (inches for lengths, radians for angles). Unknown suffixes are treated as
/// unit-less: shipped `V` values are already normalized ([MS-VSDX]
/// lengthInternalUnitNumber / angleInternalUnitNumber).
fn unit_factor(suffix: &str) -> f64 {
    const IN_PER_CM: f64 = 0.3937007874015748;
    match suffix.to_ascii_uppercase().as_str() {
        "" | "DL" | "DP" | "IN" | "PZ" => 1.0, // drawn/local, page, inch
        "FT" => 12.0,
        "YD" => 36.0,
        "MI" => 63360.0,
        "MM" => IN_PER_CM / 10.0,
        "CM" => IN_PER_CM,
        "M" => IN_PER_CM * 100.0,
        "KM" => IN_PER_CM * 100000.0,
        "HI" => IN_PER_CM / 10000.0, // himetric = 0.0001 cm
        "PT" => 1.0 / 72.0,
        "PICA" | "PICA_PT" => 1.0 / 6.0,
        "DEG" => std::f64::consts::PI / 180.0,
        "RAD" => 1.0,
        "PER" => 0.01,
        _ => 1.0,
    }
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '+' => {
                toks.push(Tok::Plus);
                i += 1;
            }
            '-' => {
                toks.push(Tok::Minus);
                i += 1;
            }
            '*' => {
                toks.push(Tok::Star);
                i += 1;
            }
            '/' => {
                toks.push(Tok::Slash);
                i += 1;
            }
            '^' => {
                toks.push(Tok::Caret);
                i += 1;
            }
            '&' => {
                toks.push(Tok::Amp);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '"' => {
                let mut out = String::new();
                i += 1;
                while i < b.len() && b[i] as char != '"' {
                    if b[i] as char == '&' && i + 2 < b.len() {
                        // minimal entity pass
                        let ent = &s[i + 1..(i + 3).min(s.len())];
                        if ent.starts_with("amp") {
                            out.push('&');
                            i += 4;
                            continue;
                        }
                    }
                    out.push(b[i] as char);
                    i += 1;
                }
                i += 1; // closing quote
                toks.push(Tok::Str(out));
            }
            '0'..='9' | '.' => {
                let start = i;
                while i < b.len() && ((b[i] as char).is_ascii_digit() || b[i] as char == '.') {
                    i += 1;
                }
                let digits_end = i;
                // A directly-attached unit suffix (0.24PT, 90DEG) is
                // converted to internal units (inches / radians) per
                // [MS-VSDX] §2.5.7–2.5.8.
                while i < b.len() && (b[i] as char).is_ascii_alphabetic() {
                    i += 1;
                }
                let n: f64 = s[start..digits_end]
                    .parse()
                    .map_err(|_| format!("bad number '{}'", &s[start..digits_end]))?;
                let factor = unit_factor(&s[digits_end..i]);
                toks.push(Tok::Num(n * factor));
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < b.len() {
                    let ch = b[i] as char;
                    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let word = &s[start..i];
                if word == "TRUE" {
                    toks.push(Tok::Num(1.0));
                } else if word == "FALSE" {
                    toks.push(Tok::Num(0.0));
                } else if i < b.len() && (b[i] as char) == '(' {
                    return Err(format!("function call '{word}' not supported"));
                } else {
                    toks.push(Tok::Ident(word.to_string()));
                }
            }
            other => return Err(format!("unexpected char '{other}'")),
        }
    }
    Ok(toks)
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    env: &'a Env,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expr(&mut self) -> Result<Value, String> {
        self.concat()
    }

    fn concat(&mut self) -> Result<Value, String> {
        let mut lhs = self.additive()?;
        while matches!(self.peek(), Some(Tok::Amp)) {
            self.bump();
            let rhs = self.additive()?;
            lhs = Value::Str(format!("{}{}", lhs.clone().coerce_str(), rhs.coerce_str()));
        }
        Ok(lhs)
    }

    fn additive(&mut self) -> Result<Value, String> {
        let mut lhs = self.multiplicative()?;
        loop {
            match self.peek() {
                Some(Tok::Plus) => {
                    self.bump();
                    let rhs = self.multiplicative()?;
                    lhs = num_bin(lhs, rhs, |a, b| a + b)?;
                }
                Some(Tok::Minus) => {
                    self.bump();
                    let rhs = self.multiplicative()?;
                    lhs = num_bin(lhs, rhs, |a, b| a - b)?;
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn multiplicative(&mut self) -> Result<Value, String> {
        let mut lhs = self.power()?;
        loop {
            match self.peek() {
                Some(Tok::Star) => {
                    self.bump();
                    let rhs = self.power()?;
                    lhs = num_bin(lhs, rhs, |a, b| a * b)?;
                }
                Some(Tok::Slash) => {
                    self.bump();
                    let rhs = self.power()?;
                    lhs = num_bin(lhs, rhs, |a, b| a / b)?;
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn power(&mut self) -> Result<Value, String> {
        let base = self.unary()?;
        if matches!(self.peek(), Some(Tok::Caret)) {
            self.bump();
            let exp = self.power()?;
            return num_bin(base, exp, |a, b| a.powf(b));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.bump();
                let v = self.unary()?;
                Ok(Value::Num(-v.as_f64().ok_or("negating string")?))
            }
            Some(Tok::Plus) => {
                self.bump();
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> Result<Value, String> {
        match self.bump() {
            Some(Tok::Num(n)) => Ok(Value::Num(n)),
            Some(Tok::Str(s)) => Ok(Value::Str(s)),
            Some(Tok::Ident(name)) => {
                // Unit-suffixed idents like `0.5DL` arrive as Num + Ident;
                // bare idents resolve from the environment.
                self.env
                    .get(&name)
                    .map(|v| Value::Num(*v))
                    .ok_or_else(|| format!("unknown cell '{name}'"))
            }
            Some(Tok::LParen) => {
                let v = self.expr()?;
                match self.bump() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err("expected ')'".into()),
                }
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

fn num_bin(a: Value, b: Value, f: impl Fn(f64, f64) -> f64) -> Result<Value, String> {
    let (a, b) = (
        a.as_f64().ok_or("numeric op on string")?,
        b.as_f64().ok_or("numeric op on string")?,
    );
    Ok(Value::Num(f(a, b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        let mut e = Env::new();
        e.insert("Width".into(), 2.0);
        e.insert("Height".into(), 1.0);
        e.insert("Geometry1.X1".into(), 0.5);
        e
    }

    #[test]
    fn arithmetic_and_refs() {
        assert_eq!(eval("Width*0.5", &env()).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("Width/Height*1", &env()).unwrap().as_f64(), Some(2.0));
        assert_eq!(eval("Geometry1.X1+1", &env()).unwrap().as_f64(), Some(1.5));
        assert_eq!(eval("-Width+5", &env()).unwrap().as_f64(), Some(3.0));
        assert_eq!(eval("2^3", &env()).unwrap().as_f64(), Some(8.0));
    }

    #[test]
    fn strings() {
        let v = eval("\"R\"&1", &env()).unwrap();
        assert_eq!(v, Value::Str("R1".into()));
    }

    #[test]
    fn unit_literals_convert_to_internal_units() {
        assert_eq!(eval("0.24PT", &env()).unwrap().as_f64(), Some(0.24 / 72.0));
        assert_eq!(eval("1PT+1PT", &env()).unwrap().as_f64(), Some(2.0 / 72.0));
        assert_eq!(
            eval("90DEG", &env()).unwrap().as_f64(),
            Some(90.0 * std::f64::consts::PI / 180.0)
        );
        let mm = eval("25.4MM", &env()).unwrap().as_f64().unwrap();
        assert!((mm - 1.0).abs() < 1e-12);
        // Unit-less (DL/local) literals are unchanged.
        assert_eq!(eval("Width-0.5DL", &env()).unwrap().as_f64(), Some(1.5));
    }

    #[test]
    fn unsupported_falls_back_to_err() {
        assert!(eval("THEMEVAL(\"LineWeight\",0.24PT)", &env()).is_err());
        assert!(eval("Inh", &env()).is_err());
    }
}
