//! Query ala JQL (pencarian lanjutan, saved filter).
//!
//! Tata bahasa (R3: `OR`, `NOT`, kurung):
//!
//! ```text
//! query  := [or] [ORDER BY field [ASC|DESC]]
//! or     := and (OR and)*
//! and    := unary (AND unary)*
//! unary  := NOT unary | "(" or ")" | syarat
//! syarat := field op nilai
//! ```
//!
//! Presedensi `NOT` > `AND` > `OR`, sama dengan Jira: `a OR b AND c` = `a OR (b AND c)`.
//! Operator: `=`, `!=`, `~` (mengandung), `in (a, b)`, `not in (…)`, `is EMPTY`, `is not EMPTY`, dan
//! `<`, `<=`, `>`, `>=` untuk `points`/`due`. Nilai: kata, `"teks berspasi"`, `currentUser()`,
//! `openSprints()`, `EMPTY`. Kata kunci tidak peka huruf besar/kecil; perbandingan nilai juga.
//!
//! Error berupa kode Fluent (`jql-…`) + argumen ([`JqlError`]); kalimatnya di `ui-planning.ftl`.
//! [`context`] membaca teks sebelum kursor untuk autocomplete kotak query (apa yang diharapkan
//! berikutnya: field, operator, nilai, atau sambungan).
//!
//! Evaluasi berjalan di memori atas [`Facts`] tiap tiket (lihat controller pencarian lanjutan).
use chrono::NaiveDate;

use crate::common::i18n;

/// Batas syarat per query.
pub const MAX_CLAUSES: usize = 20;
/// Batas kedalaman kurung/`NOT` bersarang.
pub const MAX_DEPTH: usize = 10;

pub const QUOTE_OPEN: &str = "jql-quote-open";
pub const BANG: &str = "jql-bang";
pub const FIELD_UNKNOWN: &str = "jql-field-unknown";
pub const ORDER_BY: &str = "jql-order-by";
pub const ORDER_FIELD: &str = "jql-order-field";
pub const ORDER_LAST: &str = "jql-order-last";
pub const JOIN: &str = "jql-join";
pub const CLAUSE_START: &str = "jql-clause-start";
pub const OP_UNKNOWN: &str = "jql-op-unknown";
pub const OP_COMPARE: &str = "jql-op-compare";
pub const VALUE_MISSING: &str = "jql-value-missing";
pub const NOT_IN: &str = "jql-not-in";
pub const IN_PAREN: &str = "jql-in-paren";
pub const IN_SEP: &str = "jql-in-sep";
pub const PAREN_OPEN: &str = "jql-paren-open";
pub const PAREN_CLOSE: &str = "jql-paren-close";
pub const EMPTY_GROUP: &str = "jql-empty-group";
pub const IS_EMPTY: &str = "jql-is-empty";
pub const TOO_MANY: &str = "jql-too-many";
pub const TOO_DEEP: &str = "jql-too-deep";
pub const DANGLING: &str = "jql-dangling";

/// Semua kode error (tes kontrak i18n).
pub const ERROR_CODES: &[&str] = &[
    QUOTE_OPEN,
    BANG,
    FIELD_UNKNOWN,
    ORDER_BY,
    ORDER_FIELD,
    ORDER_LAST,
    JOIN,
    CLAUSE_START,
    OP_UNKNOWN,
    OP_COMPARE,
    VALUE_MISSING,
    NOT_IN,
    IN_PAREN,
    IN_SEP,
    PAREN_OPEN,
    PAREN_CLOSE,
    EMPTY_GROUP,
    IS_EMPTY,
    TOO_MANY,
    TOO_DEEP,
    DANGLING,
];

/// Query tidak bisa dibaca: kode Fluent + argumen (`name` = field/kata yang bermasalah).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JqlError {
    pub code: &'static str,
    pub name: String,
}

impl JqlError {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            name: String::new(),
        }
    }

    fn named(code: &'static str, name: &str) -> Self {
        Self {
            code,
            name: name.to_string(),
        }
    }

    /// Kalimat untuk pengguna (Fluent `id`).
    #[must_use]
    pub fn message(&self) -> String {
        let list = FIELDS[..6]
            .iter()
            .map(|f| f.0)
            .collect::<Vec<_>>()
            .join(", ");
        i18n::text(
            self.code,
            &[
                ("name", &self.name),
                ("list", &list),
                ("max", &MAX_CLAUSES.to_string()),
                ("depth", &MAX_DEPTH.to_string()),
            ],
        )
    }
}

