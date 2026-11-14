//! The list of everything the trigger language can call (`expr.rs`) – and the interpreter that runs a tree.
//!
//! One entry per function or built-in value: its name, the types of its parameters, what it gives back (nothing for a
//! function that only does something), a sentence for the editor, and what it does (`run`). To teach the language
//! something new, add a line to `FUNCS` – the parser, the type check, the printer and the editor's help all read this
//! list. TAB's own built-in values and functions are the first entries (`ZXCodeVariablesAndFunctions.cs`); the entity
//! types and IDs TAB writes as symbols are texts here.
//!
//! Everything is deterministic: only whole numbers, the world's dice (`Random`, `D100`) and the order of the lists.

use super::expr::{Assign, Code, Expr, Op, Stmt, Ty, Value};
use super::story::Ending;
use super::{World, DAY_TICKS, HOUR_TICKS};
use crate::kinds::Resource;

/// One function (or value without parameters like `Day`).
pub struct Func {
    pub name: &'static str,
    pub params: &'static [Ty],
    /// `None`: it only does something.
    pub returns: Option<Ty>,
    /// A sentence for the editor.
    pub help: &'static str,
    pub run: fn(&mut World, &[Value]) -> Value,
}

macro_rules! func {
    ($name:expr, [$($p:expr),*], $ret:expr, $help:expr, $run:expr) => {
        Func { name: $name, params: &[$($p),*], returns: $ret, help: $help, run: $run }
    };
}

fn number(n: i32) -> Value {
    Value::Number(n)
}

/// A count as a number – 2 billion is as far as it goes.
fn count(n: usize) -> Value {
    Value::Number(n.min(i32::MAX as usize) as i32)
}

fn nothing() -> Value {
    Value::default()
}

/// Every function and built-in value, in the order the editor lists them.
pub static FUNCS: &[Func] = &[
    func!("Day", [], Some(Ty::Number), "The day of the round (the first is 1).", |w, _| number((w.tick / DAY_TICKS + 1) as i32)),
    func!("Hour", [], Some(Ty::Number), "The hour of the day, 0 to 23.", |w, _| number(((w.tick % DAY_TICKS) / HOUR_TICKS) as i32)),
    func!("TotalHours", [], Some(Ty::Number), "Day * 24 + Hour.", |w, _| number(((w.tick / DAY_TICKS + 1) * 24 + (w.tick % DAY_TICKS) / HOUR_TICKS) as i32)),
    func!("D100", [], Some(Ty::Number), "A random number from 1 to 100.", |w, _| number(w.rng.below(100) as i32 + 1)),
    func!("Random", [Ty::Number], Some(Ty::Number), "A random number from 1 to max.", |w, a| number(roll(w, 1, a[0].number()))),
    func!("Random", [Ty::Number, Ty::Number], Some(Ty::Number), "A random number from min to max.", |w, a| number(roll(w, a[0].number(), a[1].number()))),
    func!("Gold", [], Some(Ty::Number), "The colony's gold.", |w, _| count(w.gold as usize)),
    func!("Wood", [], Some(Ty::Number), "The colony's wood.", |w, _| stock(w, Resource::Wood)),
    func!("Stone", [], Some(Ty::Number), "The colony's stone.", |w, _| stock(w, Resource::Stone)),
    func!("IronOre", [], Some(Ty::Number), "The colony's iron ore.", |w, _| stock(w, Resource::IronOre)),
    func!("Coal", [], Some(Ty::Number), "The colony's coal.", |w, _| stock(w, Resource::Coal)),
    func!("FoodStock", [], Some(Ty::Number), "The food in the stores (not TAB's Food, which is the production).", |w, _| stock(w, Resource::Food)),
    func!("Iron", [], Some(Ty::Number), "The colony's iron.", |w, _| stock(w, Resource::Iron)),
    func!("Oil", [], Some(Ty::Number), "The colony's oil.", |w, _| stock(w, Resource::Oil)),
    func!("Workers", [], Some(Ty::Number), "All the workers the colony's houses give.", |w, _| count(w.workers().1 as usize)),
    func!("WorkersAvailable", [], Some(Ty::Number), "The workers nobody uses.", |w, _| count(w.free_workers() as usize)),
    func!("Colonists", [], Some(Ty::Number), "The colonists in the houses that aren't infected.", |w, _| count(w.colonists() as usize)),
    func!("Kills", [], Some(Ty::Number), "The zombies killed so far.", |w, _| count(w.score.zombies_killed as usize)),
    func!("Count", [Ty::Text], Some(Ty::Number), "How many finished buildings and units have this name (\"Soldier\", \"Tent\") - or are \"Unit\", \"Building\", \"Dwelling\", \"Infected\".", |w, a| count(w.count_kind(&a[0].text()))),
    func!("IsDead", [Ty::Text], Some(Ty::Flag), "Yes if no building or unit with this name ID is left.", |w, a| Value::Flag(!w.has_name_id(&a[0].text()))),
    func!("EventDone", [Ty::Text], Some(Ty::Flag), "Yes if the event with this name has come every time it will.", |w, a| {
        let name = a[0].text();
        Value::Flag(w.events.iter().any(|e| e.name == name && e.is_done()))
    }),
    func!("EndGameWon", [], None, "The round is won.", |w, _| {
        w.ending = w.ending.or(Some(Ending::Won));
        nothing()
    }),
    func!("EndGameLost", [], None, "The round is lost.", |w, _| {
        w.ending = Some(Ending::Lost);
        nothing()
    }),
    func!("GiveGold", [Ty::Number], None, "Gold for the colony - taken away below 0.", |w, a| {
        w.give_gold(a[0].number());
        nothing()
    }),
    func!("GiveWood", [Ty::Number], None, "Wood for the colony - taken away below 0.", |w, a| give(w, Resource::Wood, a)),
    func!("GiveStone", [Ty::Number], None, "Stone for the colony - taken away below 0.", |w, a| give(w, Resource::Stone, a)),
    func!("GiveIronOre", [Ty::Number], None, "Iron ore for the colony - taken away below 0.", |w, a| give(w, Resource::IronOre, a)),
    func!("GiveCoal", [Ty::Number], None, "Coal for the colony - taken away below 0.", |w, a| give(w, Resource::Coal, a)),
    func!("GiveFood", [Ty::Number], None, "Food for the colony - taken away below 0.", |w, a| give(w, Resource::Food, a)),
    func!("GiveIron", [Ty::Number], None, "Iron for the colony - taken away below 0.", |w, a| give(w, Resource::Iron, a)),
    func!("GiveOil", [Ty::Number], None, "Oil for the colony - taken away below 0.", |w, a| give(w, Resource::Oil, a)),
];

