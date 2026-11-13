//! AI Office (M10): JSON keadaan kantor untuk kanvas, dan data panel samping (ringkasan dan
//! detail karakter). Teks antarmuka di template (Fluent); JSON hanya membawa data dan kunci.
use std::collections::HashMap;

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    models::{
        office::{Char, CharStatus, Office, RoomKind, Who},
        office_looks::{ACCESSORIES, HAIR_STYLES, Look},
        personas::PersonaLabels,
        users,
    },
    views::{
        palette,
        runner::{Age, age},
    },
};

/// Kunci Fluent pesan flash/validasi halaman ini.
pub const FLASH_LOOK_SAVED: &str = "office-look-saved";
pub const MESSAGE_KEYS: &[&str] = &[FLASH_LOOK_SAVED, "office-look-invalid"];

/// Kunci Fluent aksesori (`office-acc-<kunci>`) dan gaya rambut (`office-style-<kunci>`).
pub const ACC_KEYS: &[&str] = ACCESSORIES;
pub const STYLE_KEYS: &[&str] = HAIR_STYLES;

/// Warna kanvas dan varian komponen per status (sama dengan prototype `OST`).
#[must_use]
pub fn status_meta(s: CharStatus) -> (&'static str, &'static str) {
    match s {
        CharStatus::Working => ("#5b5bd6", "ai"),
        CharStatus::Review => ("#e3962b", "gate"),
        CharStatus::Decision => ("#e5484d", "destructive"),
        CharStatus::Meeting => ("#3b82f6", "info"),
        CharStatus::Idle | CharStatus::Paused => ("#a1a7b5", "neutral"),
        CharStatus::Online => ("#30a46c", "success"),
    }
}

/// Aksesori bawaan persona bawaan (prototype `LOOK`).
#[must_use]
pub fn default_acc(persona: &str) -> &'static str {
    match persona {
        "dev" => "headset",
        "qa" | "reviewer" => "glasses",
        "uiux" => "beret",
        "devops" => "cap",
        "writer" => "beanie",
        _ => "none",
    }
}

const SKINS: &[&str] = &["#f2c9a4", "#e3b089", "#c88d62", "#a8714a"];
const HAIRS: &[&str] = &["#231a16", "#6b3f28", "#3b2a20", "#8a5a3b"];
const STYLES: &[&str] = &["short", "bob", "long"];

fn pick<'a>(list: &'a [&'a str], seed: &str) -> &'a str {
    let h = seed.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    list[(h as usize) % list.len()]
}

/// Tampilan efektif (tersimpan, atau bawaan).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LookView {
    pub shirt: String,
    pub acc: String,
    pub skin: String,
    pub hair: String,
    pub style: String,
    pub alias: String,
}

fn or(v: &str, d: &str) -> String {
    if v.is_empty() {
        d.to_string()
    } else {
        v.to_string()
    }
}

#[must_use]
pub fn effective_look(
    who: &Who,
    saved: &Look,
    labels: &PersonaLabels,
    user: Option<&users::Model>,
) -> LookView {
    match who {
        Who::Persona(k) => LookView {
            shirt: or(
                &saved.shirt,
                labels.get(k).map_or("#5b5bd6", |l| l.color.as_str()),
            ),
            acc: or(&saved.acc, default_acc(k)),
            skin: String::new(),
            hair: String::new(),
            style: String::new(),
            alias: saved.alias.clone(),
        },
        Who::Human(u) => {
            let seed = u.to_string();
            let shirt = user.map_or_else(
                || palette::color_for(&seed),
                |u| palette::color_for(&u.email),
            );
            LookView {
                shirt: or(&saved.shirt, shirt),
                acc: or(&saved.acc, "none"),
                skin: or(&saved.skin, pick(SKINS, &seed)),
                hair: or(&saved.hair, pick(HAIRS, &seed)),
                style: or(&saved.style, pick(STYLES, &seed)),
                alias: saved.alias.clone(),
            }
        }
    }
}