impl std::fmt::Display for JqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Project,
    Stage,
    Status,
    Assignee,
    Type,
    Sprint,
    Label,
    Priority,
    Points,
    Text,
    Release,
    Component,
    Due,
    Created,
    Updated,
}

/// Field yang dikenal: (nama di query, field). Keterangan bantuan = Fluent `jql-field-<nama>`.
pub const FIELDS: &[(&str, Field)] = &[
    ("project", Field::Project),
    ("stage", Field::Stage),
    ("status", Field::Status),
    ("assignee", Field::Assignee),
    ("type", Field::Type),
    ("sprint", Field::Sprint),
    ("label", Field::Label),
    ("priority", Field::Priority),
    ("points", Field::Points),
    ("text", Field::Text),
    ("fixversion", Field::Release),
    ("component", Field::Component),
    ("due", Field::Due),
    ("created", Field::Created),
    ("updated", Field::Updated),
];

/// Kunci Fluent keterangan field `name`.
#[must_use]
pub fn field_hint_key(name: &str) -> String {
    format!("jql-field-{name}")
}

impl Field {
    /// Nama field di query.
    #[must_use]
    pub fn name(self) -> &'static str {
        FIELDS
            .iter()
            .find(|(_, f)| *f == self)
            .map_or("text", |(n, _)| n)
    }

    /// Field berurutan (bisa dibandingkan dengan `<`, `>`).
    #[must_use]
    pub fn ordered(self) -> bool {
        matches!(self, Self::Points | Self::Due | Self::Created | Self::Updated)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Like,
    In,
    NotIn,
    Is,
    IsNot,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    CurrentUser,
    OpenSprints,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    pub field: Field,
    pub op: Op,
    pub values: Vec<Value>,
}

/// Pohon syarat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Clause(Clause),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    /// `None` = semua tiket.
    pub expr: Option<Expr>,
    /// Urutan: field dan menurun.
    pub order: Option<(Field, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String),
    Quoted(String),
    Op(&'static str),
    Open,
    Close,
    Comma,
}

const BREAK: [char; 10] = ['(', ')', ',', '"', '\'', '!', '<', '>', '=', '~'];

/// Pecah teks menjadi token. `lenient` (autocomplete): kutip terbuka di akhir menjadi token
/// `Quoted` berisi sisa teks, `!` tunggal diabaikan.
fn tokenize(src: &str, lenient: bool) -> Result<Vec<Tok>, JqlError> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                out.push(Tok::Open);
                i += 1;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '"' | '\'' => match chars[i + 1..].iter().position(|x| *x == c) {
                Some(end) => {
                    out.push(Tok::Quoted(chars[i + 1..i + 1 + end].iter().collect()));
                    i += end + 2;
                }
                None if lenient => {
                    out.push(Tok::Quoted(chars[i + 1..].iter().collect()));
                    i = chars.len();
                }
                None => return Err(JqlError::new(QUOTE_OPEN)),
            },
            '!' | '<' | '>' | '=' | '~' => {
                let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
                let op = match two.as_str() {
                    "!=" => "!=",
                    "<=" => "<=",
                    ">=" => ">=",
                    _ => match c {
                        '=' => "=",
                        '~' => "~",
                        '<' => "<",
                        '>' => ">",
                        _ if lenient => {
                            i += 1;
                            continue;
                        }
                        _ => return Err(JqlError::new(BANG)),
                    },
                };
                i += op.len();
                out.push(Tok::Op(op));
            }
            _ => {
                let start = i;
                while i < chars.len() && !chars[i].is_whitespace() && !BREAK.contains(&chars[i]) {
                    i += 1;
                }
                let mut word: String = chars[start..i].iter().collect();
                // Fungsi: `currentUser()` → satu kata.
                if i + 1 < chars.len() && chars[i] == '(' && chars[i + 1] == ')' {
                    word.push_str("()");
                    i += 2;
                }
                out.push(Tok::Word(word));
            }
        }
    }
    Ok(out)
}

fn field_of(name: &str) -> Option<Field> {
    let n = name.to_lowercase();
    let n = match n.as_str() {
        "fixversion" | "fix_version" | "release" | "versi" => "fixversion",
        "key" | "kode" | "summary" | "judul" => "text",
        other => other,
    };
    FIELDS.iter().find(|(k, _)| *k == n).map(|(_, f)| *f)
}

fn value_of(tok: &Tok) -> Option<Value> {
    match tok {
        Tok::Quoted(s) => Some(Value::Text(s.clone())),
        Tok::Word(w) => Some(match w.to_lowercase().as_str() {
            "currentuser()" => Value::CurrentUser,
            "opensprints()" => Value::OpenSprints,
            "empty" | "null" => Value::Empty,
            _ => Value::Text(w.clone()),
        }),
        _ => None,
    }
}