fn stock(w: &World, r: Resource) -> Value {
    count(w.stock.get(r) as usize)
}

fn give(w: &mut World, r: Resource, args: &[Value]) -> Value {
    w.give_good(r, args[0].number());
    nothing()
}

/// A number from `lo` to `hi` (swapped if they're the wrong way round) – one roll of the world's dice.
fn roll(w: &mut World, lo: i32, hi: i32) -> i32 {
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let span = (hi as i64 - lo as i64 + 1) as u64;
    // A span of billions still fits the dice: they roll a `u32`.
    lo.saturating_add(w.rng.below(span.min(u32::MAX as u64) as u32) as i32)
}

/// The function with this name (any case, like TAB's symbols) that takes `arity` values.
pub fn find(name: &str, arity: usize) -> Option<&'static Func> {
    FUNCS.iter().find(|f| f.name.eq_ignore_ascii_case(name) && f.params.len() == arity)
}

/// Some function with this name – for telling "unknown" from "wrong number of values".
pub fn named(name: &str) -> Option<&'static Func> {
    FUNCS.iter().find(|f| f.name.eq_ignore_ascii_case(name))
}

/// How it is written, with its types: `Random(number, number)`, `Day`.
pub fn signature(f: &Func) -> String {
    if f.params.is_empty() && f.returns.is_some() {
        return f.name.to_owned();
    }
    let params: Vec<&str> = f.params.iter().map(|t| t.name()).collect();
    format!("{}({})", f.name, params.join(", "))
}

impl World {
    /// The colonists living in houses that aren't infected.
    pub fn colonists(&self) -> u32 {
        self.buildings.iter().filter(|b| !b.infected && b.is_built()).map(|b| b.stats().colonists).sum()
    }

    /// How many finished buildings and units are called `name` (any case, no spaces) – or one of the groups "Unit",
    /// "Building" (also "Structure"), "Dwelling" (houses with colonists), "Infected" (infected buildings).
    pub fn count_kind(&self, name: &str) -> usize {
        let key = |s: &str| s.to_lowercase().replace(' ', "");
        let wanted = key(name);
        let standing = || self.buildings.iter().filter(|b| b.is_built() && !b.infected);
        match wanted.as_str() {
            "unit" => self.units.len(),
            "building" | "structure" => standing().count(),
            "dwelling" => standing().filter(|b| b.stats().colonists > 0).count(),
            "infected" => self.buildings.iter().filter(|b| b.infected).count(),
            _ => standing().filter(|b| key(&b.stats().name) == wanted).count() + self.units.iter().filter(|u| key(&u.stats().name) == wanted).count(),
        }
    }

