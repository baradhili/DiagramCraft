//! Minimal ShapeSheet formula evaluator.
//!
//! Supports the subset needed for geometry/size evaluation (Analysis doc 06)
//! plus the #122/FS-033 breadth slice: numeric literals, cell references
//! (`Width`, `Geometry1.X1`, `Sheet.5!Width` cross-sheet refs), `+ - * / ^`,
//! comparison operators (`= <> < <= > >=`), parentheses, unary minus, string
//! literals with `&` concatenation, `&<n>` number-to-string coercion, the
//! control functions master XForms lean on (`IF`, `OR`, `AND`, `NOT`,
//! `GUARD`, `THEMEGUARD`), the math set the corpus composes with them
//! (`ABS`, `ATAN2`, `CEILING`, …), string functions (`LEFT`, `LEN`, …),
//! the color functions over the #122 color model (`RGB`, `HSL`, `TINT`,
//! `SHADE`, …), and the guide-algebra pair `INTERSECTX`/`INTERSECTY`.
//! Still-unevaluated functions (`THEMEVAL`, `SETF`, …) error so callers
//! fall back to the cached `V` value.

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

    /// A numeric single-argument helper the breadth arms dispatch to.
    fn call_1(
        &self,
        name: &str,
        args: &[Value],
        f: impl Fn(f64) -> Result<f64, String>,
    ) -> Result<Value, String> {
        if args.len() != 1 {
            return Err(format!("{name} takes one argument"));
        }
        f(args[0]
            .as_f64()
            .ok_or_else(|| format!("{name}: argument not numeric"))?)
        .map(Value::Num)
    }

    /// The function subset master XForms use (issue #15) plus the
    /// #122/FS-033 breadth slice. `GUARD(x)` / `THEMEGUARD(x)` protect
    /// an expression and evaluate to it — THEMEGUARD's guard marks the
    /// cell as theme-protected (a theme application must not overwrite
    /// it); this evaluator applies no themes, so the identity IS the
    /// guarded evaluation. `IF` / `OR` / `AND` / `NOT` are the control
    /// flow behind layout switches like `User.IconY`. Unknown functions
    /// error so callers fall back to the cached `V`.
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
            // SQRT: the 1D chord masters derive their frame width from
            // the endpoints through it (`Width=SQRT((EndX-BeginX)^2+
            // (EndY-BeginY)^2)`, the Line-arc corpus master); a
            // negative argument errors so callers fall back to the
            // cached V.
            "SQRT" => {
                if args.len() != 1 {
                    return Err(format!("{name} takes one argument"));
                }
                let v = arg(&args, 0)?;
                if v < 0.0 {
                    return Err(format!("{name} of a negative value"));
                }
                Ok(Value::Num(v.sqrt()))
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
            // ---- #122/FS-033 breadth slice ----
            // Math: the corpus composes ABS/ATAN2/CEILING/INT with the
            // control functions (utilisation.vsdx master3/master36);
            // ATAN2 takes y first ("counterclockwise from the x axis",
            // the chord master's Angle=ATAN2(EndY-BeginY,EndX-BeginX));
            // (0,0) is 0, not a domain error.
            "ABS" => self.call_1(name, &args, |x| Ok(x.abs())),
            "SIGN" => self.call_1(name, &args, |x| {
                Ok(if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                })
            }),
            "INT" => self.call_1(name, &args, |x| Ok(x.floor())),
            "EXP" => self.call_1(name, &args, okf(f64::exp)),
            "LN" => self.call_1(name, &args, |x| {
                if x <= 0.0 {
                    Err("LN of a non-positive value".into())
                } else {
                    Ok(x.ln())
                }
            }),
            "COS" => self.call_1(name, &args, okf(f64::cos)),
            "SIN" => self.call_1(name, &args, okf(f64::sin)),
            "TAN" => self.call_1(name, &args, okf(f64::tan)),
            "ACOS" => self.call_1(name, &args, |x| {
                (-1.0..=1.0)
                    .contains(&x)
                    .then(|| x.acos())
                    .ok_or_else(|| "ACOS out of range".to_string())
            }),
            "ASIN" => self.call_1(name, &args, |x| {
                (-1.0..=1.0)
                    .contains(&x)
                    .then(|| x.asin())
                    .ok_or_else(|| "ASIN out of range".to_string())
            }),
            "ATAN" => self.call_1(name, &args, okf(f64::atan)),
            "DEG" => self.call_1(name, &args, |rad| Ok(rad * 180.0 / std::f64::consts::PI)),
            "RAD" => self.call_1(name, &args, |deg| Ok(deg * std::f64::consts::PI / 180.0)),
            "PI" => {
                if !args.is_empty() {
                    return Err("PI takes no arguments".into());
                }
                Ok(Value::Num(std::f64::consts::PI))
            }
            "ATAN2" => {
                if args.len() != 2 {
                    return Err("ATAN2 takes two arguments".into());
                }
                let (y, x) = (arg(&args, 0)?, arg(&args, 1)?);
                Ok(Value::Num(if y == 0.0 && x == 0.0 {
                    0.0
                } else {
                    y.atan2(x)
                }))
            }
            "CEILING" | "FLOOR" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(format!("{name} takes one or two arguments"));
                }
                let x = arg(&args, 0)?;
                let m = args.get(1).and_then(|v| v.as_f64());
                multiple_round(x, m, upper_name == "CEILING")
            }
            "ROUND" => {
                if args.is_empty() || args.len() > 2 {
                    return Err("ROUND takes one or two arguments".into());
                }
                let x = arg(&args, 0)?;
                let digits = args.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                Ok(Value::Num(decimal_round(x, digits)))
            }
            "MOD" => {
                if args.len() != 2 {
                    return Err("MOD takes two arguments".into());
                }
                let (x, y) = (arg(&args, 0)?, arg(&args, 1)?);
                if y == 0.0 {
                    return Err("MOD of zero".into());
                }
                // Visio's remainder keeps the MAGNITUDE (the ADS_M
                // Scratch.A1 witness: −π/4 mod 2π = π/4).
                Ok(Value::Num((x % y).abs()))
            }
            "LOG" => {
                if args.is_empty() || args.len() > 2 {
                    return Err("LOG takes one or two arguments".into());
                }
                let x = arg(&args, 0)?;
                let base = args.get(1).and_then(|v| v.as_f64()).unwrap_or(10.0);
                if x <= 0.0 || base <= 0.0 || base == 1.0 {
                    return Err("LOG out of domain".into());
                }
                Ok(Value::Num(x.log(base)))
            }
            // String functions (#122 names the set; LEFT/LEN/MID/RIGHT
            // are corpus-evidenced in utilisation.vsdx). Positions are
            // 1-based and character-counted like LEN.
            "LEN" => {
                if args.len() != 1 {
                    return Err("LEN takes one argument".into());
                }
                Ok(Value::Num(args[0].clone().coerce_str().chars().count() as f64))
            }
            "LEFT" | "RIGHT" => {
                if args.len() != 2 {
                    return Err(format!("{name} takes two arguments"));
                }
                let text = args[0].clone().coerce_str();
                let n = arg(&args, 1)?;
                if n < 0.0 {
                    return Err(format!("{name} of a negative count"));
                }
                let n = (n as usize).min(text.chars().count());
                let s: String = if upper_name == "LEFT" {
                    text.chars().take(n).collect()
                } else {
                    let skip = text.chars().count() - n;
                    text.chars().skip(skip).collect()
                };
                Ok(Value::Str(s))
            }
            "MID" => {
                if args.len() != 3 {
                    return Err("MID takes three arguments".into());
                }
                let text = args[0].clone().coerce_str();
                let start = arg(&args, 1)?;
                let count = arg(&args, 2)?;
                if start < 1.0 || count < 0.0 {
                    return Err("MID out of domain".into());
                }
                Ok(Value::Str(
                    text.chars()
                        .skip((start - 1.0) as usize)
                        .take(count as usize)
                        .collect(),
                ))
            }
            "TRIM" => {
                if args.len() != 1 {
                    return Err("TRIM takes one argument".into());
                }
                let text = args[0].clone().coerce_str();
                // Visio's TRIM: single spaces between words, none
                // leading/trailing.
                Ok(Value::Str(
                    text.split_whitespace().collect::<Vec<_>>().join(" "),
                ))
            }
            "UPPER" => {
                if args.len() != 1 {
                    return Err("UPPER takes one argument".into());
                }
                Ok(Value::Str(args[0].clone().coerce_str().to_uppercase()))
            }
            "LOWER" => {
                if args.len() != 1 {
                    return Err("LOWER takes one argument".into());
                }
                Ok(Value::Str(args[0].clone().coerce_str().to_lowercase()))
            }
            "SUBSTITUTE" => {
                if args.len() != 3 {
                    return Err("SUBSTITUTE takes three arguments".into());
                }
                let text = args[0].clone().coerce_str();
                let old = args[1].clone().coerce_str();
                let new = args[2].clone().coerce_str();
                if old.is_empty() {
                    return Ok(Value::Str(text));
                }
                Ok(Value::Str(text.replace(&old, &new)))
            }
            "FIND" => {
                if args.len() < 2 || args.len() > 4 {
                    return Err("FIND takes two to four arguments".into());
                }
                let needle = args[0].clone().coerce_str();
                let haystack = args[1].clone().coerce_str();
                let start = args.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0);
                let ignore_case = args
                    .get(3)
                    .and_then(|v| v.as_f64())
                    .map(|v| v != 0.0)
                    .unwrap_or(false);
                // The start range checks the ORIGINAL haystack; the
                // search bounds off the SEARCHED string's char count —
                // case folding can change the count (İ), and a needle
                // longer than the haystack is simply not found (#218
                // review: `saturating_sub` + a slice could panic).
                if start < 1.0 || start > haystack.chars().count() as f64 {
                    return Err("FIND start out of range".into());
                }
                let skip = (start - 1.0) as usize;
                let (hay, needle) = if ignore_case {
                    (haystack.to_lowercase(), needle.to_lowercase())
                } else {
                    (haystack, needle)
                };
                let hay: Vec<char> = hay.chars().collect();
                let needle: Vec<char> = needle.chars().collect();
                if needle.is_empty() {
                    return Ok(Value::Num(start));
                }
                // `checked_sub` makes a needle longer than the
                // haystack an empty search range (never a slice panic).
                let at = hay
                    .len()
                    .checked_sub(needle.len())
                    .and_then(|last| (skip..=last).find(|&i| hay[i..i + needle.len()] == needle[..]));
                match at {
                    Some(i) => Ok(Value::Num(i as f64 + 1.0)),
                    None => Err("FIND: text not found".into()),
                }
            }
            "VALUE" => {
                if args.len() != 1 {
                    return Err("VALUE takes one argument".into());
                }
                let text = args[0].clone().coerce_str();
                text.trim()
                    .parse::<f64>()
                    .map(Value::Num)
                    .map_err(|_| "VALUE of a non-numeric string".to_string())
            }
            // Color functions over the #122 color model: a color is a
            // number — 0–23 the fixed [MS-VSDX] palette, ≥24 a packed
            // 0xRRGGBB literal. HSL's integer path reproduces the
            // legacy 240-step grid exactly (HSL(144,116,125) =
            // #4979C0, witnessed against a shipped stencil master);
            // TINT/SHADE apply the measured endpoint rule.
            "RGB" | "HSL" => {
                if args.len() != 3 {
                    return Err(format!("{name} takes three arguments"));
                }
                let (a, b, c) = (arg(&args, 0)?, arg(&args, 1)?, arg(&args, 2)?);
                pack_color(if upper_name == "RGB" {
                    rgb_from_channels(a, b, c)?
                } else {
                    hsl_to_channels(a, b, c)?
                })
            }
            "RED" | "GREEN" | "BLUE" => {
                if args.len() != 1 {
                    return Err(format!("{name} takes one argument"));
                }
                let ch = match color_channels(arg(&args, 0)?) {
                    Some([r, g, b]) => match upper_name.as_str() {
                        "RED" => f64::from(r),
                        "GREEN" => f64::from(g),
                        _ => f64::from(b),
                    },
                    None => 0.0, // an invalid color extracts as 0
                };
                Ok(Value::Num(ch))
            }
            "HUE" | "SAT" | "LUM" => {
                if args.len() != 1 {
                    return Err(format!("{name} takes one argument"));
                }
                let v = match color_channels(arg(&args, 0)?) {
                    Some(rgb) => hsl_components(rgb),
                    None => [0.0; 3],
                };
                let slot = match upper_name.as_str() {
                    "HUE" => 0,
                    "SAT" => 1,
                    _ => 2,
                };
                let mut out = v[slot].round();
                if slot == 0 {
                    out = out.rem_euclid(240.0);
                }
                Ok(Value::Num(out))
            }
            "TINT" | "SHADE" => {
                if args.len() != 2 {
                    return Err(format!("{name} takes two arguments"));
                }
                let color = arg(&args, 0)?;
                let amount = arg(&args, 1)?.trunc();
                // SHADE is TINT with the opposite signed adjustment —
                // positive moves toward black.
                let amount = if upper_name == "SHADE" { -amount } else { amount };
                let Some(rgb) = color_channels(color) else {
                    return Err(format!("{name} of an invalid color"));
                };
                let [h, s, l] = hsl_components(rgb);
                let (h, s, l) = (h.round().rem_euclid(240.0), s.round(), l.round().clamp(0.0, 240.0));
                if amount == 0.0 {
                    return pack_color(rgb);
                }
                // The measured endpoint rule: within 100 steps of the
                // approached endpoint the adjustment scales by the
                // remaining distance, then truncates.
                let distance = if amount > 0.0 { 240.0 - l } else { l };
                let adjustment =
                    (amount.clamp(-240.0, 240.0) * distance.min(100.0) / 100.0).trunc();
                let l2 = (l + adjustment).clamp(0.0, 240.0);
                pack_color(hsl_to_channels(h, s, l2)?)
            }
            // No theme engine: the measured no-theme branch (inherited
            // 65534 sentinel, no page theme) is false.
            "ISTHEMED" => {
                if !args.is_empty() {
                    return Err("ISTHEMED takes no arguments".into());
                }
                Ok(Value::Num(0.0))
            }
            // Guide algebra (#122 parity evidence): the intersection of
            // the line (x1,y1)-(x2,y2) with the line (x3,y3)-(x4,y4),
            // treated as infinite lines. Parallel lines error so the
            // caller falls back to the cached V.
            "INTERSECTX" | "INTERSECTY" => {
                if args.len() != 8 {
                    return Err(format!("{name} takes eight arguments"));
                }
                let v: Vec<f64> = (0..8).map(|i| arg(&args, i)).collect::<Result<_, _>>()?;
                let (x1, y1, x2, y2, x3, y3, x4, y4) =
                    (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
                let denom = (x2 - x1) * (y4 - y3) - (y2 - y1) * (x4 - x3);
                if denom.abs() < 1e-12 {
                    return Err(format!("{name}: parallel lines"));
                }
                let t = ((x3 - x1) * (y4 - y3) - (y3 - y1) * (x4 - x3)) / denom;
                let (ix, iy) = (x1 + t * (x2 - x1), y1 + t * (y2 - y1));
                Ok(Value::Num(if upper_name == "INTERSECTX" { ix } else { iy }))
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

fn okf(f: impl Fn(f64) -> f64) -> impl Fn(f64) -> Result<f64, String> {
    move |x| Ok(f(x))
}

/// CEILING/FLOOR: to a same-sign multiple, away from / toward zero.
/// A sign-mismatch pair errors; x=0 or multiple=0 is 0; the multiple
/// defaults to ±1 (x's sign).
fn multiple_round(x: f64, m: Option<f64>, away: bool) -> Result<Value, String> {
    let m = m.unwrap_or(x.signum());
    if x == 0.0 || m == 0.0 {
        return Ok(Value::Num(0.0));
    }
    if x.signum() != m.signum() {
        return Err("CEILING/FLOOR sign mismatch".into());
    }
    let q = x / m;
    Ok(Value::Num((if away { q.ceil() } else { q.floor() }) * m))
}

/// ROUND at a decimal precision — negative precision counts left of
/// the decimal point; ties round away from zero (f64::round).
fn decimal_round(x: f64, digits: f64) -> f64 {
    if digits >= 309.0 {
        return x;
    }
    if digits <= -309.0 {
        return 0.0;
    }
    let scale = 10.0f64.powf(digits.trunc());
    let n = x * scale;
    if !n.is_finite() {
        return x;
    }
    n.round() / scale
}

// ---- The #122 color model ----
// A color is an f64: 0–23 the fixed [MS-VSDX] standard palette,
// ≥24 a packed 0xRRGGBB literal. Fractions are invalid.

/// The specification-fixed standard palette (indices 0–23); custom
/// colors start at 24.
const STANDARD_PALETTE: [[u8; 3]; 24] = [
    [0, 0, 0],
    [255, 255, 255],
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [255, 0, 255],
    [0, 255, 255],
    [128, 0, 0],
    [0, 128, 0],
    [0, 0, 128],
    [128, 128, 0],
    [128, 0, 128],
    [0, 128, 128],
    [192, 192, 192],
    [230, 230, 230],
    [205, 205, 205],
    [179, 179, 179],
    [154, 154, 154],
    [128, 128, 128],
    [102, 102, 102],
    [77, 77, 77],
    [51, 51, 51],
    [26, 26, 26],
];

/// The RGB channels of a color number, or None when invalid.
fn color_channels(color: f64) -> Option<[u8; 3]> {
    if !color.is_finite() || color < 0.0 || color.fract() != 0.0 || color > f64::from(u32::MAX) {
        return None;
    }
    let n = color as u32;
    if n < 24 {
        Some(STANDARD_PALETTE[n as usize])
    } else {
        Some([
            ((n >> 16) & 0xFF) as u8,
            ((n >> 8) & 0xFF) as u8,
            (n & 0xFF) as u8,
        ])
    }
}

/// Pack 0xRRGGBB channels into the color number.
fn pack_color(rgb: [u8; 3]) -> Result<Value, String> {
    Ok(Value::Num(f64::from(
        (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]),
    )))
}

/// Clamped 0–255 channels from RGB()'s arguments.
fn rgb_from_channels(r: f64, g: f64, b: f64) -> Result<[u8; 3], String> {
    let cl = |v: f64| -> Result<u8, String> {
        if !v.is_finite() {
            return Err("RGB of a non-finite value".into());
        }
        Ok(v.round().clamp(0.0, 255.0) as u8)
    };
    Ok([cl(r)?, cl(g)?, cl(b)?])
}

/// HSL(h, s, l) on the 0–240 scale → RGB channels. Integral triples go
/// through the legacy 240-step integer grid (HSL(144,116,125) =
/// #4979C0, witnessed against a shipped stencil master); fractional
/// triples use the continuous conversion with rounded channels.
fn hsl_to_channels(h: f64, s: f64, l: f64) -> Result<[u8; 3], String> {
    if !(0.0..240.0).contains(&h) || !(0.0..=240.0).contains(&s) || !(0.0..=240.0).contains(&l) {
        return Err("HSL out of range".into());
    }
    if h.fract() == 0.0 && s.fract() == 0.0 && l.fract() == 0.0 {
        return Ok(integer_hsl(h as u32, s as u32, l as u32));
    }
    let (h, s, l) = (h / 40.0, s / 240.0, l / 240.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let rgb = if h < 1.0 {
        [c, x, 0.0]
    } else if h < 2.0 {
        [x, c, 0.0]
    } else if h < 3.0 {
        [0.0, c, x]
    } else if h < 4.0 {
        [0.0, x, c]
    } else if h < 5.0 {
        [x, 0.0, c]
    } else {
        [c, 0.0, x]
    };
    Ok(rgb.map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8))
}

/// The legacy 240-step integer interpolation: upper/lower luminosity
/// rails, then per-channel interpolation at the hue offsets
/// (R: h+80, G: h, B: h+160 mod 240), each scaled back to 0–255.
fn integer_hsl(h: u32, s: u32, l: u32) -> [u8; 3] {
    if s == 0 {
        return [((l * 255 + 120) / 240) as u8; 3];
    }
    let upper = if l <= 120 {
        (l * (240 + s) + 120) / 240
    } else {
        l + s - (l * s + 120) / 240
    };
    let lower = 2 * l - upper;
    let channel = |h: u32| {
        let h = h % 240;
        let c = if h < 40 {
            lower + ((upper - lower) * h + 20) / 40
        } else if h < 120 {
            upper
        } else if h < 160 {
            lower + ((upper - lower) * (160 - h) + 20) / 40
        } else {
            lower
        };
        ((c * 255 + 120) / 240) as u8
    };
    [channel(h + 80), channel(h), channel(h + 160)]
}

/// An RGB triple's HSL components on the 0–240 scale (HUE/SAT/LUM and
/// the TINT/SHADE endpoint rule decompose through this).
fn hsl_components(rgb: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(|v| f64::from(v) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let c = max - min;
    let l = (max + min) / 2.0;
    if c == 0.0 {
        return [0.0, 0.0, l * 240.0];
    }
    let h = if max == r {
        ((g - b) / c).rem_euclid(6.0)
    } else if max == g {
        (b - r) / c + 2.0
    } else {
        (r - g) / c + 4.0
    };
    [
        h * 40.0,
        c / (1.0 - (2.0 * l - 1.0).abs()) * 240.0,
        l * 240.0,
    ]
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
            eval_measured("TEXTWIDTH(\"abc\")+1", &e, &m)
                .unwrap()
                .as_f64(),
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
    fn sqrt_serves_the_chord_masters() {
        let mut e = env();
        e.insert("BeginX".into(), 1.0);
        e.insert("BeginY".into(), 6.0);
        e.insert("EndX".into(), 3.5);
        e.insert("EndY".into(), 6.2);
        // The Line-arc corpus master's Width formula: the endpoint
        // chord, not the signed dx.
        assert_eq!(
            eval("SQRT((EndX-BeginX)^2+(EndY-BeginY)^2)", &e)
                .unwrap()
                .as_f64(),
            Some(2.5f64.hypot(0.2))
        );
        assert_eq!(eval("SQRT(0)", &e).unwrap().as_f64(), Some(0.0));
        // Negative argument errors (callers fall back to cached V).
        assert!(eval("SQRT(Width-3)", &e).is_err());
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

    // ---- #122/FS-033 breadth slice ----

    #[test]
    fn math_functions_and_corpus_specimens() {
        let e = env();
        // The chord master's Angle (utilisation master3): y first.
        let mut e2 = env();
        e2.insert("EndX".into(), 3.5);
        e2.insert("EndY".into(), 6.2);
        e2.insert("BeginX".into(), 1.0);
        e2.insert("BeginY".into(), 6.0);
        assert_eq!(
            eval("ATAN2(EndY-BeginY,EndX-BeginX)", &e2).unwrap().as_f64(),
            Some((6.2f64 - 6.0).atan2(3.5 - 1.0))
        );
        // ATAN2(0,0) is 0, not a domain error.
        assert_eq!(eval("ATAN2(0,0)", &e).unwrap().as_f64(), Some(0.0));
        // ABS(User.Inset) (master16) composes with the control flow.
        let mut e3 = env();
        e3.insert("User.Inset".into(), -0.25);
        assert_eq!(
            eval("ABS(User.Inset)", &e3).unwrap().as_f64(),
            Some(0.25)
        );
        // CEILING(TEXTHEIGHT(TheText,TxtWidth),margin) (master36).
        let m = FakeText;
        // TEXTHEIGHT = 0.52 → up to the 0.1 grid's 0.6.
        let ceiled = eval_measured("CEILING(TEXTHEIGHT(TheText,0.26),0.1)", &e, &m)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!((ceiled - 0.6).abs() < 1e-9, "ceiling to the 0.1 grid: {ceiled}");
        // The rest of the set at known values.
        assert_eq!(eval("INT(3.9)", &e).unwrap().as_f64(), Some(3.0));
        assert_eq!(eval("INT(-3.1)", &e).unwrap().as_f64(), Some(-4.0));
        assert_eq!(eval("SIGN(-9)", &e).unwrap().as_f64(), Some(-1.0));
        assert_eq!(eval("SIGN(0)", &e).unwrap().as_f64(), Some(0.0));
        assert_eq!(eval("EXP(0)", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("LN(EXP(2))", &e).unwrap().as_f64(), Some(2.0));
        let log1000 = eval("LOG(1000)", &e).unwrap().as_f64().unwrap();
        assert!((log1000 - 3.0).abs() < 1e-9, "LOG(1000) = 3: {log1000}");
        let log8 = eval("LOG(8,2)", &e).unwrap().as_f64().unwrap();
        assert!((log8 - 3.0).abs() < 1e-9, "LOG(8,2) = 3: {log8}");
        assert_eq!(eval("PI()", &e).unwrap().as_f64(), Some(std::f64::consts::PI));
        assert_eq!(
            eval("DEG(PI())", &e).unwrap().as_f64(),
            Some(180.0)
        );
        assert!(
            (eval("RAD(90)", &e).unwrap().as_f64().unwrap() - std::f64::consts::FRAC_PI_2).abs()
                < 1e-12
        );
        assert_eq!(eval("COS(0)", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("SIN(PI()/2)", &e).unwrap().as_f64(), Some(1.0));
        // Arity and domain errors stay errors (cached-V fallback).
        assert!(eval("ABS(1,2)", &e).is_err());
        assert!(eval("LN(0)", &e).is_err());
        assert!(eval("ACOS(2)", &e).is_err());
        assert!(eval("LOG(8,1)", &e).is_err());
        assert!(eval("PI(1)", &e).is_err());
    }

    #[test]
    fn rounding_family_semantics() {
        let e = env();
        // CEILING away from zero on a multiple grid; FLOOR toward zero.
        assert_eq!(eval("CEILING(2.5,1)", &e).unwrap().as_f64(), Some(3.0));
        // The multiple must share the number's sign (else #NUM!).
        assert_eq!(eval("CEILING(-2.5,-1)", &e).unwrap().as_f64(), Some(-3.0));
        assert_eq!(eval("FLOOR(2.5,1)", &e).unwrap().as_f64(), Some(2.0));
        assert_eq!(eval("FLOOR(-2.5,-1)", &e).unwrap().as_f64(), Some(-2.0));
        // Non-1 multiples, sign matching, sign mismatch errors.
        assert_eq!(eval("CEILING(2.1,0.5)", &e).unwrap().as_f64(), Some(2.5));
        assert_eq!(eval("FLOOR(2.9,0.5)", &e).unwrap().as_f64(), Some(2.5));
        assert_eq!(eval("CEILING(2.5)", &e).unwrap().as_f64(), Some(3.0));
        assert_eq!(eval("CEILING(0,1)", &e).unwrap().as_f64(), Some(0.0));
        assert!(eval("CEILING(2, -1)", &e).is_err());
        // ROUND at a decimal precision, negative precision left of the
        // point; ties away from zero.
        assert_eq!(eval("ROUND(2.5)", &e).unwrap().as_f64(), Some(3.0));
        assert_eq!(eval("ROUND(-2.5)", &e).unwrap().as_f64(), Some(-3.0));
        assert_eq!(eval("ROUND(2.345,2)", &e).unwrap().as_f64(), Some(2.35));
        assert_eq!(eval("ROUND(1234,-2)", &e).unwrap().as_f64(), Some(1200.0));
        // Visio's MOD keeps the magnitude (the ADS_M Scratch.A1 rule).
        assert_eq!(eval("MOD(-5,2)", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("MOD(5,2)", &e).unwrap().as_f64(), Some(1.0));
        assert!(eval("MOD(5,0)", &e).is_err());
    }

    #[test]
    fn string_functions() {
        let e = env();
        // The corpus shape: LEN(User.visCustomLabel)=0 (master10) —
        // an empty custom label tests as zero length.
        assert_eq!(eval("LEN(\"\")=0", &e).unwrap().as_f64(), Some(1.0));
        assert_eq!(eval("LEN(\"abc\")", &e).unwrap().as_f64(), Some(3.0));
        assert_eq!(
            eval("LEFT(\"abcdef\",2)", &e).unwrap(),
            Value::Str("ab".into())
        );
        assert_eq!(
            eval("RIGHT(\"abcdef\",2)", &e).unwrap(),
            Value::Str("ef".into())
        );
        // Over-long counts clamp to the text.
        assert_eq!(
            eval("LEFT(\"ab\",9)", &e).unwrap(),
            Value::Str("ab".into())
        );
        // 1-based MID.
        assert_eq!(
            eval("MID(\"abcdef\",2,3)", &e).unwrap(),
            Value::Str("bcd".into())
        );
        assert_eq!(
            eval("MID(\"abc\",9,2)", &e).unwrap(),
            Value::Str("".into())
        );
        // TRIM: single spaces between words, none at the edges.
        assert_eq!(
            eval("TRIM(\"  a   b  \")", &e).unwrap(),
            Value::Str("a b".into())
        );
        assert_eq!(
            eval("UPPER(\"aBc\")&LOWER(\"DeF\")", &e).unwrap(),
            Value::Str("ABCdef".into())
        );
        assert_eq!(
            eval("SUBSTITUTE(\"a-b-c\",\"-\",\"+\")", &e).unwrap(),
            Value::Str("a+b+c".into())
        );
        // FIND: 1-based position, optional 1-based start, case folding.
        assert_eq!(eval("FIND(\"b\",\"abc\")", &e).unwrap().as_f64(), Some(2.0));
        assert_eq!(
            eval("FIND(\"b\",\"abcabc\",3)", &e).unwrap().as_f64(),
            Some(5.0)
        );
        assert_eq!(
            eval("FIND(\"B\",\"abc\",1,1)", &e).unwrap().as_f64(),
            Some(2.0)
        );
        // Not-found and out-of-range starts are errors (cached-V fallback).
        assert!(eval("FIND(\"z\",\"abc\")", &e).is_err());
        assert!(eval("FIND(\"b\",\"abc\",9)", &e).is_err());
        // A needle longer than the haystack is not found — no panic
        // (#218 review).
        assert!(eval("FIND(\"abcd\",\"ab\")", &e).is_err());
        // VALUE parses a number out of text; garbage errors.
        assert_eq!(eval("VALUE(\"2.5\")", &e).unwrap().as_f64(), Some(2.5));
        assert!(eval("VALUE(\"abc\")", &e).is_err());
        assert!(eval("LEN(\"a\",\"b\")", &e).is_err());
    }

    #[test]
    fn color_functions_match_the_measured_grid() {
        let e = env();
        // The legacy 240-step integer grid witness: HSL(144,116,125)
        // = #4979C0 (shipped-stencil measurement).
        assert_eq!(eval("HSL(144,116,125)", &e).unwrap().as_f64(), Some(0x4979C0 as f64));
        // Full-saturation blue at luminosity 120: HSL(160,240,120) =
        // RGB(0,0,255).
        assert_eq!(
            eval("HSL(160,240,120)", &e).unwrap().as_f64(),
            eval("RGB(0,0,255)", &e).unwrap().as_f64()
        );
        assert_eq!(eval("RGB(0,0,255)", &e).unwrap().as_f64(), Some(255.0));
        // Component extraction is self-consistent.
        assert_eq!(eval("RED(RGB(10,20,30))", &e).unwrap().as_f64(), Some(10.0));
        assert_eq!(eval("GREEN(RGB(10,20,30))", &e).unwrap().as_f64(), Some(20.0));
        assert_eq!(eval("BLUE(RGB(10,20,30))", &e).unwrap().as_f64(), Some(30.0));
        // The fixed [MS-VSDX] palette: index 0 black, 1 white.
        assert_eq!(eval("RED(0)", &e).unwrap().as_f64(), Some(0.0));
        assert_eq!(eval("LUM(1)", &e).unwrap().as_f64(), Some(240.0));
        assert_eq!(eval("RED(1)", &e).unwrap().as_f64(), Some(255.0));
        // HUE/SAT/LUM on the 240 scale (green sits at hue 80).
        assert_eq!(eval("HUE(RGB(0,255,0))", &e).unwrap().as_f64(), Some(80.0));
        assert_eq!(eval("SAT(RGB(0,255,0))", &e).unwrap().as_f64(), Some(240.0));
        assert_eq!(eval("LUM(RGB(0,255,0))", &e).unwrap().as_f64(), Some(120.0));
        // Round-trip through the integer grid.
        assert_eq!(
            eval("LUM(HSL(0,0,120))", &e).unwrap().as_f64(),
            Some(120.0)
        );
        // The measured TINT endpoint rule: TINT(RGB(225,225,225),28) =
        // RGB(233,233,233).
        assert_eq!(
            eval("TINT(RGB(225,225,225),28)", &e).unwrap().as_f64(),
            eval("RGB(233,233,233)", &e).unwrap().as_f64()
        );
        // The parity witnesses (LUM of the adjusted color).
        assert_eq!(
            eval("LUM(TINT(HSL(0,0,99),-1.5))", &e).unwrap().as_f64(),
            Some(99.0)
        );
        assert_eq!(
            eval("LUM(TINT(HSL(0,0,100),1.5))", &e).unwrap().as_f64(),
            Some(101.0)
        );
        assert_eq!(
            eval("LUM(TINT(HSL(0,0,211),100))", &e).unwrap().as_f64(),
            Some(240.0)
        );
        assert_eq!(
            eval("LUM(SHADE(HSL(0,0,99),1.5))", &e).unwrap().as_f64(),
            Some(99.0)
        );
        assert_eq!(
            eval("LUM(SHADE(HSL(0,0,141),-1.5))", &e).unwrap().as_f64(),
            Some(141.0)
        );
        // SHADE moves toward black; the corpus's SHADE(BaseColor,40)
        // darkens.
        let shaded = eval("LUM(SHADE(RGB(128,128,128),40))", &e)
            .unwrap()
            .as_f64()
            .unwrap();
        assert!(shaded < 128.0, "SHADE darkens: {shaded}");
        // Out-of-range and invalid inputs error (cached-V fallback).
        assert!(eval("HSL(241,0,0)", &e).is_err());
        assert!(eval("HSL(0,-1,0)", &e).is_err());
        assert!(eval("TINT(-3,10)", &e).is_err());
        // An invalid color extracts as 0 (the HUE/SAT/LUM rule).
        assert_eq!(eval("HUE(-3)", &e).unwrap().as_f64(), Some(0.0));
    }

    #[test]
    fn guide_algebra_intersections() {
        let e = env();
        // A vertical guide at x=2 crossing a horizontal guide at y=3
        // (the guide-alignment cell pattern).
        let f = "INTERSECTX(2,0,2,1, 0,3,1,3)";
        assert_eq!(eval(f, &e).unwrap().as_f64(), Some(2.0));
        let fy = "INTERSECTY(2,0,2,1, 0,3,1,3)";
        assert_eq!(eval(fy, &e).unwrap().as_f64(), Some(3.0));
        // Off-axis crossing: y = x line vs y = 1 line → (1, 1).
        assert_eq!(
            eval("INTERSECTX(0,0,2,2, 0,1,2,1)", &e).unwrap().as_f64(),
            Some(1.0)
        );
        assert_eq!(
            eval("INTERSECTY(0,0,2,2, 0,1,2,1)", &e).unwrap().as_f64(),
            Some(1.0)
        );
        // Parallel lines error (cached-V fallback).
        assert!(eval("INTERSECTX(0,0,1,0, 2,0,3,0)", &e).is_err());
        assert!(eval("INTERSECTX(0,0,1,0, 0,1,1,1)", &e).is_err());
        assert!(eval("INTERSECTY(0,0,0,1, 1,0,1,1)", &e).is_err());
    }

    #[test]
    fn themeguard_identity_and_isthemed_false() {
        let e = env();
        // THEMEGUARD guards the cell from theme overwrites; this
        // evaluator applies no themes, so the identity IS the
        // guarded evaluation.
        assert_eq!(
            eval("THEMEGUARD(Width*0.5)", &e).unwrap().as_f64(),
            Some(1.0)
        );
        // No theme engine: the measured no-theme branch is false.
        assert_eq!(eval("ISTHEMED()", &e).unwrap().as_f64(), Some(0.0));
        assert!(eval("ISTHEMED(1)", &e).is_err());
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