fn is_kw(t: Option<&Tok>, kw: &str) -> bool {
    matches!(t, Some(Tok::Word(w)) if w.eq_ignore_ascii_case(kw))
}

/// Teks token untuk pesan error.
fn shown(t: &Tok) -> String {
    match t {
        Tok::Word(w) => w.clone(),
        Tok::Quoted(q) => format!("\"{q}\""),
        Tok::Op(o) => (*o).to_string(),
        Tok::Open => "(".into(),
        Tok::Close => ")".into(),
        Tok::Comma => ",".into(),
    }
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    clauses: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }

    /// Token berikutnya mengakhiri ekspresi (bukan awal syarat).
    fn at_end(&self) -> bool {
        self.peek().is_none() || is_kw(self.peek(), "order")
    }

    fn or(&mut self, depth: usize) -> Result<Expr, JqlError> {
        let mut parts = vec![self.and(depth)?];
        while is_kw(self.peek(), "or") {
            self.i += 1;
            if self.at_end() || self.peek() == Some(&Tok::Close) {
                return Err(JqlError::named(DANGLING, "OR"));
            }
            parts.push(self.and(depth)?);
        }
        Ok(if parts.len() == 1 {
            parts.remove(0)
        } else {
            Expr::Or(parts)
        })
    }

    fn and(&mut self, depth: usize) -> Result<Expr, JqlError> {
        let mut parts = vec![self.unary(depth)?];
        loop {
            if is_kw(self.peek(), "and") {
                self.i += 1;
                if self.at_end() || self.peek() == Some(&Tok::Close) {
                    return Err(JqlError::named(DANGLING, "AND"));
                }
                parts.push(self.unary(depth)?);
            } else if self.at_end()
                || is_kw(self.peek(), "or")
                || self.peek() == Some(&Tok::Close)
            {
                break;
            } else {
                return Err(JqlError::new(JOIN));
            }
        }
        Ok(if parts.len() == 1 {
            parts.remove(0)
        } else {
            Expr::And(parts)
        })
    }

    fn unary(&mut self, depth: usize) -> Result<Expr, JqlError> {
        if depth >= MAX_DEPTH {
            return Err(JqlError::new(TOO_DEEP));
        }
        if is_kw(self.peek(), "not") {
            self.i += 1;
            if self.at_end() || self.peek() == Some(&Tok::Close) {
                return Err(JqlError::named(DANGLING, "NOT"));
            }
            return Ok(Expr::Not(Box::new(self.unary(depth + 1)?)));
        }
        if self.peek() == Some(&Tok::Open) {
            self.i += 1;
            if self.peek() == Some(&Tok::Close) {
                return Err(JqlError::new(EMPTY_GROUP));
            }
            let inner = self.or(depth + 1)?;
            if self.next() != Some(Tok::Close) {
                return Err(JqlError::new(PAREN_OPEN));
            }
            return Ok(inner);
        }
        self.clause().map(Expr::Clause)
    }

    fn clause(&mut self) -> Result<Clause, JqlError> {
        let name = match self.next() {
            Some(Tok::Word(w)) => w,
            Some(Tok::Close) => return Err(JqlError::new(PAREN_CLOSE)),
            Some(other) => return Err(JqlError::named(CLAUSE_START, &shown(&other))),
            None => return Err(JqlError::named(CLAUSE_START, "")),
        };
        let field = field_of(&name).ok_or_else(|| JqlError::named(FIELD_UNKNOWN, &name))?;
        let op = match self.next() {
            Some(Tok::Op("=")) => Op::Eq,
            Some(Tok::Op("!=")) => Op::Ne,
            Some(Tok::Op("~")) => Op::Like,
            Some(Tok::Op("<")) => Op::Lt,
            Some(Tok::Op("<=")) => Op::Le,
            Some(Tok::Op(">")) => Op::Gt,
            Some(Tok::Op(">=")) => Op::Ge,
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("in") => Op::In,
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("not") => {
                if is_kw(self.peek(), "in") {
                    self.i += 1;
                    Op::NotIn
                } else {
                    return Err(JqlError::new(NOT_IN));
                }
            }
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("is") => {
                if is_kw(self.peek(), "not") {
                    self.i += 1;
                    Op::IsNot
                } else {
                    Op::Is
                }
            }
            _ => return Err(JqlError::named(OP_UNKNOWN, &name)),
        };
        if matches!(op, Op::Lt | Op::Le | Op::Gt | Op::Ge) && !field.ordered() {
            return Err(JqlError::named(OP_COMPARE, &name));
        }
        let func = matches!(self.peek(), Some(Tok::Word(w)) if w.ends_with("()"));
        let values = if matches!(op, Op::In | Op::NotIn) && func {
            // `sprint in openSprints()`, `assignee in currentUser()`.
            vec![self.value(&name)?]
        } else if matches!(op, Op::In | Op::NotIn) {
            if self.next() != Some(Tok::Open) {
                return Err(JqlError::new(IN_PAREN));
            }
            let mut vals = Vec::new();
            loop {
                match self.peek() {
                    None => return Err(JqlError::new(PAREN_OPEN)),
                    Some(Tok::Close) if vals.is_empty() => return Err(JqlError::new(EMPTY_GROUP)),
                    _ => {}
                }
                vals.push(self.value(&name)?);
                match self.next() {
                    Some(Tok::Comma) => {}
                    Some(Tok::Close) => break,
                    None => return Err(JqlError::new(PAREN_OPEN)),
                    _ => return Err(JqlError::new(IN_SEP)),
                }
            }
            vals
        } else {
            vec![self.value(&name)?]
        };
        if matches!(op, Op::Is | Op::IsNot) && values != [Value::Empty] {
            return Err(JqlError::new(IS_EMPTY));
        }
        self.clauses += 1;
        if self.clauses > MAX_CLAUSES {
            return Err(JqlError::new(TOO_MANY));
        }
        Ok(Clause { field, op, values })
    }

    fn value(&mut self, name: &str) -> Result<Value, JqlError> {
        // Kata kunci sambungan bukan nilai: `status = AND …` berarti nilai lupa ditulis.
        let missing = || JqlError::named(VALUE_MISSING, name);
        match self.peek() {
            Some(Tok::Word(w))
                if ["and", "or", "order"]
                    .iter()
                    .any(|k| w.eq_ignore_ascii_case(k)) =>
            {
                Err(missing())
            }
            Some(t) => {
                let v = value_of(t).ok_or_else(missing)?;
                self.i += 1;
                Ok(v)
            }
            None => Err(missing()),
        }
    }

    fn order(&mut self) -> Result<(Field, bool), JqlError> {
        self.i += 1; // ORDER
        if !is_kw(self.peek(), "by") {
            return Err(JqlError::new(ORDER_BY));
        }
        self.i += 1;
        let Some(Tok::Word(f)) = self.next() else {
            return Err(JqlError::new(ORDER_FIELD));
        };
        let field = field_of(&f).ok_or_else(|| JqlError::named(FIELD_UNKNOWN, &f))?;
        let desc = if is_kw(self.peek(), "desc") {
            self.i += 1;
            true
        } else {
            if is_kw(self.peek(), "asc") {
                self.i += 1;
            }
            false
        };
        if self.peek().is_some() {
            return Err(JqlError::new(ORDER_LAST));
        }
        Ok((field, desc))
    }
}