    /// Is there a building or unit of the colony with this name ID?
    pub fn has_name_id(&self, name_id: &str) -> bool {
        let tagged = |tag: &Option<Box<super::Tag>>| tag.as_ref().is_some_and(|t| t.name_id == name_id);
        self.buildings.iter().any(|b| tagged(&b.tag)) || self.units.iter().any(|u| tagged(&u.tag))
    }

    /// Gold for the colony – or, below 0, taken away.
    pub(super) fn give_gold(&mut self, gold: i32) {
        if gold >= 0 {
            self.earn(gold as u32);
        } else {
            self.gold = self.gold.saturating_sub(gold.unsigned_abs());
        }
    }

    /// A good for the colony (up to its storage) – or, below 0, taken away.
    pub(super) fn give_good(&mut self, r: Resource, n: i32) {
        if n >= 0 {
            self.stock.add(r, n as u32, self.storage(r));
        } else {
            self.stock.take_up_to(r, n.unsigned_abs());
        }
    }

    /// A variable's value: what the round has made of it – else what it starts with, else 0.
    pub fn variable_value(&self, name: &str) -> Value {
        if let Some((_, v)) = self.variables.iter().find(|v| v.0 == name) {
            return v.clone();
        }
        self.story.variables.iter().find(|v| v.name == name).map_or(Value::default(), |v| v.start.as_ty(v.kind.ty()))
    }

    /// A variable as a number – 0 if nobody set it yet.
    #[cfg(test)]
    pub fn variable(&self, name: &str) -> i32 {
        self.variable_value(name).number()
    }

    /// Is the (yes/no) expression true?
    pub fn holds(&mut self, e: &Expr) -> bool {
        self.eval(e).flag()
    }

    /// An expression's value, now.
    pub fn eval(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Lit(v) => v.clone(),
            Expr::Var(name) => self.variable_value(name),
            Expr::Call { name, args } => {
                let values: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
                match find(name, values.len()) {
                    Some(f) => (f.run)(self, &values),
                    None => Value::default(),
                }
            }
            Expr::Not(e) => Value::Flag(!self.eval(e).flag()),
            Expr::Neg(e) => number(self.eval(e).number().saturating_neg()),
            Expr::Bin(Op::Or, l, r) => Value::Flag(self.eval(l).flag() || self.eval(r).flag()),
            Expr::Bin(Op::And, l, r) => Value::Flag(self.eval(l).flag() && self.eval(r).flag()),
            Expr::Bin(op, l, r) => {
                let (a, b) = (self.eval(l), self.eval(r));
                match op {
                    Op::Eq | Op::Ne => {
                        let same = match a.ty() {
                            Ty::Text => a.text() == b.text(),
                            Ty::Flag => a.flag() == b.flag(),
                            Ty::Number => a.number() == b.number(),
                        };
                        Value::Flag(same == (*op == Op::Eq))
                    }
                    Op::Lt => Value::Flag(a.number() < b.number()),
                    Op::Le => Value::Flag(a.number() <= b.number()),
                    Op::Gt => Value::Flag(a.number() > b.number()),
                    Op::Ge => Value::Flag(a.number() >= b.number()),
                    Op::Add if a.ty() == Ty::Text => Value::Text(a.text() + &b.text()),
                    Op::Add => number(a.number().saturating_add(b.number())),
                    Op::Sub => number(a.number().saturating_sub(b.number())),
                    Op::Mul => number(a.number().saturating_mul(b.number())),
                    // Nothing divides by 0 here: the result is 0.
                    Op::Div => number(a.number().checked_div(b.number()).unwrap_or(0)),
                    Op::Or | Op::And => unreachable!("done above"),
                }
            }
        }
    }

    /// Code, statement after statement, now.
    pub fn exec(&mut self, code: &Code) {
        for s in code {
            match s {
                Stmt::Set { name, how, value } => {
                    let v = self.eval(value);
                    self.assign(name, *how, v);
                }
                Stmt::Do(e) => {
                    self.eval(e);
                }
            }
        }
    }

    /// A variable set to a value or changed by it. A declared variable keeps its kind (yes/no stays yes or no).
    fn assign(&mut self, name: &str, how: Assign, v: Value) {
        let now = self.variable_value(name);
        let mut new = match how {
            Assign::Set => v,
            Assign::Add if now.ty() == Ty::Text => Value::Text(now.text() + &v.text()),
            Assign::Add => number(now.number().saturating_add(v.number())),
            Assign::Sub => number(now.number().saturating_sub(v.number())),
        };
        if let Some(kind) = self.story.variables.iter().find(|d| d.name == name).map(|d| d.kind) {
            new = new.as_ty(kind.ty());
        }
        match self.variables.iter_mut().find(|c| c.0 == name) {
            Some(c) => c.1 = new,
            None => self.variables.push((name.to_owned(), new)),
        }
    }
}
