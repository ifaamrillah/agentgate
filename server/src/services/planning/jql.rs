//! Query ala JQL (pencarian lanjutan, saved filter).
//!
//! Tata bahasa: `syarat (AND syarat)* [ORDER BY field [ASC|DESC]]`, dengan
//! `syarat = field op nilai`. Operator: `=`, `!=`, `~` (mengandung), `in (a, b)`, `not in (…)`,
//! `is EMPTY`, `is not EMPTY`, dan `<`, `<=`, `>`, `>=` untuk `points`/`due`. Nilai: kata,
//! `"teks berspasi"`, `currentUser()`, `openSprints()`, `EMPTY`. Kata kunci tidak peka huruf
//! besar/kecil; perbandingan nilai juga.
//!
//! Evaluasi berjalan di memori atas [`Facts`] tiap tiket (lihat controller pencarian lanjutan).
use chrono::NaiveDate;

/// Batas syarat per query.
pub const MAX_CLAUSES: usize = 20;

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

/// Field yang dikenal: (nama di query, field, keterangan untuk bantuan).
pub const FIELDS: &[(&str, Field, &str)] = &[
    ("project", Field::Project, "Kunci project, mis. KSR"),
    ("stage", Field::Stage, "Kunci atau nama stage workflow"),
    ("status", Field::Status, "backlog, todo, in_progress, done"),
    (
        "assignee",
        Field::Assignee,
        "Nama/email penerima, currentUser(), EMPTY",
    ),
    ("type", Field::Type, "Kunci atau nama tipe tiket"),
    ("sprint", Field::Sprint, "Nama sprint, openSprints(), EMPTY"),
    ("label", Field::Label, "Label tiket"),
    ("priority", Field::Priority, "low, medium, high, urgent"),
    ("points", Field::Points, "Story point; bisa <, >, EMPTY"),
    ("text", Field::Text, "Judul dan deskripsi (pakai ~)"),
    ("fixversion", Field::Release, "Nama rilis (Fix version)"),
    ("component", Field::Component, "Nama component"),
    ("due", Field::Due, "Tenggat YYYY-MM-DD; bisa <, >, EMPTY"),
    ("created", Field::Created, "Tanggal dibuat (ORDER BY)"),
    ("updated", Field::Updated, "Tanggal diubah (ORDER BY)"),
];

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    pub clauses: Vec<Clause>,
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

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
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
            '"' | '\'' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|x| *x == c)
                    .ok_or_else(|| "Tanda kutip belum ditutup.".to_string())?;
                out.push(Tok::Quoted(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
            }
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
                        _ => return Err("Operator “!” harus ditulis “!=”.".into()),
                    },
                };
                i += op.len();
                out.push(Tok::Op(op));
            }
            _ => {
                let start = i;
                while i < chars.len()
                    && !chars[i].is_whitespace()
                    && !matches!(
                        chars[i],
                        '(' | ')' | ',' | '"' | '\'' | '!' | '<' | '>' | '=' | '~'
                    )
                {
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

fn field_of(name: &str) -> Result<Field, String> {
    let n = name.to_lowercase();
    let n = match n.as_str() {
        "fixversion" | "fix_version" | "release" | "versi" => "fixversion",
        "key" | "kode" => "text",
        "summary" | "judul" => "text",
        other => other,
    };
    FIELDS
        .iter()
        .find(|(k, ..)| *k == n)
        .map(|(_, f, _)| *f)
        .ok_or_else(|| {
            format!(
                "Field “{name}” tidak dikenal. Coba: {}.",
                FIELDS[..6]
                    .iter()
                    .map(|f| f.0)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn value_of(tok: &Tok) -> Result<Value, String> {
    match tok {
        Tok::Quoted(s) => Ok(Value::Text(s.clone())),
        Tok::Word(w) => Ok(match w.to_lowercase().as_str() {
            "currentuser()" => Value::CurrentUser,
            "opensprints()" => Value::OpenSprints,
            "empty" | "null" => Value::Empty,
            _ => Value::Text(w.clone()),
        }),
        _ => Err("Nilai diharapkan setelah operator.".into()),
    }
}

fn is_kw(t: Option<&Tok>, kw: &str) -> bool {
    matches!(t, Some(Tok::Word(w)) if w.eq_ignore_ascii_case(kw))
}

/// Parse query. Query kosong = semua tiket.
///
/// # Errors
///
/// Pesan dalam bahasa Indonesia yang bisa ditampilkan ke pengguna.
pub fn parse(src: &str) -> Result<Query, String> {
    let toks = tokenize(src)?;
    let mut q = Query::default();
    let mut i = 0;
    let next = |i: &mut usize| {
        let t = toks.get(*i).cloned();
        *i += 1;
        t
    };
    while i < toks.len() {
        if is_kw(toks.get(i), "order") {
            i += 1;
            if !is_kw(toks.get(i), "by") {
                return Err("Tulis “ORDER BY field”.".into());
            }
            i += 1;
            let Some(Tok::Word(f)) = next(&mut i) else {
                return Err("Field urutan belum ditulis setelah ORDER BY.".into());
            };
            let field = field_of(&f)?;
            let desc = if is_kw(toks.get(i), "desc") {
                i += 1;
                true
            } else {
                if is_kw(toks.get(i), "asc") {
                    i += 1;
                }
                false
            };
            q.order = Some((field, desc));
            if i < toks.len() {
                return Err("ORDER BY harus di akhir query.".into());
            }
            break;
        }
        if !q.clauses.is_empty() {
            if is_kw(toks.get(i), "and") {
                i += 1;
            } else if is_kw(toks.get(i), "or") {
                return Err(
                    "OR belum didukung. Gabungkan syarat dengan AND, atau pakai in (…).".into(),
                );
            } else {
                return Err("Gabungkan syarat dengan AND.".into());
            }
        }
        let Some(Tok::Word(name)) = next(&mut i) else {
            return Err("Syarat harus diawali nama field, mis. assignee = currentUser().".into());
        };
        let field = field_of(&name)?;
        let op = match next(&mut i) {
            Some(Tok::Op("=")) => Op::Eq,
            Some(Tok::Op("!=")) => Op::Ne,
            Some(Tok::Op("~")) => Op::Like,
            Some(Tok::Op("<")) => Op::Lt,
            Some(Tok::Op("<=")) => Op::Le,
            Some(Tok::Op(">")) => Op::Gt,
            Some(Tok::Op(">=")) => Op::Ge,
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("in") => Op::In,
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("not") => {
                if is_kw(toks.get(i), "in") {
                    i += 1;
                    Op::NotIn
                } else {
                    return Err("Tulis “not in (…)”.".into());
                }
            }
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("is") => {
                if is_kw(toks.get(i), "not") {
                    i += 1;
                    Op::IsNot
                } else {
                    Op::Is
                }
            }
            _ => {
                return Err(format!(
                    "Operator untuk “{name}” tidak dikenal. Pakai =, !=, ~, in, not in, is EMPTY."
                ));
            }
        };
        if matches!(op, Op::Lt | Op::Le | Op::Gt | Op::Ge)
            && !matches!(
                field,
                Field::Points | Field::Due | Field::Created | Field::Updated
            )
        {
            return Err(format!(
                "Operator <, > hanya untuk points dan due, bukan “{name}”."
            ));
        }
        let func = matches!(toks.get(i), Some(Tok::Word(w)) if w.ends_with("()"));
        let values = if matches!(op, Op::In | Op::NotIn) && func {
            // `sprint in openSprints()`, `assignee in currentUser()`.
            let t = next(&mut i).ok_or_else(|| format!("Nilai untuk “{name}” belum ditulis."))?;
            vec![value_of(&t)?]
        } else if matches!(op, Op::In | Op::NotIn) {
            if next(&mut i) != Some(Tok::Open) {
                return Err("Nilai in harus di dalam kurung, mis. in (a, b).".into());
            }
            let mut vals = Vec::new();
            loop {
                let t = next(&mut i).ok_or_else(|| "Kurung belum ditutup.".to_string())?;
                if t == Tok::Close && !vals.is_empty() {
                    break;
                }
                vals.push(value_of(&t)?);
                match next(&mut i) {
                    Some(Tok::Comma) => {}
                    Some(Tok::Close) => break,
                    None => return Err("Kurung belum ditutup.".into()),
                    _ => return Err("Pisahkan nilai in dengan koma dan tutup dengan “)”.".into()),
                }
            }
            vals
        } else {
            let t = next(&mut i).ok_or_else(|| format!("Nilai untuk “{name}” belum ditulis."))?;
            vec![value_of(&t)?]
        };
        if matches!(op, Op::Is | Op::IsNot) && values != [Value::Empty] {
            return Err("Operator is hanya untuk EMPTY, mis. assignee is EMPTY.".into());
        }
        q.clauses.push(Clause { field, op, values });
        if q.clauses.len() > MAX_CLAUSES {
            return Err(format!("Maksimal {MAX_CLAUSES} syarat per query."));
        }
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

/// Apakah tiket memenuhi semua syarat.
#[must_use]
pub fn matches(q: &Query, f: &Facts, me: uuid::Uuid) -> bool {
    q.clauses.iter().all(|c| {
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
    })
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
    }

    #[test]
    fn reports_readable_errors() {
        assert!(
            parse("warna = merah")
                .unwrap_err()
                .contains("tidak dikenal")
        );
        assert!(
            parse("project = KSR OR project = POS")
                .unwrap_err()
                .contains("OR")
        );
        assert!(parse("project KSR").unwrap_err().contains("Operator"));
        assert!(parse("label in (a, b").unwrap_err().contains("Kurung"));
        assert!(parse("text ~ \"qris").unwrap_err().contains("kutip"));
        assert!(parse("label < 3").unwrap_err().contains("points"));
        assert!(parse("project = KSR ORDER BY due x").is_err());
    }
}