/// Parse query. Query kosong = semua tiket.
///
/// # Errors
///
/// [`JqlError`] dengan kode Fluent dan argumennya.
pub fn parse(src: &str) -> Result<Query, JqlError> {
    let mut p = Parser {
        toks: tokenize(src, false)?,
        i: 0,
        clauses: 0,
    };
    let mut q = Query::default();
    if !p.at_end() {
        q.expr = Some(p.or(0)?);
    }
    match p.peek() {
        None => {}
        Some(t) if is_kw(Some(t), "order") => q.order = Some(p.order()?),
        Some(Tok::Close) => return Err(JqlError::new(PAREN_CLOSE)),
        Some(_) => return Err(JqlError::new(JOIN)),
    }
    Ok(q)
}

/// Fakta satu tiket untuk evaluasi (semua teks sudah huruf kecil).
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub project: String,
    /// Kunci dan nama stage.
    pub stage: Vec<String>,
    pub status: String,
    /// Id, nama, email penerima.
    pub assignee: Vec<String>,
    pub assignee_id: Option<uuid::Uuid>,
    /// Kunci dan nama tipe.
    pub ticket_type: Vec<String>,
    pub sprint: Option<String>,
    pub sprint_open: bool,
    pub labels: Vec<String>,
    pub priority: String,
    pub points: Option<i32>,
    /// Kode, judul, dan deskripsi.
    pub text: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub due: Option<NaiveDate>,
}

