//! The small typed language of the triggers (docs/trigger-und-28yl.md step 3), like TAB's ZXCode: **expressions**
//! (`Gold >= 200 & Day > 3`) for conditions and **code** (`keys += 1; GiveGold(100)`) for what an action does after it.
//!
//! It's typed as text in the editor and checked at once (`parse_condition`, `parse_code`: a wrong type or an unknown
//! name says where), and kept as a fixed tree (`Expr`, `Stmt`) – the simulation never reads text. Three types: number
//! (whole numbers only: the simulation is deterministic), yes/no and text. The functions and built-in values come from
//! one list (`funcs.rs`), so the language grows without touching the parser or the interpreter.
//!
//! Binding is the usual one, from loosest to tightest: `|`, `&`, comparisons (`= != < <= > >=`), `+ -`, `* /`, then
//! `!` and `-` in front. TAB's own binding is odd (`a - b - c` = `a - (b - c)`): the converter (step 8) sets the
//! brackets there, this language binds normally.

use std::fmt::{self, Write as _};

use serde::{Deserialize, Serialize};

use super::funcs;
use super::story::{VarKind, Variable};

/// What a value is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Ty {
    Number,
    /// Yes or no.
    Flag,
    Text,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Number => "number",
            Ty::Flag => "yes/no",
            Ty::Text => "text",
        }
    }
}

/// A value – in the file as a plain JSON number, `true`/`false` or string.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Flag(bool),
    Number(i32),
    Text(String),
}

impl Default for Value {
    fn default() -> Value {
        Value::Number(0)
    }
}

impl Value {
    pub fn ty(&self) -> Ty {
        match self {
            Value::Flag(_) => Ty::Flag,
            Value::Number(_) => Ty::Number,
            Value::Text(_) => Ty::Text,
        }
    }

    /// As a number: yes is 1, no is 0, a text is its length – the types are checked when the code is typed, this is
    /// only for a variable whose kind was changed afterwards.
    pub fn number(&self) -> i32 {
        match self {
            Value::Flag(f) => i32::from(*f),
            Value::Number(n) => *n,
            Value::Text(t) => t.len() as i32,
        }
    }

    pub fn flag(&self) -> bool {
        match self {
            Value::Flag(f) => *f,
            Value::Number(n) => *n != 0,
            Value::Text(t) => !t.is_empty(),
        }
    }

    pub fn text(&self) -> String {
        match self {
            Value::Flag(f) => if *f { "yes" } else { "no" }.to_owned(),
            Value::Number(n) => n.to_string(),
            Value::Text(t) => t.clone(),
        }
    }

    /// This value as a value of type `ty`.
    pub fn as_ty(&self, ty: Ty) -> Value {
        match ty {
            Ty::Number => Value::Number(self.number()),
            Ty::Flag => Value::Flag(self.flag()),
            Ty::Text => Value::Text(self.text()),
        }
    }
}

/// The operators between two values.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    fn symbol(self) -> &'static str {
        match self {
            Op::Or => "|",
            Op::And => "&",
            Op::Eq => "=",
            Op::Ne => "!=",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
        }
    }

    /// How tightly it binds: the higher, the tighter.
    fn level(self) -> usize {
        match self {
            Op::Or => 0,
            Op::And => 1,
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => 2,
            Op::Add | Op::Sub => 3,
            Op::Mul | Op::Div => 4,
        }
    }
}

/// An expression as a tree.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Expr {
    Lit(Value),
    /// A variable of the map (`Story::variables`).
    Var(String),
    /// A function or built-in value of `funcs.rs` – by name; no arguments for a value like `Day`.
    Call { name: String, args: Vec<Expr> },
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
}

impl Expr {
    pub fn yes() -> Expr {
        Expr::Lit(Value::Flag(true))
    }

    pub fn number(n: i32) -> Expr {
        Expr::Lit(Value::Number(n))
    }

