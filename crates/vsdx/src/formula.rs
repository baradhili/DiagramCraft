//! Minimal ShapeSheet formula evaluator.
//!
//! Supports the subset needed for geometry/size evaluation (Analysis doc 06):
//! numeric literals, cell references (`Width`, `Geometry1.X1`,
//! `Sheet.5!Width` cross-sheet refs), `+ - * / ^`, comparison operators
//! (`= <> < <= > >=`), parentheses, unary minus, string literals with `&`
//! concatenation, `&<n>` number-to-string coercion, and the control
//! functions master XForms lean on (`IF`, `OR`, `AND`, `NOT`, `GUARD`,
//! `THEMEGUARD`). Other functions (`THEMEVAL`, `SETF`, …) are not
//! evaluated; callers should fall back to the cached `V` value.

use std::collections::HashMap;

pub type Env = HashMap<String, f64>;

/// Text measurement bridge for `TEXTWIDTH`/`TEXTHEIGHT` (GitHub #147).
/// The evaluator lives in this bottom crate while the metrics engine
/// (`dc::textmetrics`) lives further up; callers that CAN measure
/// implement this trait and pass it to [`eval_measured`], and the two
/// functions evaluate against real font metrics instead of erroring
/// back to the shape's cached `V`. [`eval`] keeps the no-metrics
/// behavior: `TEXTWIDTH`/`TEXTHEIGHT`/`TheText` are unsupported there
/// and callers fall back to the cached value.
pub trait TextMeasure {
    /// The sheet's `TheText` — the composed text the shape carries.
    fn the_text(&self) -> Option<String>;
    /// `TEXTWIDTH(text)`: single-line width of `text` in the shape's
    /// resolved character formatting, internal inches.
    fn text_width(&self, text: &str) -> Option<f64>;
    /// `TEXTHEIGHT(text, width)`: height of `text` wrapped at `width`
    /// internal inches (the same wrap the renderer performs — a
    /// trailing line break does NOT add a line, #148).
    fn text_height(&self, text: &str, width: f64) -> Option<f64>;
}

/// [`TextMeasure`] for callers without a metrics engine: every method
/// is unavailable, so text functions fail evaluation and callers use
/// the cached `V` (the pre-#147 behavior).
struct NoText;

impl TextMeasure for NoText {
    fn the_text(&self) -> Option<String> {
        None
    }
    fn text_width(&self, _text: &str) -> Option<f64> {
        None
    }
    fn text_height(&self, _text: &str, _width: f64) -> Option<f64> {
        None
    }
}

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
    Comma,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
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
/// can fall back to the cached value. Text functions
/// (`TEXTWIDTH`/`TEXTHEIGHT`/`TheText`) are unsupported here — use
/// [`eval_measured`] with a [`TextMeasure`] implementation.
pub fn eval(expr: &str, env: &Env) -> Result<Value, String> {
    eval_measured(expr, env, &NoText)
}