fn text_values(f: &Facts, field: Field) -> Vec<String> {
    match field {
        Field::Project => vec![f.project.clone()],
        Field::Stage => f.stage.clone(),
        Field::Status => vec![f.status.clone(), f.status.replace('_', " ")],
        Field::Assignee => f.assignee.clone(),
        Field::Type => f.ticket_type.clone(),
        Field::Sprint => f.sprint.clone().into_iter().collect(),
        Field::Label => f.labels.clone(),
        Field::Priority => vec![f.priority.clone()],
        Field::Points => f.points.map(|p| p.to_string()).into_iter().collect(),
        Field::Text => vec![f.text.clone()],
        Field::Release => f.release.clone().into_iter().collect(),
        Field::Component => f.component.clone().into_iter().collect(),
        Field::Due => f.due.map(|d| d.to_string()).into_iter().collect(),
        Field::Created | Field::Updated => Vec::new(),
    }
}

fn cmp_value(f: &Facts, field: Field, v: &Value) -> Option<std::cmp::Ordering> {
    let Value::Text(raw) = v else { return None };
    match field {
        Field::Points => Some(f.points?.cmp(&raw.trim().parse::<i32>().ok()?)),
        Field::Due => Some(
            f.due?
                .cmp(&NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()?),
        ),
        _ => None,
    }
}

fn hit(f: &Facts, field: Field, v: &Value, me: uuid::Uuid) -> bool {
    match v {
        Value::CurrentUser => field == Field::Assignee && f.assignee_id == Some(me),
        Value::OpenSprints => field == Field::Sprint && f.sprint.is_some() && f.sprint_open,
        Value::Empty => text_values(f, field).is_empty(),
        Value::Text(t) => {
            let t = t.to_lowercase();
            text_values(f, field).contains(&t)
        }
    }
}

fn clause_matches(c: &Clause, f: &Facts, me: uuid::Uuid) -> bool {
    let v = &c.values[0];
    match c.op {
        Op::Eq => hit(f, c.field, v, me),
        Op::Ne => !hit(f, c.field, v, me),
        Op::In => c.values.iter().any(|v| hit(f, c.field, v, me)),
        Op::NotIn => !c.values.iter().any(|v| hit(f, c.field, v, me)),
        Op::Is => text_values(f, c.field).is_empty(),
        Op::IsNot => !text_values(f, c.field).is_empty(),
        Op::Like => match v {
            Value::Text(t) => {
                let t = t.to_lowercase();
                text_values(f, c.field).iter().any(|x| x.contains(&t))
            }
            _ => hit(f, c.field, v, me),
        },
        Op::Lt => cmp_value(f, c.field, v).is_some_and(std::cmp::Ordering::is_lt),
        Op::Le => cmp_value(f, c.field, v).is_some_and(std::cmp::Ordering::is_le),
        Op::Gt => cmp_value(f, c.field, v).is_some_and(std::cmp::Ordering::is_gt),
        Op::Ge => cmp_value(f, c.field, v).is_some_and(std::cmp::Ordering::is_ge),
    }
}

fn eval(e: &Expr, f: &Facts, me: uuid::Uuid) -> bool {
    match e {
        Expr::Clause(c) => clause_matches(c, f, me),
        Expr::Not(inner) => !eval(inner, f, me),
        Expr::And(parts) => parts.iter().all(|p| eval(p, f, me)),
        Expr::Or(parts) => parts.iter().any(|p| eval(p, f, me)),
    }
}

/// Apakah tiket memenuhi query.
#[must_use]
pub fn matches(q: &Query, f: &Facts, me: uuid::Uuid) -> bool {
    q.expr.as_ref().is_none_or(|e| eval(e, f, me))
}

// ---------------------------------------------------------------------------------------------
// Autocomplete

/// Yang diharapkan di posisi kursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Awal syarat: field, `NOT`, `(`.
    Field,
    /// Setelah field: operator untuk field itu.
    Operator(Field),
    /// Setelah `in`/`not in`: `(` atau fungsi.
    InStart(Field),
    /// Nilai field.
    Value(Field),
    /// Setelah `is`/`is not`: `EMPTY` (dan `NOT`).
    IsValue,
    /// Di dalam `in (…)` setelah nilai: `,` atau `)`.
    ListNext,
    /// Setelah syarat lengkap: `AND`, `OR`, `ORDER BY`, `)` bila di dalam kurung.
    Join { open: bool },
    /// Setelah `ORDER`.
    OrderBy,
    /// Setelah `ORDER BY`.
    OrderField,
    /// Setelah field urutan.
    OrderDir,
    /// Tidak ada saran (query tidak terbaca atau sudah lengkap).
    Nothing,
}

/// Hasil [`context`]: harapan + kata yang sedang diketik (akan diganti saran).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub expect: Expect,
    /// Teks persis di akhir prefix yang diganti saat saran dipilih (mis. `assig`, `"Spr`).
    pub partial: String,
}