    pub fn text(t: &str) -> Expr {
        Expr::Lit(Value::Text(t.to_owned()))
    }

    pub fn var(name: &str) -> Expr {
        Expr::Var(name.to_owned())
    }

    pub fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Call { name: name.to_owned(), args }
    }

    pub fn bin(op: Op, left: Expr, right: Expr) -> Expr {
        Expr::Bin(op, Box::new(left), Box::new(right))
    }

    /// The names of all variables in it.
    pub fn variables(&self, names: &mut Vec<String>) {
        match self {
            Expr::Var(name) => names.push(name.clone()),
            Expr::Lit(_) => {}
            Expr::Call { args, .. } => args.iter().for_each(|a| a.variables(names)),
            Expr::Not(e) | Expr::Neg(e) => e.variables(names),
            Expr::Bin(_, l, r) => {
                l.variables(names);
                r.variables(names);
            }
        }
    }

    /// How tightly its outermost part binds (for the brackets when it's written out).
    fn level(&self) -> usize {
        match self {
            Expr::Bin(op, ..) => op.level(),
            Expr::Not(_) | Expr::Neg(_) => 5,
            _ => 6,
        }
    }

    /// Writes it as a child of something that binds with `level`; `strict`: brackets for the same level too (the right
    /// side of an operator – the language binds to the left).
    fn child(&self, out: &mut String, level: usize, strict: bool) {
        if self.level() < level || (strict && self.level() == level && matches!(self, Expr::Bin(..))) {
            out.push('(');
            self.write(out);
            out.push(')');
        } else {
            self.write(out);
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            Expr::Lit(Value::Flag(f)) => out.push_str(if *f { "true" } else { "false" }),
            Expr::Lit(Value::Number(n)) => {
                let _ = write!(out, "{n}");
            }
            Expr::Lit(Value::Text(t)) => {
                out.push('"');
                for c in t.chars() {
                    if c == '"' || c == '\\' {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('"');
            }
            Expr::Var(name) => out.push_str(name),
            Expr::Call { name, args } => {
                out.push_str(name);
                // A built-in value (`Day`) is written without brackets.
                let value = funcs::find(name, 0).is_some_and(|f| f.returns.is_some());
                if !args.is_empty() || !value {
                    out.push('(');
                    for (k, a) in args.iter().enumerate() {
                        if k > 0 {
                            out.push_str(", ");
                        }
                        a.write(out);
                    }
                    out.push(')');
                }
            }
            Expr::Not(e) => {
                out.push('!');
                e.child(out, 5, false);
            }
            Expr::Neg(e) => {
                out.push('-');
                e.child(out, 5, false);
            }
            Expr::Bin(op, l, r) => {
                l.child(out, op.level(), false);
                let _ = write!(out, " {} ", op.symbol());
                r.child(out, op.level(), true);
            }
        }
    }
}

/// An expression as text – what `parse_expr` reads back into the same tree.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.write(&mut out);
        f.write_str(&out)
    }
}

/// How an assignment changes a variable: `=`, `+=`, `-=`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Assign {
    Set,
    Add,
    Sub,
}

/// One statement of code.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Stmt {
    Set { name: String, how: Assign, value: Expr },
    /// A function that does something (`GiveGold(100)`) – anything it gives back is dropped.
    Do(Expr),
}

/// Code: statements one after the other.
pub type Code = Vec<Stmt>;

/// Code as text – what `parse_code` reads back into the same tree.
pub fn code_text(code: &[Stmt]) -> String {
    let parts: Vec<String> = code
        .iter()
        .map(|s| match s {
            Stmt::Set { name, how, value } => {
                let op = match how {
                    Assign::Set => "=",
                    Assign::Add => "+=",
                    Assign::Sub => "-=",
                };
                format!("{name} {op} {value}")
            }
            Stmt::Do(e) => e.to_string(),
        })
        .collect();
    parts.join("; ")
}