/// Satu karakter untuk kanvas dan panel.
#[derive(Debug, Clone, Serialize)]
pub struct CharView {
    pub id: String,
    pub bot: bool,
    /// Kunci persona (agent) atau id user (manusia).
    pub key: String,
    /// Nama lengkap: nama persona (template menambah "Agent") atau nama user.
    pub name: String,
    /// Label pendek di atas kepala.
    pub short: String,
    pub icon: String,
    pub st: &'static str,
    pub color: &'static str,
    pub variant: &'static str,
    pub label_key: String,
    pub room: String,
    pub tk: String,
    pub title: String,
    pub ticket_href: String,
    pub job_href: String,
    pub runner: String,
    pub doing: String,
    pub step: usize,
    pub steps: usize,
    pub percent: usize,
    pub since: Option<Age>,
    pub jobs: Vec<String>,
    pub look: LookView,
    pub act: bool,
}

/// Ruangan untuk kanvas.
#[derive(Debug, Clone, Serialize)]
pub struct RoomView {
    pub k: String,
    pub kind: &'static str,
    pub n: String,
    pub c: String,
    pub col: usize,
    pub front: bool,
    pub href: String,
    /// Jumlah karakter di ruangan.
    pub count: usize,
    pub todo: usize,
}

/// Warna ruangan: warna project, atau warna palet dari kunci (sama dengan `projects::color_of`).
#[must_use]
pub fn room_color(r: &crate::models::office::Room) -> String {
    if r.color.is_empty() {
        palette::color_for(&r.key).to_string()
    } else {
        r.color.clone()
    }
}

/// Bahan tampilan di luar model kantor.
pub struct Ctx<'a> {
    pub slug: &'a str,
    pub labels: &'a PersonaLabels,
    pub users: &'a HashMap<Uuid, users::Model>,
}

/// Karakter siap tampil.
#[must_use]
pub fn char_view(office: &Office, c: &Char, ctx: &Ctx<'_>) -> CharView {
    let (color, variant) = status_meta(c.status);
    let user = match &c.who {
        Who::Human(u) => ctx.users.get(u),
        Who::Persona(_) => None,
    };
    let look = effective_look(&c.who, &office.look_of(&c.who), ctx.labels, user);
    let (key, name, icon, bot) = match &c.who {
        Who::Persona(k) => {
            let l = ctx.labels.get(k);
            (
                k.clone(),
                l.map_or_else(|| k.clone(), |l| l.name.clone()),
                l.map_or_else(|| "robot".to_string(), |l| l.icon.clone()),
                true,
            )
        }
        Who::Human(u) => (
            u.to_string(),
            user.map_or_else(String::new, |u| {
                if u.name.is_empty() {
                    u.email.clone()
                } else {
                    u.name.clone()
                }
            }),
            String::new(),
            false,
        ),
    };
    let short = if look.alias.is_empty() {
        if bot {
            name.clone()
        } else {
            name.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
        }
    } else {
        look.alias.clone()
    };
    let now = Utc::now();
    CharView {
        id: c.id.clone(),
        bot,
        key,
        name,
        short,
        icon,
        st: c.status.as_str(),
        color,
        variant,
        label_key: format!("office-st-{}", c.status.as_str()),
        room: c.room.clone(),
        tk: c.ticket_code.clone(),
        title: c.ticket_title.clone(),
        ticket_href: if c.ticket_id.is_some() {
            format!("/w/{}/t/{}", ctx.slug, c.ticket_code)
        } else {
            String::new()
        },
        job_href: c
            .job_id
            .map(|j| format!("/w/{}/jobs/{j}", ctx.slug))
            .unwrap_or_default(),
        runner: c.runner_name.clone(),
        doing: c.doing.clone(),
        step: c.step,
        steps: c.steps,
        percent: if c.steps == 0 {
            0
        } else {
            c.step * 100 / c.steps
        },
        since: c.since.map(|t| age(&t.fixed_offset(), now)),
        jobs: c.jobs.clone(),
        look,
        act: c.status.needs_action(),
    }
}