/// [`eval`] with a metrics bridge: `TheText` resolves to the sheet's
/// text and `TEXTWIDTH`/`TEXTHEIGHT` measure through `m` (#147). Any
/// measurement the bridge cannot supply still errors, so callers fall
/// back to the cached `V` exactly as before.
pub fn eval_measured(expr: &str, env: &Env, m: &dyn TextMeasure) -> Result<Value, String> {
    let toks = tokenize(expr)?;
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        env,
        measure: m,
    };
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
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '=' => {
                toks.push(Tok::Eq);
                i += 1;
            }
            '<' => {
                // `<>` is Visio's not-equal; `<=` less-or-equal.
                if i + 1 < b.len() {
                    match b[i + 1] as char {
                        '>' => {
                            toks.push(Tok::Ne);
                            i += 2;
                            continue;
                        }
                        '=' => {
                            toks.push(Tok::Le);
                            i += 2;
                            continue;
                        }
                        _ => {}
                    }
                }
                toks.push(Tok::Lt);
                i += 1;
            }
            '>' => {
                if i + 1 < b.len() && b[i + 1] as char == '=' {
                    toks.push(Tok::Ge);
                    i += 2;
                } else {
                    toks.push(Tok::Gt);
                    i += 1;
                }
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
                let mut digits_end = i;
                // Scientific notation: `E`/`e` followed by an optional
                // sign and digits is an exponent, not a unit suffix —
                // shipped masters carry cells like `Height*9.2184E-5`
                // whose misparse as `Height*9.218-5` throws geometry
                // inches off the shape (issue #15).
                if i < b.len() && matches!(b[i] as char, 'E' | 'e') {
                    let mut j = i + 1;
                    if j < b.len() && matches!(b[j] as char, '+' | '-') {
                        j += 1;
                    }
                    if j < b.len() && (b[j] as char).is_ascii_digit() {
                        while j < b.len() && (b[j] as char).is_ascii_digit() {
                            j += 1;
                        }
                        digits_end = j;
                        i = j;
                    }
                }
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
                    // `!` keeps `Sheet.5!Width` / `Sheet.5!User.X` refs one
                    // token (Visio has no other use of `!`; not-equal is `<>`).
                    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '!' {
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
    measure: &'a dyn TextMeasure,
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
        self.comparison()
    }

    /// Comparisons are Visio's lowest-precedence operators and yield
    /// 1/0 (ShapeSheet booleans are numbers).
    fn comparison(&mut self) -> Result<Value, String> {
        let mut lhs = self.concat()?;
        loop {
            let which = match self.peek() {
                Some(Tok::Eq) => 0,
                Some(Tok::Ne) => 1,
                Some(Tok::Lt) => 2,
                Some(Tok::Le) => 3,
                Some(Tok::Gt) => 4,
                Some(Tok::Ge) => 5,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.concat()?;
            lhs = match (lhs.as_f64(), rhs.as_f64()) {
                (Some(a), Some(b)) => {
                    let r = match which {
                        0 => a == b,
                        1 => a != b,
                        2 => a < b,
                        3 => a <= b,
                        4 => a > b,
                        _ => a >= b,
                    };
                    Value::Num(f64::from(r as i32))
                }
                // Non-numeric operands: only (in)equality is meaningful.
                _ => match which {
                    0 => Value::Num(f64::from(
                        (lhs.clone().coerce_str() == rhs.clone().coerce_str()) as i32,
                    )),
                    1 => Value::Num(f64::from(
                        (lhs.clone().coerce_str() != rhs.clone().coerce_str()) as i32,
                    )),
                    _ => return Err("ordering comparison on strings".into()),
                },
            };
        }
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
                if matches!(self.peek(), Some(Tok::LParen)) {
                    return self.call(&name);
                }
                // `TheText` is the sheet's composed text — a STRING,
                // so it only resolves under a measurement bridge
                // (#147); plain `eval` leaves it (and the text
                // functions taking it) to the cached-V fallback.
                if name == "TheText" {
                    return self
                        .measure
                        .the_text()
                        .map(Value::Str)
                        .ok_or_else(|| "TheText: no text metrics bridge".to_string());
                }
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

    /// The function subset master XForms use (issue #15): `GUARD(x)` /
    /// `THEMEGUARD(x)` protect an expression and evaluate to it; `IF` /
    /// `OR` / `AND` / `NOT` are the control flow behind layout switches
    /// like `User.IconY`. Unknown functions error so callers fall back
    /// to the cached `V`.
    fn call(&mut self, name: &str) -> Result<Value, String> {
        self.bump(); // '('
        let mut args = Vec::new();
        if !matches!(self.peek(), Some(Tok::RParen)) {
            loop {
                args.push(self.expr()?);
                match self.bump() {
                    Some(Tok::Comma) => continue,
                    Some(Tok::RParen) => break,
                    _ => return Err(format!("expected ',' or ')' in {name}(...)")),
                }
            }
        } else {
            self.bump(); // ')'
        }
        let arg = |args: &[Value], i: usize| -> Result<f64, String> {
            args.get(i)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| format!("{name}: arg {} not numeric", i + 1))
        };
        let upper_name = name.to_ascii_uppercase();
        match upper_name.as_str() {
            "GUARD" | "THEMEGUARD" => match args.len() {
                1 => Ok(args.pop().unwrap()),
                _ => Err(format!("{name} takes one argument")),
            },
            "IF" => {
                if args.len() != 3 {
                    return Err("IF takes three arguments".into());
                }
                // IF(cond, then, else): branches are already evaluated
                // (this evaluator has no AST); pick by the condition.
                let cond = arg(&args, 0)?;
                Ok(if cond != 0.0 {
                    args.into_iter().nth(1).unwrap()
                } else {
                    args.into_iter().nth(2).unwrap()
                })
            }
            "OR" => {
                if args.is_empty() {
                    return Err("OR needs arguments".into());
                }
                let r: bool = (0..args.len())
                    .try_fold(false, |acc, i| -> Result<bool, String> {
                        Ok(acc || arg(&args, i)? != 0.0)
                    })?;
                Ok(Value::Num(f64::from(r as i32)))
            }
            "AND" => {
                if args.is_empty() {
                    return Err("AND needs arguments".into());
                }
                let r: bool = (0..args.len()).try_fold(true, |acc, i| -> Result<bool, String> {
                    Ok(acc && arg(&args, i)? != 0.0)
                })?;
                Ok(Value::Num(f64::from(r as i32)))
            }
            "NOT" => {
                if args.len() != 1 {
                    return Err("NOT takes one argument".into());
                }
                Ok(Value::Num(f64::from((arg(&args, 0)? == 0.0) as i32)))
            }
            // MAX/MIN: the corpus's text-aware blocks compose them
            // around the text functions (`MAX(Width,TEXTWIDTH(TheText))`,
            // GDT's `MAX(3*Char.Size*Scratch.B1,TEXTWIDTH(TheText))`).
            // `upper` keeps the selection case-insensitive like every
            // other arm (CodeRabbit: comparing `name` made `max(...)`
            // silently take the MIN branch).
            "MAX" | "MIN" => {
                if args.is_empty() {
                    return Err(format!("{name} needs arguments"));
                }
                let is_max = upper_name == "MAX";
                let mut best = arg(&args, 0)?;
                for i in 1..args.len() {
                    let v = arg(&args, i)?;
                    let take = if is_max { v > best } else { v < best };
                    if take {
                        best = v;
                    }
                }
                Ok(Value::Num(best))
            }
            // Text measurement (#147): the args are already evaluated
            // (TheText arrives as a Str through the bridge), the
            // measurement itself defers to the bridge.
            "TEXTWIDTH" => {
                if args.len() != 1 {
                    return Err("TEXTWIDTH takes one argument".into());
                }
                let text = args[0].clone().coerce_str();
                self.measure
                    .text_width(&text)
                    .map(Value::Num)
                    .ok_or_else(|| "TEXTWIDTH: no text metrics bridge".to_string())
            }
            "TEXTHEIGHT" => {
                if args.len() != 2 {
                    return Err("TEXTHEIGHT takes two arguments".into());
                }
                let text = args[0].clone().coerce_str();
                let width = arg(&args, 1)?;
                self.measure
                    .text_height(&text, width)
                    .map(Value::Num)
                    .ok_or_else(|| "TEXTHEIGHT: no text metrics bridge".to_string())
            }
            other => Err(format!("function '{other}' not supported")),
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
    fn scientific_notation_literals() {
        // The Role-callout specimen (issue #15): `Height*9.2184490821108E-5`
        // misparsed as `Height*9.2184-5` and threw an arc control point
        // four inches below a 0.1-inch shape.
        let e = env(); // Height = 1
        assert_eq!(
            eval("Height*9.2184490821108E-5", &e).unwrap().as_f64(),
            Some(9.2184490821108E-5)
        );
        assert_eq!(eval("1E2", &e).unwrap().as_f64(), Some(100.0));
        assert_eq!(eval("2.5e+3", &e).unwrap().as_f64(), Some(2500.0));
        assert_eq!(eval("1E-2PT", &e).unwrap().as_f64(), Some(0.01 / 72.0));
        // A trailing `E` that is not an exponent still falls back to the
        // (unknown → 1.0) unit factor rather than breaking the number.
        assert_eq!(eval("1.5", &e).unwrap().as_f64(), Some(1.5));
    }

    #[test]
    fn unsupported_falls_back_to_err() {
        assert!(eval("THEMEVAL(\"LineWeight\",0.24PT)", &env()).is_err());
        assert!(eval("Inh", &env()).is_err());
        // Without a metrics bridge the text functions stay unsupported
        // (the pre-#147 contract: callers fall back to the cached V).
        assert!(eval("TEXTWIDTH(TheText)", &env()).is_err());
        assert!(eval("TEXTHEIGHT(TheText,TxtWidth)", &env()).is_err());
    }

    /// A deterministic fake bridge: width = 0.1×chars, height = the
    /// width passed in (so tests can see which width the function
    /// forwarded).
    struct FakeText;

    impl TextMeasure for FakeText {
        fn the_text(&self) -> Option<String> {
            Some("hello\r\n".into())
        }
        fn text_width(&self, text: &str) -> Option<f64> {
            Some(0.1 * text.trim_end().chars().count() as f64)
        }
        fn text_height(&self, _text: &str, width: f64) -> Option<f64> {
            Some(width * 2.0)
        }
    }

    #[test]
    fn text_functions_evaluate_through_a_bridge() {
        let e = env();
        let m = FakeText;
        // TheText arrives as the sheet's string; TEXTWIDTH measures it.
        assert_eq!(
            eval_measured("TEXTWIDTH(TheText)", &e, &m)
                .unwrap()
                .as_f64(),
            Some(0.5)
        );
        // String literals measure too, and arithmetic composes — the
        // corpus's `MAX(Width,TEXTWIDTH(TheText))` shapes evaluate
        // end to end.
        assert_eq!(
            eval_measured("TEXTWIDTH(\"abc\")+1", &e, &m).unwrap().as_f64(),
            Some(1.3)
        );
        let mut e2 = env();
        e2.insert("Width".into(), 0.75);
        assert_eq!(
            eval_measured("MAX(Width, TEXTWIDTH(TheText))", &e2, &m)
                .unwrap()
                .as_f64(),
            Some(0.75)
        );
        assert_eq!(
            eval_measured("MIN(Width, TEXTWIDTH(TheText))", &e2, &m)
                .unwrap()
                .as_f64(),
            Some(0.5)
        );
        // Case-insensitive dispatch selects the right function
        // (CodeRabbit: `max(...)` used to take the MIN branch).
        assert_eq!(eval("max(1, 2)", &e2).unwrap().as_f64(), Some(2.0));
        assert_eq!(eval("Min(3, 2)", &e2).unwrap().as_f64(), Some(2.0));
        // TEXTHEIGHT forwards the resolved width argument.
        assert_eq!(
            eval_measured("TEXTHEIGHT(TheText, 0.25)", &e, &m)
                .unwrap()
                .as_f64(),
            Some(0.5)
        );
        // GUARD composes transparently over the text functions.
        assert_eq!(
            eval_measured("GUARD(TEXTHEIGHT(TheText, 0.25))", &e, &m)
                .unwrap()
                .as_f64(),
            Some(0.5)
        );
        // Arity errors are real errors, not cached-V fallbacks.
        assert!(eval_measured("TEXTHEIGHT(TheText)", &e, &m).is_err());
        assert!(eval_measured("TEXTWIDTH()", &e, &m).is_err());
    }

    #[test]
    fn the_text_alone_resolves_as_a_string_under_a_bridge() {
        let e = env();
        let m = FakeText;
        let v = eval_measured("TheText&\"!\"", &e, &m).unwrap();
        assert_eq!(v, Value::Str("hello\r\n!".into()));
    }

    #[test]
    fn comparisons_yield_one_or_zero() {
        let e = env();
        assert_eq!(eval("Width=2", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("Width<>2", &e).unwrap().as_f64(), Some(0.0));
        assert_eq!(eval("Height<Width", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("Width<=2", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("Width>2", &e).unwrap().as_f64(), Some(0.0));
        assert_eq!(eval("Width>=Height*2", &e).unwrap().as_f64(), Some(1.0));
    }

    #[test]
    fn control_functions() {
        let e = env();
        assert_eq!(
            eval("IF(Width=2, 10, 20)", &e).unwrap().as_f64(),
            Some(10.0)
        );
        assert_eq!(
            eval("IF(Width=3, 10, 20)", &e).unwrap().as_f64(),
            Some(20.0)
        );
        assert_eq!(
            eval("OR(Width=3, Height=1)", &e).unwrap().as_f64(),
            Some(1.0)
        );
        assert_eq!(
            eval("OR(Width=3, Height=2)", &e).unwrap().as_f64(),
            Some(0.0)
        );
        assert_eq!(
            eval("AND(Width=2, Height=1)", &e).unwrap().as_f64(),
            Some(1.0)
        );
        assert_eq!(eval("NOT(Width=3)", &e).unwrap().as_f64(), Some(1.0));
        // GUARD transparently yields its protected expression.
        assert_eq!(
            eval("GUARD(4MM)", &e).unwrap().as_f64(),
            Some(4.0 * 0.03937007874015748)
        );
        assert_eq!(
            eval("THEMEGUARD(Width*0.5)", &e).unwrap().as_f64(),
            Some(1.0)
        );
        // Unknown functions still error (callers fall back to cached V).
        assert!(eval("SETF(GetRef(Width), 1)", &e).is_err());
    }

    #[test]
    fn cross_sheet_references_resolve_from_env() {
        // The Role-callout specimen (issue #15): a child stretched with its
        // group evaluates `Sheet.5!Width*1` against the instance width.
        let mut e = env();
        e.insert("Sheet.5!Width".into(), 2.657480296178819);
        e.insert("Sheet.5!User.InfoPanelHeight".into(), 0.2362204724409449);
        assert_eq!(
            eval("Sheet.5!Width*1", &e).unwrap().as_f64(),
            Some(2.657480296178819)
        );
        assert_eq!(
            eval("GUARD(Sheet.5!User.InfoPanelHeight)", &e)
                .unwrap()
                .as_f64(),
            Some(0.2362204724409449)
        );
    }

    #[test]
    fn role_icon_y_specimen() {
        // User.IconY from master5.xml, verbatim shape:
        // IF(OR(User.ShapeLayout<>1), Height-User.InfoPanelHeight/2, User.InfoPanelHeight/2)
        let mut e = env();
        e.insert("User.ShapeLayout".into(), 0.0);
        e.insert("User.InfoPanelHeight".into(), 0.2362204724409449);
        // Height=1 from env(): 1 - 0.1181…
        let v = eval(
            "IF(OR(User.ShapeLayout<>1), Height-User.InfoPanelHeight/2, User.InfoPanelHeight/2)",
            &e,
        )
        .unwrap()
        .as_f64()
        .unwrap();
        assert!((v - (1.0 - 0.2362204724409449 / 2.0)).abs() < 1e-12);
        e.insert("User.ShapeLayout".into(), 1.0);
        let v = eval(
            "IF(OR(User.ShapeLayout<>1), Height-User.InfoPanelHeight/2, User.InfoPanelHeight/2)",
            &e,
        )
        .unwrap()
        .as_f64()
        .unwrap();
        assert!((v - 0.2362204724409449 / 2.0).abs() < 1e-12);
    }
}