#[derive(Debug, Clone, Copy)]
enum St {
    Start,
    AfterField(Field),
    AfterNot(Field),
    AfterIs,
    InOpen(Field),
    InList(Field),
    InListAfter(Field),
    AfterOp(Field),
    AfterClause,
    Order,
    OrderField,
    OrderDir,
    Done,
    Bad,
}

/// Baca `prefix` (teks sebelum kursor) untuk autocomplete.
#[must_use]
pub fn context(prefix: &str) -> Context {
    let Ok(mut toks) = tokenize(prefix, true) else {
        return Context {
            expect: Expect::Nothing,
            partial: String::new(),
        };
    };
    // Kata yang belum selesai diketik: token terakhir bila tidak diikuti spasi/tanda baca.
    let ends_word = prefix
        .chars()
        .last()
        .is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | ',' | '=' | '~' | '<' | '>'));
    let mut partial = String::new();
    if ends_word {
        match toks.last() {
            Some(Tok::Word(w)) => {
                partial.clone_from(w);
                toks.pop();
            }
            Some(Tok::Quoted(_)) => {
                // Kutip terbuka (`"Spr`) atau tertutup (`"Sprint 4"`): ganti seluruhnya, mulai dari
                // tanda kutip pembuka.
                let closed = prefix.ends_with(['"', '\'']);
                let body = if closed {
                    &prefix[..prefix.len() - 1]
                } else {
                    prefix
                };
                let start = body.rfind(['"', '\'']).unwrap_or_default();
                partial = prefix[start..].to_string();
                toks.pop();
            }
            _ => {}
        }
    }
    let mut st = St::Start;
    let mut depth = 0usize;
    for t in &toks {
        st = match (st, t) {
            (St::Start, Tok::Word(w)) if w.eq_ignore_ascii_case("not") => St::Start,
            (St::Start, Tok::Open) => {
                depth += 1;
                St::Start
            }
            (St::Start, Tok::Word(w)) if w.eq_ignore_ascii_case("order") => St::Order,
            (St::Start, Tok::Word(w)) => field_of(w).map_or(St::Bad, St::AfterField),
            (St::AfterField(f), Tok::Op(_)) => St::AfterOp(f),
            (St::AfterField(f), Tok::Word(w)) if w.eq_ignore_ascii_case("in") => St::InOpen(f),
            (St::AfterField(f), Tok::Word(w)) if w.eq_ignore_ascii_case("not") => St::AfterNot(f),
            (St::AfterField(_), Tok::Word(w)) if w.eq_ignore_ascii_case("is") => St::AfterIs,
            (St::AfterNot(f), Tok::Word(w)) if w.eq_ignore_ascii_case("in") => St::InOpen(f),
            (St::AfterIs, Tok::Word(w)) if w.eq_ignore_ascii_case("not") => St::AfterIs,
            (St::AfterIs, Tok::Word(w)) if w.eq_ignore_ascii_case("empty") => St::AfterClause,
            (St::InOpen(f), Tok::Open) => St::InList(f),
            (St::InOpen(_), Tok::Word(w)) if w.ends_with("()") => St::AfterClause,
            (St::InList(f), Tok::Word(_) | Tok::Quoted(_)) => St::InListAfter(f),
            (St::InListAfter(f), Tok::Comma) => St::InList(f),
            (St::InListAfter(_), Tok::Close) => St::AfterClause,
            (St::AfterOp(_), Tok::Word(_) | Tok::Quoted(_)) => St::AfterClause,
            (St::AfterClause, Tok::Word(w))
                if w.eq_ignore_ascii_case("and") || w.eq_ignore_ascii_case("or") =>
            {
                St::Start
            }
            (St::AfterClause, Tok::Close) if depth > 0 => {
                depth -= 1;
                St::AfterClause
            }
            (St::AfterClause, Tok::Word(w)) if w.eq_ignore_ascii_case("order") => St::Order,
            (St::Order, Tok::Word(w)) if w.eq_ignore_ascii_case("by") => St::OrderField,
            (St::OrderField, Tok::Word(w)) => field_of(w).map_or(St::Bad, |_| St::OrderDir),
            (St::OrderDir, Tok::Word(w))
                if w.eq_ignore_ascii_case("asc") || w.eq_ignore_ascii_case("desc") =>
            {
                St::Done
            }
            _ => St::Bad,
        };
        if matches!(st, St::Bad) {
            break;
        }
    }
    let expect = match st {
        St::Start => Expect::Field,
        St::AfterField(f) => Expect::Operator(f),
        St::AfterNot(f) | St::InOpen(f) => Expect::InStart(f),
        St::AfterIs => Expect::IsValue,
        St::InList(f) | St::AfterOp(f) => Expect::Value(f),
        St::InListAfter(_) => Expect::ListNext,
        St::AfterClause => Expect::Join { open: depth > 0 },
        St::Order => Expect::OrderBy,
        St::OrderField => Expect::OrderField,
        St::OrderDir => Expect::OrderDir,
        St::Done | St::Bad => Expect::Nothing,
    };
    Context { expect, partial }
}

