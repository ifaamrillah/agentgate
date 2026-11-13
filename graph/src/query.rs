//! Query ala Cypher (subset) di atas graph di memori:
//!
//! ```text
//! MATCH (a:Function {name: 'calcPromo'})<-[:CALLS*1..2]-(b)
//! WHERE b.file CONTAINS 'cart' AND b.dead = false
//! RETURN b.name, b.file LIMIT 20
//! ```
//!
//! Satu pola: satu node, atau node–relasi–node. Label = jenis simbol (`Function`, `Method`,
//! `Class`, …, `File`, `Route`, `Symbol` = apa saja). Relasi `CALLS`, `INHERITS`, `IMPORTS`,
//! `HANDLES`, lintas repo `HTTP` dan `EVENT` (kosong = apa saja), panjang variabel `*min..max` (maks. 5). Properti: `name`,
//! `qualified`, `kind`, `file`, `line`, `repo`, `module`, `exported`, `dead`, `test`. Operator
//! `=`, `<>`, `CONTAINS`, `STARTS WITH`, `ENDS WITH`, `=~` (regex), digabung `AND`/`OR`/`NOT`.
//! `RETURN` berisi `var`, `var.prop`, atau `count(*)`, opsional `LIMIT n` (maks. 200).
use std::collections::{HashSet, VecDeque};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::facts::SymbolKind;