/// Where in the text (counted in characters from 0) and what is wrong.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExprError {
    pub at: usize,
    pub msg: String,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}: {}", self.at + 1, self.msg)
    }
}

fn error<T>(at: usize, msg: impl Into<String>) -> Result<T, ExprError> {
    Err(ExprError { at, msg: msg.into() })
}

/// The variables the text may use.
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub variables: &'a [Variable],
}

impl Scope<'_> {
    fn ty_of(&self, name: &str) -> Option<Ty> {
        self.variables.iter().find(|v| v.name == name).map(|v| v.kind.ty())
    }
}

impl VarKind {
    pub fn ty(self) -> Ty {
        match self {
            VarKind::Number => Ty::Number,
            VarKind::Flag => Ty::Flag,
            VarKind::Text => Ty::Text,
        }
    }
}

/// An expression of type `want`. Empty text is an error.
pub fn parse_expr(text: &str, scope: Scope, want: Ty) -> Result<Expr, ExprError> {
    let mut p = Parser::new(text, scope)?;
    let t = p.expr()?;
    p.finish()?;
    let ty = t.ty.ok_or(ExprError { at: t.at, msg: "that gives no value".to_owned() })?;
    if ty != want {
        return error(t.at, format!("expected {}, found {}", want.name(), ty.name()));
    }
    Ok(t.expr)
}

/// An expression of any type – what a `{...}` in a text holds. Empty text is an error.
pub fn parse_value(text: &str, scope: Scope) -> Result<Expr, ExprError> {
    let mut p = Parser::new(text, scope)?;
    let t = p.expr()?;
    p.finish()?;
    t.ty.ok_or(ExprError { at: t.at, msg: "that gives no value".to_owned() })?;
    Ok(t.expr)
}

/// A text cut at its `{...}`: pieces that are text as it is (`false`) and pieces that are an expression's source
/// (`true`, without the braces). A `{` without a closing `}` stays text. TAB's `FinderGetText("{", "}")`: from a
/// `{` to the first `}` after it.
pub fn template_parts(text: &str) -> Vec<(bool, &str)> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        if open > 0 {
            parts.push((false, &rest[..open]));
        }
        parts.push((true, &rest[open + 1..open + close]));
        rest = &rest[open + close + 1..];
    }
    if !rest.is_empty() {
        parts.push((false, rest));
    }
    parts
}

/// A condition: an expression of yes/no.
pub fn parse_condition(text: &str, scope: Scope) -> Result<Expr, ExprError> {
    parse_expr(text, scope, Ty::Flag)
}

/// Code: statements separated by `;` – nothing at all is fine.
pub fn parse_code(text: &str, scope: Scope) -> Result<Code, ExprError> {
    let mut p = Parser::new(text, scope)?;
    let mut code = Code::new();
    while !p.done() {
        if p.eat_op(";") {
            continue;
        }
        code.push(p.statement()?);
        if !p.done() && !p.eat_op(";") {
            let (tok, at) = p.peek().expect("not done");
            return error(at, format!("expected ; or the end, found {tok}"));
        }
    }
    Ok(code)
}

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Num(i32),
    Id(String),
    Str(String),
    Op(&'static str),
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Num(n) => write!(f, "{n}"),
            Tok::Id(s) => f.write_str(s),
            Tok::Str(s) => write!(f, "\"{s}\""),
            Tok::Op(o) => f.write_str(o),
        }
    }
}

/// What the parser has for a part of the text: its tree, its type (`None`: a function that gives nothing back) and
/// where it starts.
struct Typed {
    expr: Expr,
    ty: Option<Ty>,
    at: usize,
}

struct Parser<'a> {
    tokens: Vec<(Tok, usize)>,
    next: usize,
    /// Where the text ends (for "unexpected end").
    end: usize,
    scope: Scope<'a>,
}