/// Seluruh keadaan kantor untuk kanvas (`GET /w/{slug}/office/state`).
#[must_use]
pub fn state_view(office: &Office, ctx: &Ctx<'_>) -> serde_json::Value {
    let chars: Vec<CharView> = office
        .chars
        .iter()
        .map(|c| char_view(office, c, ctx))
        .collect();
    let rooms: Vec<RoomView> = office
        .rooms
        .iter()
        .map(|r| RoomView {
            k: r.key.clone(),
            kind: r.kind.as_str(),
            n: r.name.clone(),
            c: room_color(r),
            col: r.col,
            front: r.front,
            href: if r.kind == RoomKind::Project {
                format!("/w/{}/p/{}", ctx.slug, r.project_key)
            } else {
                String::new()
            },
            count: office.chars.iter().filter(|c| c.room == r.key).count(),
            todo: office
                .chars
                .iter()
                .filter(|c| c.room == r.key && c.status.needs_action())
                .count(),
        })
        .collect();
    let counts: HashMap<&str, usize> = CharStatus::ALL
        .iter()
        .map(|s| (s.as_str(), office.count(*s)))
        .collect();
    serde_json::json!({
        "rooms": rooms,
        "chars": chars,
        "counts": counts,
        "hidden": office.hidden_projects,
    })
}

/// Baris runner di panel ringkasan.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerRow {
    pub name: String,
    pub client: String,
    pub owner: Option<crate::views::tickets::PersonView>,
    /// `busy` | `online` | `offline`.
    pub state: &'static str,
    pub jobs: Vec<String>,
    pub last_seen: Option<Age>,
    /// Karakter pemilik di kantor (pilih karakter).
    pub char_id: Option<String>,
}

#[must_use]
pub fn runner_rows(office: &Office, users: &HashMap<Uuid, users::Model>) -> Vec<RunnerRow> {
    let now = Utc::now();
    office
        .runners
        .iter()
        .map(|r| RunnerRow {
            name: r.name.clone(),
            client: r.client.clone(),
            owner: users
                .get(&r.owner)
                .map(crate::views::tickets::PersonView::of),
            state: if !r.online {
                "offline"
            } else if r.busy {
                "busy"
            } else {
                "online"
            },
            jobs: r.jobs.iter().take(3).cloned().collect(),
            last_seen: r.last_seen.map(|t| age(&t.fixed_offset(), now)),
            char_id: office
                .chars
                .iter()
                .find(|c| c.who == Who::Human(r.owner))
                .map(|c| c.id.clone()),
        })
        .collect()
}

/// Pilihan aksesori/gaya untuk form kustom.
#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub key: &'static str,
    pub label_key: String,
    pub on: bool,
}

#[must_use]
pub fn choices(prefix: &str, keys: &[&'static str], current: &str) -> Vec<Choice> {
    keys.iter()
        .map(|k| Choice {
            key: k,
            label_key: format!("{prefix}-{k}"),
            on: *k == current,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::personas::PersonaLabel;

    #[test]
    fn defaults_follow_persona_and_saved_look_wins() {
        let mut labels = PersonaLabels::new();
        labels.insert(
            "dev".into(),
            PersonaLabel {
                key: "dev".into(),
                name: "Developer".into(),
                icon: "code".into(),
                color: "#5b5bd6".into(),
            },
        );
        let who = Who::Persona("dev".into());
        let d = effective_look(&who, &Look::default(), &labels, None);
        assert_eq!((d.shirt.as_str(), d.acc.as_str()), ("#5b5bd6", "headset"));
        let saved = Look {
            shirt: "#0d9488".into(),
            acc: "cap".into(),
            ..Look::default()
        };
        let s = effective_look(&who, &saved, &labels, None);
        assert_eq!((s.shirt.as_str(), s.acc.as_str()), ("#0d9488", "cap"));
        let human = effective_look(&Who::Human(Uuid::nil()), &Look::default(), &labels, None);
        assert!(SKINS.contains(&human.skin.as_str()));
        assert!(STYLES.contains(&human.style.as_str()));
    }
}