pub const MAX_LIMIT: usize = 200;
const MAX_DEPTH: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QNode {
    pub name: String,
    pub qualified: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: u32,
    pub repo: String,
    pub module: String,
    pub exported: bool,
    pub dead: bool,
    pub test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QEdge {
    pub from: usize,
    pub to: usize,
    /// `calls`, `inherits`, `imports`, `handles`.
    pub kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct QGraph {
    pub nodes: Vec<QNode>,
    pub edges: Vec<QEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct QueryError(pub String);

fn err<T>(m: impl Into<String>) -> Result<T, QueryError> {
    Err(QueryError(m.into()))
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Str(String),
    Num(i64),
    Sym(&'static str),
}

fn lex(q: &str) -> Result<Vec<Tok>, QueryError> {
    let mut out = Vec::new();
    let c: Vec<char> = q.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '\'' || ch == '"' {
            let mut s = String::new();
            i += 1;
            while i < c.len() && c[i] != ch {
                if c[i] == '\\' && i + 1 < c.len() {
                    i += 1;
                }
                s.push(c[i]);
                i += 1;
            }
            if i >= c.len() {
                return err("String tidak ditutup.");
            }
            i += 1;
            out.push(Tok::Str(s));
        } else if ch.is_ascii_digit() {
            let start = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            let n: String = c[start..i].iter().collect();
            out.push(Tok::Num(n.parse().unwrap_or(0)));
        } else if ch.is_alphanumeric() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Word(c[start..i].iter().collect()));
        } else {
            let two: String = c[i..(i + 2).min(c.len())].iter().collect();
            let sym = match two.as_str() {
                "<>" => Some("<>"),
                "=~" => Some("=~"),
                ".." => Some(".."),
                "->" => Some("->"),
                "<-" => Some("<-"),
                _ => None,
            };
            if let Some(s) = sym {
                out.push(Tok::Sym(s));
                i += 2;
                continue;
            }
            let s = match ch {
                '(' => "(",
                ')' => ")",
                '[' => "[",
                ']' => "]",
                '{' => "{",
                '}' => "}",
                ':' => ":",
                ',' => ",",
                '.' => ".",
                '-' => "-",
                '*' => "*",
                '=' => "=",
                _ => return err(format!("Karakter tidak dikenal: {ch}")),
            };
            out.push(Tok::Sym(s));
            i += 1;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Str(String),
    Num(i64),
    Bool(bool),
}

#[derive(Debug, Clone)]
struct NodePat {
    var: String,
    kind: Option<SymbolKind>,
    props: Vec<(String, Val)>,
}

#[derive(Debug, Clone)]
struct RelPat {
    kind: Option<String>,
    min: u8,
    max: u8,
    /// `true` = (a)-[]->(b), `false` = (a)<-[]-(b).
    forward: bool,
}

#[derive(Debug, Clone)]
enum Cond {
    Cmp {
        var: String,
        prop: String,
        op: String,
        val: Val,
    },
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Not(Box<Self>),
}

#[derive(Debug, Clone)]
enum Item {
    Var(String),
    Prop(String, String),
    Count,
}

/// Query hasil parse.
#[derive(Debug, Clone)]
pub struct Query {
    a: NodePat,
    rel: Option<(RelPat, NodePat)>,
    cond: Option<Cond>,
    items: Vec<Item>,
    limit: usize,
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn sym(&mut self, s: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Sym(x)) if *x == s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), QueryError> {
        if self.sym(s) {
            Ok(())
        } else {
            err(format!("Diharapkan `{s}`."))
        }
    }
    fn kw(&mut self, k: &str) -> bool {
        if let Some(Tok::Word(w)) = self.peek()
            && w.eq_ignore_ascii_case(k)
        {
            self.i += 1;
            return true;
        }
        false
    }
    fn word(&mut self) -> Result<String, QueryError> {
        match self.next() {
            Some(Tok::Word(w)) => Ok(w),
            _ => err("Diharapkan nama."),
        }
    }
    fn val(&mut self) -> Result<Val, QueryError> {
        match self.next() {
            Some(Tok::Str(s)) => Ok(Val::Str(s)),
            Some(Tok::Num(n)) => Ok(Val::Num(n)),
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("true") => Ok(Val::Bool(true)),
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("false") => Ok(Val::Bool(false)),
            _ => err("Diharapkan nilai (string, angka, true/false)."),
        }
    }
    fn node(&mut self) -> Result<NodePat, QueryError> {
        self.expect("(")?;
        let var = match self.peek() {
            Some(Tok::Word(_)) => self.word()?,
            _ => String::new(),
        };
        let mut kind = None;
        if self.sym(":") {
            let label = self.word()?;
            if !label.eq_ignore_ascii_case("symbol") {
                kind = Some(
                    SymbolKind::parse(&label.to_ascii_lowercase())
                        .ok_or_else(|| QueryError(format!("Label tidak dikenal: {label}")))?,
                );
            }
        }
        let mut props = Vec::new();
        if self.sym("{") {
            loop {
                let k = self.word()?;
                self.expect(":")?;
                props.push((k, self.val()?));
                if !self.sym(",") {
                    break;
                }
            }
            self.expect("}")?;
        }
        self.expect(")")?;
        Ok(NodePat { var, kind, props })
    }
    fn rel(&mut self) -> Result<Option<RelPat>, QueryError> {
        let back = if self.sym("<-") {
            true
        } else if self.sym("-") {
            false
        } else {
            return Ok(None);
        };
        let mut kind = None;
        let (mut min, mut max) = (1u8, 1u8);
        if self.sym("[") {
            if self.sym(":") {
                let k = self.word()?.to_ascii_lowercase();
                if !matches!(
                    k.as_str(),
                    "calls" | "inherits" | "imports" | "handles" | "http" | "event"
                ) {
                    return err(format!("Relasi tidak dikenal: {k}"));
                }
                kind = Some(k);
            }
            if self.sym("*") {
                let num = |p: &mut Self| match p.next() {
                    Some(Tok::Num(n)) => Ok(u8::try_from(n).unwrap_or(MAX_DEPTH)),
                    _ => err("Diharapkan angka."),
                };
                if matches!(self.peek(), Some(Tok::Num(_))) {
                    min = num(self)?;
                    max = min;
                }
                if self.sym("..") {
                    max = num(self)?;
                } else if min == 1
                    && max == 1
                    && !matches!(self.t.get(self.i.saturating_sub(1)), Some(Tok::Num(_)))
                {
                    max = MAX_DEPTH;
                }
            }
            self.expect("]")?;
        }
        let forward = if back {
            self.expect("-")?;
            false
        } else if self.sym("->") {
            true
        } else {
            return err("Arah relasi wajib (`->` atau `<-`).");
        };
        if max == 0 || min > max || max > MAX_DEPTH {
            return err(format!("Panjang relasi harus 1..{MAX_DEPTH}."));
        }
        Ok(Some(RelPat {
            kind,
            min: min.max(1),
            max,
            forward,
        }))
    }
    fn cmp(&mut self) -> Result<Cond, QueryError> {
        if self.kw("NOT") {
            return Ok(Cond::Not(Box::new(self.cmp()?)));
        }
        if self.sym("(") {
            let c = self.cond()?;
            self.expect(")")?;
            return Ok(c);
        }
        let var = self.word()?;
        self.expect(".")?;
        let prop = self.word()?;
        let op = if self.sym("=") {
            "="
        } else if self.sym("<>") {
            "<>"
        } else if self.sym("=~") {
            "=~"
        } else if self.kw("CONTAINS") {
            "contains"
        } else if self.kw("STARTS") {
            if !self.kw("WITH") {
                return err("Diharapkan `STARTS WITH`.");
            }
            "starts"
        } else if self.kw("ENDS") {
            if !self.kw("WITH") {
                return err("Diharapkan `ENDS WITH`.");
            }
            "ends"
        } else {
            return err("Operator tidak dikenal.");
        };
        Ok(Cond::Cmp {
            var,
            prop,
            op: op.into(),
            val: self.val()?,
        })
    }
    fn cond(&mut self) -> Result<Cond, QueryError> {
        let mut left = self.cmp()?;
        loop {
            if self.kw("AND") {
                left = Cond::And(Box::new(left), Box::new(self.cmp()?));
            } else if self.kw("OR") {
                left = Cond::Or(Box::new(left), Box::new(self.cmp()?));
            } else {
                return Ok(left);
            }
        }
    }
}

/// Parse query.
///
/// # Errors
///
/// Sintaks di luar subset yang didukung, dengan pesan berbahasa Indonesia.
pub fn parse(q: &str) -> Result<Query, QueryError> {
    let mut p = P { t: lex(q)?, i: 0 };
    if !p.kw("MATCH") {
        return err("Query harus diawali MATCH.");
    }
    let a = p.node()?;
    let rel = match p.rel()? {
        Some(r) => Some((r, p.node()?)),
        None => None,
    };
    let cond = if p.kw("WHERE") { Some(p.cond()?) } else { None };
    if !p.kw("RETURN") {
        return err("Diharapkan RETURN.");
    }
    let mut items = Vec::new();
    loop {
        let w = p.word()?;
        if w.eq_ignore_ascii_case("count") {
            p.expect("(")?;
            p.expect("*")?;
            p.expect(")")?;
            items.push(Item::Count);
        } else if p.sym(".") {
            items.push(Item::Prop(w, p.word()?));
        } else {
            items.push(Item::Var(w));
        }
        if !p.sym(",") {
            break;
        }
    }
    let mut limit = 50;
    if p.kw("LIMIT") {
        match p.next() {
            Some(Tok::Num(n)) => limit = usize::try_from(n).unwrap_or(MAX_LIMIT).min(MAX_LIMIT),
            _ => return err("LIMIT butuh angka."),
        }
    }
    if p.peek().is_some() {
        return err("Ada sisa teks setelah query.");
    }
    let vars: Vec<&str> = std::iter::once(a.var.as_str())
        .chain(rel.as_ref().map(|(_, b)| b.var.as_str()))
        .filter(|v| !v.is_empty())
        .collect();
    for it in &items {
        if let Item::Var(v) | Item::Prop(v, _) = it
            && !vars.contains(&v.as_str())
        {
            return err(format!("Variabel tidak dikenal: {v}"));
        }
    }
    Ok(Query {
        a,
        rel,
        cond,
        items,
        limit,
    })
}

/// Hasil query (tabel teks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

fn prop(n: &QNode, p: &str) -> Option<Val> {
    Some(match p {
        "name" => Val::Str(n.name.clone()),
        "qualified" => Val::Str(n.qualified.clone()),
        "kind" => Val::Str(n.kind.as_str().into()),
        "file" => Val::Str(n.file.clone()),
        "repo" => Val::Str(n.repo.clone()),
        "module" => Val::Str(n.module.clone()),
        "line" => Val::Num(i64::from(n.line)),
        "exported" => Val::Bool(n.exported),
        "dead" => Val::Bool(n.dead),
        "test" => Val::Bool(n.test),
        _ => return None,
    })
}

fn show(v: &Val) -> String {
    match v {
        Val::Str(s) => s.clone(),
        Val::Num(n) => n.to_string(),
        Val::Bool(b) => b.to_string(),
    }
}

fn matches_pat(n: &QNode, p: &NodePat) -> bool {
    p.kind.is_none_or(|k| n.kind == k)
        && p.props
            .iter()
            .all(|(k, v)| prop(n, k).is_some_and(|x| x == *v))
}

fn eval(c: &Cond, bind: &[(&str, &QNode)]) -> Result<bool, QueryError> {
    Ok(match c {
        Cond::And(a, b) => eval(a, bind)? && eval(b, bind)?,
        Cond::Or(a, b) => eval(a, bind)? || eval(b, bind)?,
        Cond::Not(a) => !eval(a, bind)?,
        Cond::Cmp {
            var,
            prop: p,
            op,
            val,
        } => {
            let Some((_, n)) = bind.iter().find(|(v, _)| v == var) else {
                return err(format!("Variabel tidak dikenal: {var}"));
            };
            let Some(x) = prop(n, p) else {
                return err(format!("Properti tidak dikenal: {p}"));
            };
            let (xs, vs) = (show(&x), show(val));
            match op.as_str() {
                "=" => x == *val,
                "<>" => x != *val,
                "contains" => xs.contains(&vs),
                "starts" => xs.starts_with(&vs),
                "ends" => xs.ends_with(&vs),
                "=~" => Regex::new(&vs)
                    .map_err(|e| QueryError(format!("Regex tidak valid: {e}")))?
                    .is_match(&xs),
                _ => false,
            }
        }
    })
}

/// Jalankan query.
///
/// # Errors
///
/// Properti/variabel tidak dikenal atau regex tidak valid.
pub fn execute(g: &QGraph, q: &Query) -> Result<QueryResult, QueryError> {
    let mut out: Vec<(usize, Option<usize>)> = Vec::new();
    let cap = 10_000;
    let starts: Vec<usize> = (0..g.nodes.len())
        .filter(|&i| matches_pat(&g.nodes[i], &q.a))
        .collect();
    match &q.rel {
        None => out.extend(starts.into_iter().map(|a| (a, None))),
        Some((r, bp)) => {
            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); g.nodes.len()];
            for e in &g.edges {
                if r.kind.as_deref().is_none_or(|k| k == e.kind) {
                    if r.forward {
                        adj[e.from].push(e.to);
                    } else {
                        adj[e.to].push(e.from);
                    }
                }
            }
            'outer: for a in starts {
                let mut seen: HashSet<usize> = HashSet::from([a]);
                let mut queue: VecDeque<(usize, u8)> = VecDeque::from([(a, 0)]);
                while let Some((n, d)) = queue.pop_front() {
                    if d >= r.max {
                        continue;
                    }
                    for &m in &adj[n] {
                        if seen.insert(m) {
                            if d + 1 >= r.min && matches_pat(&g.nodes[m], bp) {
                                out.push((a, Some(m)));
                                if out.len() >= cap {
                                    break 'outer;
                                }
                            }
                            queue.push_back((m, d + 1));
                        }
                    }
                }
            }
        }
    }
    let b_var = q.rel.as_ref().map_or("", |(_, b)| b.var.as_str());
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut seen_rows: HashSet<Vec<String>> = HashSet::new();
    let mut count = 0usize;
    let has_count = q.items.iter().any(|i| matches!(i, Item::Count));
    let mut truncated = false;
    for (a, b) in out {
        let mut bind: Vec<(&str, &QNode)> = vec![(q.a.var.as_str(), &g.nodes[a])];
        if let Some(b) = b {
            bind.push((b_var, &g.nodes[b]));
        }
        if let Some(c) = &q.cond
            && !eval(c, &bind)?
        {
            continue;
        }
        count += 1;
        if has_count {
            continue;
        }
        let row: Vec<String> = q
            .items
            .iter()
            .map(|it| {
                let get = |v: &str| bind.iter().find(|(x, _)| *x == v).map(|(_, n)| *n);
                match it {
                    Item::Var(v) => get(v).map_or_else(String::new, |n| {
                        format!("{} ({}:{})", n.qualified, n.file, n.line)
                    }),
                    Item::Prop(v, p) => get(v)
                        .and_then(|n| prop(n, p))
                        .map_or_else(String::new, |x| show(&x)),
                    Item::Count => String::new(),
                }
            })
            .collect();
        if seen_rows.insert(row.clone()) {
            if rows.len() >= q.limit {
                truncated = true;
                break;
            }
            rows.push(row);
        }
    }
    let columns = q
        .items
        .iter()
        .map(|it| match it {
            Item::Var(v) => v.clone(),
            Item::Prop(v, p) => format!("{v}.{p}"),
            Item::Count => "count(*)".into(),
        })
        .collect();
    if has_count {
        rows = vec![
            q.items
                .iter()
                .map(|it| {
                    if matches!(it, Item::Count) {
                        count.to_string()
                    } else {
                        String::new()
                    }
                })
                .collect(),
        ];
    }
    Ok(QueryResult {
        columns,
        rows,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn n(name: &str, kind: SymbolKind, file: &str, dead: bool) -> QNode {
        QNode {
            name: name.into(),
            qualified: name.into(),
            kind,
            file: file.into(),
            line: 1,
            repo: "web".into(),
            module: "cart".into(),
            exported: true,
            dead,
            test: false,
        }
    }

    fn g() -> QGraph {
        QGraph {
            nodes: vec![
                n("calcPromo", SymbolKind::Function, "cart/promo.ts", false),
                n("useTotals", SymbolKind::Function, "cart/totals.ts", false),
                n(
                    "Summary",
                    SymbolKind::Function,
                    "checkout/Summary.tsx",
                    false,
                ),
                n("legacy", SymbolKind::Function, "cart/promo.ts", true),
                n("Cart", SymbolKind::Class, "cart/cart.ts", false),
            ],
            edges: vec![
                QEdge {
                    from: 1,
                    to: 0,
                    kind: "calls".into(),
                },
                QEdge {
                    from: 2,
                    to: 1,
                    kind: "calls".into(),
                },
            ],
        }
    }

    fn run(q: &str) -> Vec<Vec<String>> {
        execute(&g(), &parse(q).unwrap()).unwrap().rows
    }

    #[test]
    fn match_where_return() {
        assert_eq!(
            run("MATCH (f:Function) WHERE f.dead = true RETURN f.name"),
            vec![vec!["legacy"]]
        );
        assert_eq!(
            run("match (a {name: 'calcPromo'})<-[:CALLS*1..2]-(b) return b.name, b.file"),
            vec![
                vec!["useTotals", "cart/totals.ts"],
                vec!["Summary", "checkout/Summary.tsx"]
            ]
        );
        assert_eq!(
            run("MATCH (a {name: 'calcPromo'})<-[:CALLS]-(b) RETURN b.name"),
            vec![vec!["useTotals"]]
        );
        assert_eq!(
            run(
                "MATCH (s)-[:CALLS*]->(t) WHERE s.file STARTS WITH 'checkout' AND NOT t.name = 'useTotals' RETURN t.name"
            ),
            vec![vec!["calcPromo"]]
        );
        assert_eq!(
            run("MATCH (x:Symbol) WHERE x.name =~ '^[A-Z]' RETURN count(*)"),
            vec![vec!["2"]]
        );
        assert_eq!(
            run("MATCH (c:Class) RETURN c"),
            vec![vec!["Cart (cart/cart.ts:1)"]]
        );
        assert_eq!(run("MATCH (f:Function) RETURN f.name LIMIT 1").len(), 1);
    }

    #[test]
    fn errors_are_readable() {
        for (q, m) in [
            ("RETURN x", "MATCH"),
            ("MATCH (a:Widget) RETURN a", "Label"),
            ("MATCH (a)-[:EATS]->(b) RETURN a", "Relasi"),
            ("MATCH (a)-[:CALLS*1..9]->(b) RETURN a", "Panjang"),
            ("MATCH (a) RETURN z", "Variabel"),
            ("MATCH (a) RETURN a.name LIMIT", "LIMIT"),
            ("MATCH (a {name: 'x) RETURN a", "String"),
        ] {
            let e = parse(q).unwrap_err();
            assert!(e.0.contains(m), "{q}: {}", e.0);
        }
        let e = execute(
            &g(),
            &parse("MATCH (a) WHERE a.color = 'x' RETURN a").unwrap(),
        )
        .unwrap_err();
        assert!(e.0.contains("Properti"));
    }
}