const OPS: [&str; 18] = ["!=", "<=", ">=", "+=", "-=", "==", "|", "&", "=", "<", ">", "+", "-", "*", "/", "!", "(", ")"];

fn tokens(text: &str) -> Result<Vec<(Tok, usize)>, ExprError> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digits: String = chars[start..i].iter().collect();
            match digits.parse() {
                Ok(n) => out.push((Tok::Num(n), start)),
                Err(_) => return error(start, "the number is too big"),
            }
        } else if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push((Tok::Id(chars[start..i].iter().collect()), start));
        } else if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return error(start, "the text has no closing \""),
                    Some('"') => break,
                    Some('\\') if i + 1 < chars.len() => {
                        s.push(chars[i + 1]);
                        i += 2;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }
            i += 1;
            out.push((Tok::Str(s), start));
        } else if c == ',' || c == ';' {
            i += 1;
            out.push((Tok::Op(if c == ',' { "," } else { ";" }), start));
        } else {
            let rest: String = chars[i..chars.len().min(i + 2)].iter().collect();
            match OPS.iter().find(|op| rest.starts_with(**op)) {
                Some(op) => {
                    i += op.len();
                    out.push((Tok::Op(op), start));
                }
                None => return error(start, format!("unexpected {c:?}")),
            }
        }
    }
    Ok(out)
}