/// Operator yang masuk akal untuk `field` (teks sisipan).
#[must_use]
pub fn operators(field: Field) -> &'static [&'static str] {
    if field == Field::Text {
        &["~", "is EMPTY", "is not EMPTY"]
    } else if field.ordered() {
        &["=", "!=", "<", "<=", ">", ">=", "is EMPTY", "is not EMPTY"]
    } else {
        &["=", "!=", "~", "in", "not in", "is EMPTY", "is not EMPTY"]
    }
}

/// Nilai siap sisip: dikutip bila berisi spasi atau tanda baca query.
#[must_use]
pub fn quote(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| !c.is_whitespace() && !BREAK.contains(&c))
        && !["and", "or", "not", "order", "in", "is", "empty", "null"]
            .iter()
            .any(|k| value.eq_ignore_ascii_case(k));
    if plain {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            project: "ksr".into(),
            stage: vec!["dev".into(), "development".into()],
            status: "in_progress".into(),
            assignee: vec!["olivia owner".into(), "olivia@x.id".into()],
            assignee_id: Some(uuid::Uuid::nil()),
            ticket_type: vec!["bug".into()],
            sprint: Some("sprint 4".into()),
            sprint_open: true,
            labels: vec!["pembayaran".into()],
            priority: "high".into(),
            points: Some(5),
            text: "ksr-1 bayar qris".into(),
            due: NaiveDate::from_ymd_opt(2026, 11, 20),
            ..Facts::default()
        }
    }

    fn run(q: &str) -> bool {
        matches(&parse(q).unwrap(), &facts(), uuid::Uuid::nil())
    }

    fn code(q: &str) -> &'static str {
        parse(q).unwrap_err().code
    }

    #[test]
    fn parses_and_matches_common_queries() {
        assert!(run(""));
        assert!(run("project = KSR AND assignee = currentUser()"));
        assert!(run("sprint in openSprints() AND stage = Development"));
        assert!(run("sprint = \"Sprint 4\" and type in (bug, story)"));
        assert!(run("text ~ qris AND points >= 5 AND due < 2026-12-01"));
        assert!(run("component is EMPTY AND label != urgent"));
        assert!(!run("assignee is EMPTY"));
        assert!(!run("priority not in (high, urgent)"));
        assert!(run("status = \"in progress\" ORDER BY priority DESC"));
        let q = parse("points > 1 order by due desc").unwrap();
        assert_eq!(q.order, Some((Field::Due, true)));
        let q = parse("ORDER BY created").unwrap();
        assert_eq!((q.expr, q.order), (None, Some((Field::Created, false))));
    }

    #[test]
    fn or_and_not_follow_precedence() {
        assert!(run("project = POS OR project = KSR"));
        assert!(!run("project = POS OR project = LMS"));
        // AND mengikat lebih kuat: POS OR (KSR AND low) → salah; (POS OR KSR) AND high → benar.
        assert!(!run("project = POS OR project = KSR AND priority = low"));
        assert!(run("(project = POS OR project = KSR) AND priority = high"));
        assert!(run("project = KSR AND priority = low OR points = 5"));
        // NOT mengikat paling kuat.
        assert!(!run("NOT project = KSR"));
        assert!(run("NOT project = KSR OR priority = high"));
        assert!(!run("NOT (project = KSR OR priority = low)"));
        assert!(run("not not project = ksr"));
        assert!(run("((project = KSR))"));
        assert!(run("label = pembayaran AND (NOT status = done) AND (type = bug OR type = story)"));
        let q = parse("project = A OR project = B AND label = c").unwrap();
        let Some(Expr::Or(parts)) = q.expr else {
            panic!("OR di puncak")
        };
        assert!(matches!(parts[1], Expr::And(_)));
    }

    #[test]
    fn reports_error_codes() {
        assert_eq!(code("warna = merah"), FIELD_UNKNOWN);
        assert_eq!(parse("warna = merah").unwrap_err().name, "warna");
        assert_eq!(code("project KSR"), OP_UNKNOWN);
        assert_eq!(code("label in (a, b"), PAREN_OPEN);
        assert_eq!(code("text ~ \"qris"), QUOTE_OPEN);
        assert_eq!(code("label < 3"), OP_COMPARE);
        assert_eq!(code("project = KSR ORDER BY due x"), ORDER_LAST);
        assert_eq!(code("project = KSR ORDER due"), ORDER_BY);
        assert_eq!(code("ORDER BY"), ORDER_FIELD);
        assert_eq!(code("project = KSR label = a"), JOIN);
        assert_eq!(code("project = KSR AND"), DANGLING);
        assert_eq!(code("project = KSR OR"), DANGLING);
        assert_eq!(code("NOT"), DANGLING);
        assert_eq!(code("(project = KSR"), PAREN_OPEN);
        assert_eq!(code("project = KSR)"), PAREN_CLOSE);
        assert_eq!(code("()"), EMPTY_GROUP);
        assert_eq!(code("label in ()"), EMPTY_GROUP);
        assert_eq!(code("label in a"), IN_PAREN);
        assert_eq!(code("label in (a b)"), IN_SEP);
        assert_eq!(code("label not (a)"), NOT_IN);
        assert_eq!(code("label is a"), IS_EMPTY);
        assert_eq!(code("project ! KSR"), BANG);
        assert_eq!(code("project = AND label = a"), VALUE_MISSING);
        assert_eq!(code("project ="), VALUE_MISSING);
        assert_eq!(code(", project = KSR"), CLAUSE_START);
        let many = vec!["label = a"; MAX_CLAUSES + 1].join(" OR ");
        assert_eq!(code(&many), TOO_MANY);
        let deep = format!("{}label = a{}", "(".repeat(12), ")".repeat(12));
        assert_eq!(code(&deep), TOO_DEEP);
    }

    #[test]
    fn errors_have_indonesian_messages() {
        let e = parse("warna = merah").unwrap_err();
        let m = e.message();
        assert!(m.contains("warna") && m.contains("tidak dikenal"), "{m}");
        let m = parse("project = KSR)").unwrap_err().message();
        assert!(m.contains("kurung"), "{m}");
    }

    fn ctx(prefix: &str) -> (Expect, String) {
        let c = context(prefix);
        (c.expect, c.partial)
    }

    #[test]
    fn context_tracks_what_comes_next() {
        assert_eq!(ctx(""), (Expect::Field, String::new()));
        assert_eq!(ctx("assig"), (Expect::Field, "assig".into()));
        assert_eq!(ctx("status "), (Expect::Operator(Field::Status), String::new()));
        assert_eq!(ctx("status = "), (Expect::Value(Field::Status), String::new()));
        assert_eq!(ctx("status=do"), (Expect::Value(Field::Status), "do".into()));
        assert_eq!(
            ctx("sprint = \"Spr"),
            (Expect::Value(Field::Sprint), "\"Spr".into())
        );
        assert_eq!(
            ctx("status = done "),
            (Expect::Join { open: false }, String::new())
        );
        assert_eq!(ctx("(status = done "), (Expect::Join { open: true }, String::new()));
        assert_eq!(ctx("(status = done) "), (Expect::Join { open: false }, String::new()));
        assert_eq!(ctx("status = done AND "), (Expect::Field, String::new()));
        assert_eq!(ctx("status = done or NOT ("), (Expect::Field, String::new()));
        assert_eq!(ctx("label in "), (Expect::InStart(Field::Label), String::new()));
        assert_eq!(ctx("label not in "), (Expect::InStart(Field::Label), String::new()));
        assert_eq!(ctx("label in (a, "), (Expect::Value(Field::Label), String::new()));
        assert_eq!(ctx("label in (a "), (Expect::ListNext, String::new()));
        assert_eq!(ctx("assignee is "), (Expect::IsValue, String::new()));
        assert_eq!(ctx("label = a ORDER "), (Expect::OrderBy, String::new()));
        assert_eq!(ctx("label = a ORDER BY "), (Expect::OrderField, String::new()));
        assert_eq!(ctx("label = a ORDER BY due "), (Expect::OrderDir, String::new()));
        assert_eq!(ctx("warna "), (Expect::Nothing, String::new()));
    }

    #[test]
    fn values_are_quoted_when_needed() {
        assert_eq!(quote("KSR"), "KSR");
        assert_eq!(quote("Sprint 4"), "\"Sprint 4\"");
        assert_eq!(quote("and"), "\"and\"");
        assert_eq!(quote("a(b)"), "\"a(b)\"");
        assert!(operators(Field::Points).contains(&"<="));
        assert!(!operators(Field::Label).contains(&"<"));
    }
}