impl<'a> Parser<'a> {
    fn new(text: &str, scope: Scope<'a>) -> Result<Parser<'a>, ExprError> {
        Ok(Parser { tokens: tokens(text)?, next: 0, end: text.chars().count(), scope })
    }

    fn done(&self) -> bool {
        self.next >= self.tokens.len()
    }

    fn peek(&self) -> Option<(Tok, usize)> {
        self.tokens.get(self.next).cloned()
    }

    fn at(&self) -> usize {
        self.tokens.get(self.next).map_or(self.end, |t| t.1)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if matches!(self.tokens.get(self.next), Some((Tok::Op(o), _)) if *o == op) {
            self.next += 1;
            true
        } else {
            false
        }
    }

    /// Nothing may be left after an expression.
    fn finish(&self) -> Result<(), ExprError> {
        match self.peek() {
            None => Ok(()),
            Some((tok, at)) => error(at, format!("unexpected {tok}")),
        }
    }

    fn expr(&mut self) -> Result<Typed, ExprError> {
        self.binary(0)
    }

    /// The operators of one level, to the left (the next level binds tighter).
    fn binary(&mut self, level: usize) -> Result<Typed, ExprError> {
        if level > 4 {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        while let Some((Tok::Op(symbol), at)) = self.peek() {
            let op = match symbol {
                "|" => Op::Or,
                "&" => Op::And,
                "=" | "==" => Op::Eq,
                "!=" => Op::Ne,
                "<" => Op::Lt,
                "<=" => Op::Le,
                ">" => Op::Gt,
                ">=" => Op::Ge,
                "+" => Op::Add,
                "-" => Op::Sub,
                "*" => Op::Mul,
                "/" => Op::Div,
                _ => break,
            };
            if op.level() != level {
                break;
            }
            self.next += 1;
            let right = self.binary(level + 1)?;
            left = self.join(op, at, left, right)?;
        }
        Ok(left)
    }

    /// `left op right` with the types checked.
    fn join(&self, op: Op, op_at: usize, left: Typed, right: Typed) -> Result<Typed, ExprError> {
        let lt = left.ty.ok_or(ExprError { at: left.at, msg: "that gives no value".to_owned() })?;
        let rt = right.ty.ok_or(ExprError { at: right.at, msg: "that gives no value".to_owned() })?;
        let need = |t: Ty, side: &Typed, found: Ty| if found == t { Ok(()) } else { error(side.at, format!("expected {}, found {}", t.name(), found.name())) };
        let ty = match op {
            Op::Or | Op::And => {
                need(Ty::Flag, &left, lt)?;
                need(Ty::Flag, &right, rt)?;
                Ty::Flag
            }
            Op::Eq | Op::Ne => {
                if lt != rt {
                    return error(op_at, format!("can't compare {} with {}", lt.name(), rt.name()));
                }
                Ty::Flag
            }
            Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                need(Ty::Number, &left, lt)?;
                need(Ty::Number, &right, rt)?;
                Ty::Flag
            }
            Op::Add if lt == Ty::Text => {
                need(Ty::Text, &right, rt)?;
                Ty::Text
            }
            Op::Add | Op::Sub | Op::Mul | Op::Div => {
                need(Ty::Number, &left, lt)?;
                need(Ty::Number, &right, rt)?;
                Ty::Number
            }
        };
        Ok(Typed { at: left.at, expr: Expr::bin(op, left.expr, right.expr), ty: Some(ty) })
    }

    fn unary(&mut self) -> Result<Typed, ExprError> {
        let at = self.at();
        let (want, make): (Ty, fn(Box<Expr>) -> Expr) = if self.eat_op("!") {
            (Ty::Flag, Expr::Not)
        } else if self.eat_op("-") {
            (Ty::Number, Expr::Neg)
        } else {
            return self.primary();
        };
        let inner = self.unary()?;
        match inner.ty {
            // A minus in front of a number is part of the number: `-3` reads back as `-3`.
            Some(Ty::Number) if want == Ty::Number && matches!(inner.expr, Expr::Lit(Value::Number(n)) if n != i32::MIN) => {
                let Expr::Lit(Value::Number(n)) = inner.expr else { unreachable!("just matched") };
                Ok(Typed { expr: Expr::number(-n), ty: Some(Ty::Number), at })
            }
            Some(t) if t == want => Ok(Typed { expr: make(Box::new(inner.expr)), ty: Some(want), at }),
            Some(t) => error(inner.at, format!("expected {}, found {}", want.name(), t.name())),
            None => error(inner.at, "that gives no value"),
        }
    }

    fn primary(&mut self) -> Result<Typed, ExprError> {
        let at = self.at();
        let Some((tok, _)) = self.peek() else {
            return error(at, "the end came too early - an expression is missing");
        };
        self.next += 1;
        let lit = |v: Value, ty: Ty| Ok(Typed { expr: Expr::Lit(v), ty: Some(ty), at });
        match tok {
            Tok::Num(n) => lit(Value::Number(n), Ty::Number),
            Tok::Str(s) => lit(Value::Text(s), Ty::Text),
            Tok::Op("(") => {
                let inner = self.expr()?;
                if !self.eat_op(")") {
                    return error(self.at(), "expected )");
                }
                Ok(Typed { at, ..inner })
            }
            Tok::Id(name) => {
                let lower = name.to_lowercase();
                if lower == "true" || lower == "false" {
                    return lit(Value::Flag(lower == "true"), Ty::Flag);
                }
                if self.eat_op("(") {
                    return self.call(name, at);
                }
                if let Some(f) = funcs::find(&name, 0) {
                    if f.returns.is_none() {
                        return error(at, format!("{} needs brackets: {}()", f.name, f.name));
                    }
                    return Ok(Typed { expr: Expr::call(f.name, Vec::new()), ty: f.returns, at });
                }
                match self.scope.ty_of(&name) {
                    Some(ty) => Ok(Typed { expr: Expr::Var(name), ty: Some(ty), at }),
                    None if funcs::named(&name).is_some() => error(at, format!("{name} needs values in brackets: {}", signature_of(&name))),
                    None => error(at, format!("{name} is not known - a variable is made under Variables")),
                }
            }
            other => error(at, format!("unexpected {other}")),
        }
    }

    /// The arguments of a call – the bracket is already read.
    fn call(&mut self, name: String, at: usize) -> Result<Typed, ExprError> {
        let mut args = Vec::new();
        if !self.eat_op(")") {
            loop {
                args.push(self.expr()?);
                if self.eat_op(")") {
                    break;
                }
                if !self.eat_op(",") {
                    return error(self.at(), "expected , or )");
                }
            }
        }
        let Some(f) = funcs::find(&name, args.len()) else {
            return match funcs::named(&name) {
                Some(_) => error(at, format!("wrong number of values: {}", signature_of(&name))),
                None => error(at, format!("{name} is not a function")),
            };
        };
        for (arg, &param) in args.iter().zip(f.params) {
            match arg.ty {
                Some(t) if t == param => {}
                Some(t) => return error(arg.at, format!("{} wants {}, found {}", f.name, param.name(), t.name())),
                None => return error(arg.at, "that gives no value"),
            }
        }
        let args = args.into_iter().map(|a| a.expr).collect();
        Ok(Typed { expr: Expr::Call { name: f.name.to_owned(), args }, ty: f.returns, at })
    }

    fn statement(&mut self) -> Result<Stmt, ExprError> {
        let at = self.at();
        if let (Some((Tok::Id(name), _)), Some((Tok::Op(op), _))) = (self.tokens.get(self.next).cloned(), self.tokens.get(self.next + 1).cloned())
            && matches!(op, "=" | "+=" | "-=")
        {
            self.next += 2;
            let how = match op {
                "=" => Assign::Set,
                "+=" => Assign::Add,
                _ => Assign::Sub,
            };
            if funcs::named(&name).is_some() {
                return error(at, format!("{name} is built in and can't be changed"));
            }
            let Some(var) = self.scope.ty_of(&name) else {
                return error(at, format!("{name} is not known - a variable is made under Variables"));
            };
            let value = self.expr()?;
            let found = value.ty.ok_or(ExprError { at: value.at, msg: "that gives no value".to_owned() })?;
            let fits = match how {
                Assign::Set => found == var,
                Assign::Add => found == var && var != Ty::Flag,
                Assign::Sub => found == var && var == Ty::Number,
            };
            if !fits {
                let msg = match how {
                    Assign::Set => format!("{name} holds {}, not {}", var.name(), found.name()),
                    Assign::Add => format!("+= works with numbers and texts of the same kind as {name} ({}), found {}", var.name(), found.name()),
                    Assign::Sub => format!("-= works with numbers; {name} is {}, found {}", var.name(), found.name()),
                };
                return error(value.at, msg);
            }
            return Ok(Stmt::Set { name, how, value: value.expr });
        }
        let t = self.expr()?;
        match t.expr {
            Expr::Call { .. } => Ok(Stmt::Do(t.expr)),
            _ => error(t.at, "a statement is an assignment (x = 1, x += 1) or a function (GiveGold(100))"),
        }
    }
}

/// `Count(text)` – how a function is called, for the messages.
fn signature_of(name: &str) -> String {
    match funcs::named(name) {
        Some(f) => funcs::signature(f),
        None => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vec<Variable> {
        let v = |name: &str, kind| Variable { name: name.into(), kind, start: Value::default() };
        vec![v("keys", VarKind::Number), v("met", VarKind::Flag), v("who", VarKind::Text)]
    }

    fn cond(text: &str) -> Result<Expr, ExprError> {
        parse_condition(text, Scope { variables: &vars() })
    }

    fn code(text: &str) -> Result<Code, ExprError> {
        parse_code(text, Scope { variables: &vars() })
    }

    /// What's typed is written back the same (spaces and brackets as the language writes them) and reads again as the
    /// same tree – the editor shows the tree as text.
    #[test]
    fn expressions_come_back_as_typed() {
        for text in [
            "Gold >= 200 & Day > 3",
            "keys + 1 * 2 = 3",
            "keys - (2 - 1) = 0",
            "keys - 2 - 1 = 0",
            "!met | IsDead(\"boss\")",
            "Count(\"Tent\") >= 10",
            "who + \"!\" = \"hi!\"",
            "-keys < -3",
            "D100 <= Random(5, 9)",
            "!(met & keys = 1)",
        ] {
            let e = cond(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(e.to_string(), text);
            assert_eq!(cond(&e.to_string()).unwrap(), e);
        }
        let c = code("keys += 1; who = \"a \\\"b\\\"\"; GiveGold(-5); met = true;").unwrap();
        assert_eq!(code_text(&c), "keys += 1; who = \"a \\\"b\\\"\"; GiveGold(-5); met = true");
        assert_eq!(code(&code_text(&c)).unwrap(), c);
        assert_eq!(code("  ").unwrap(), Vec::new());
    }

    /// The usual binding: `*` before `+` before comparisons before `&` before `|`, to the left.
    #[test]
    fn operators_bind_the_usual_way() {
        assert_eq!(
            cond("met | met & keys + 2 * 3 > 1").unwrap(),
            Expr::bin(
                Op::Or,
                Expr::var("met"),
                Expr::bin(
                    Op::And,
                    Expr::var("met"),
                    Expr::bin(Op::Gt, Expr::bin(Op::Add, Expr::var("keys"), Expr::bin(Op::Mul, Expr::number(2), Expr::number(3))), Expr::number(1))
                )
            )
        );
        // To the left: 10 - 3 - 2 is (10 - 3) - 2, not TAB's 10 - (3 - 2).
        assert_eq!(cond("10 - 3 - 2 = 5").unwrap(), Expr::bin(Op::Eq, Expr::bin(Op::Sub, Expr::bin(Op::Sub, Expr::number(10), Expr::number(3)), Expr::number(2)), Expr::number(5)));
    }

    /// Mistakes are found as they are typed, with their place (counted in characters from 1 in the message).
    #[test]
    fn mistakes_say_where() {
        let at = |r: Result<Expr, ExprError>| r.unwrap_err().at;
        // The place is where the wrong part starts.
        assert_eq!(at(cond("keys > 1 & 5")), 11);
        assert_eq!(at(cond("keys > true")), 7);
        assert_eq!(at(cond("keys = who")), 5);
        assert_eq!(at(cond("nobody > 1")), 0);
        assert_eq!(at(cond("Count(1) > 1")), 6);
        assert_eq!(at(cond("Count() > 1")), 0);
        assert_eq!(at(cond("keys +")), 6);
        assert_eq!(at(cond("(keys > 1")), 9);
        assert_eq!(at(cond("keys")), 0, "a number isn't a condition");
        assert_eq!(at(cond("GiveGold(1)")), 0, "a function that gives nothing back isn't a value");
        assert_eq!(at(cond("\"open")), 0);
        assert_eq!(at(cond("keys > 99999999999")), 7);
        assert_eq!(at(cond("")), 0);
        // Code: the kinds must fit, built-in values can't be set, a statement is an assignment or a function.
        let bad = |text: &str| code(text).unwrap_err();
        assert_eq!(bad("keys = true").at, 7);
        assert_eq!(bad("met += 1").at, 7);
        assert_eq!(bad("who -= \"a\"").at, 7);
        assert_eq!(bad("Gold = 5").at, 0);
        assert_eq!(bad("nobody = 5").at, 0);
        assert_eq!(bad("keys + 1").at, 0);
        assert_eq!(bad("keys = 1 keys = 2").at, 9);
        assert_eq!(bad("EndGameWon").at, 0, "a function that gives nothing needs its brackets");
        assert!(bad("keys = 1 +").msg.contains("expression is missing"), "{}", bad("keys = 1 +").msg);
    }

    /// Names of functions and built-in values don't care about the case (TAB's symbols don't).
    #[test]
    fn names_have_any_case() {
        let e = cond("gold >= 1 & DAY < Random(3)").unwrap();
        assert_eq!(e.to_string(), "Gold >= 1 & Day < Random(3)");
    }
}
