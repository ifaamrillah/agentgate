//! Keadaan AI Office (M10) sebagai data murni: ruangan dan karakter yang dihitung
//! `services::office::state` dari job, tiket, izin, dan runner. Tidak ada tabel sendiri (selain
//! `office_looks`); dipakai bersama service (mengisi) dan view (menyajikan JSON/panel).
use uuid::Uuid;

use super::office_looks::Look;

/// Status karakter (prototype: `OST`). Kunci string dipakai JSON, kelas CSS, dan kunci Fluent
/// `office-st-<kunci>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharStatus {
    /// Job `auto` claimed/running.
    Working,
    /// Tiket menunggu review di stage job ini, atau izin aksi menunggu jawaban.
    Review,
    /// Tiket butuh keputusan (batas gagal).
    Decision,
    /// Job `interactive` berjalan (Ruang rapat).
    Meeting,
    /// Persona tanpa pekerjaan (Pantry).
    Idle,
    /// Manusia pemilik runner yang online.
    Online,
    /// Job `paused`/`quota`.
    Paused,
}

impl CharStatus {
    pub const ALL: &'static [Self] = &[
        Self::Working,
        Self::Review,
        Self::Decision,
        Self::Meeting,
        Self::Idle,
        Self::Online,
        Self::Paused,
    ];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Review => "review",
            Self::Decision => "decision",
            Self::Meeting => "meeting",
            Self::Idle => "idle",
            Self::Online => "online",
            Self::Paused => "paused",
        }
    }

    /// Butuh tindakan manusia (daftar "Perlu tindakan Anda").
    #[must_use]
    pub fn needs_action(self) -> bool {
        matches!(self, Self::Review | Self::Decision)
    }
}

/// Jenis ruangan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    Project,
    Meeting,
    Pantry,
    /// Slot depan kosong (dekor).
    Lounge,
}

impl RoomKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Meeting => "meeting",
            Self::Pantry => "pantry",
            Self::Lounge => "lounge",
        }
    }
}

/// Kunci ruangan non-project.
pub const MEET: &str = "MEET";
pub const PANTRY: &str = "PANTRY";
pub const LOUNGE: &str = "LOUNGE";

/// Satu ruangan di denah. Project: kolom `col` (lebar 17 ubin), `front` = baris depan.
#[derive(Debug, Clone)]
pub struct Room {
    pub key: String,
    pub kind: RoomKind,
    pub name: String,
    pub color: String,
    pub col: usize,
    pub front: bool,
    /// Path project (tautan), kosong untuk ruangan umum.
    pub project_key: String,
}

/// Siapa karakter ini.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Who {
    /// Persona (kunci).
    Persona(String),
    /// Manusia (id user).
    Human(Uuid),
}

/// Satu karakter di kantor.
#[derive(Debug, Clone)]
pub struct Char {
    /// Id stabil: `j-<job>`, `t-<tiket>`, `p-<persona>`, `u-<user>`.
    pub id: String,
    pub who: Who,
    pub status: CharStatus,
    /// Kunci ruangan tempat karakter duduk.
    pub room: String,
    pub job_id: Option<Uuid>,
    pub ticket_id: Option<Uuid>,
    pub ticket_code: String,
    pub ticket_title: String,
    pub stage_key: String,
    pub project_key: String,
    pub runner_id: Option<Uuid>,
    pub runner_name: String,
    /// Teks "sedang apa" (langkah berjalan / izin yang diminta), data apa adanya.
    pub doing: String,
    pub step: usize,
    pub steps: usize,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    /// Manusia: kode tiket job yang ia awasi.
    pub jobs: Vec<String>,
    /// Izin aksi menunggu jawaban (id) bila status Review karena izin.
    pub permission_id: Option<Uuid>,
}

impl Char {
    #[must_use]
    pub fn persona_key(&self) -> Option<&str> {
        match &self.who {
            Who::Persona(k) => Some(k),
            Who::Human(_) => None,
        }
    }
}

/// Runner di kantor (panel ringkasan).
#[derive(Debug, Clone)]
pub struct OfficeRunner {
    pub id: Uuid,
    pub name: String,
    pub client: String,
    pub owner: Uuid,
    pub online: bool,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
    pub jobs: Vec<String>,
    pub busy: bool,
}

/// Keadaan kantor satu workspace untuk satu user.
#[derive(Debug, Clone, Default)]
pub struct Office {
    pub rooms: Vec<Room>,
    pub chars: Vec<Char>,
    pub runners: Vec<OfficeRunner>,
    /// Tampilan karakter per (`persona`/`user`, kunci).
    pub looks: Vec<(String, String, Look)>,
    /// Project yang tidak muat di denah (lebih dari [`ROOMS_MAX`]).
    pub hidden_projects: usize,
}

/// Maks. ruangan project di denah (4 kolom × 2).
pub const ROOMS_MAX: usize = 8;

impl Office {
    #[must_use]
    pub fn char(&self, id: &str) -> Option<&Char> {
        self.chars.iter().find(|c| c.id == id)
    }

    #[must_use]
    pub fn count(&self, status: CharStatus) -> usize {
        self.chars
            .iter()
            .filter(|c| c.status == status && matches!(c.who, Who::Persona(_)))
            .count()
    }

    /// Tampilan tersimpan untuk persona/manusia.
    #[must_use]
    pub fn look_of(&self, who: &Who) -> Look {
        let (subject, key) = match who {
            Who::Persona(k) => ("persona", k.clone()),
            Who::Human(u) => ("user", u.to_string()),
        };
        self.looks
            .iter()
            .find(|(s, k, _)| s == subject && *k == key)
            .map(|(_, _, l)| l.clone())
            .unwrap_or_default()
    }
}

/// Susun ruangan: project paling ramai dulu, dua per kolom (atas lalu depan), kolom terakhir
/// Ruang rapat + Pantry, slot depan kosong jadi Lounge. `projects` = (kunci, nama, warna, ramai).
#[must_use]
pub fn plan_rooms(mut projects: Vec<(String, String, String, usize)>) -> (Vec<Room>, usize) {
    projects.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.1.cmp(&b.1)));
    let hidden = projects.len().saturating_sub(ROOMS_MAX);
    projects.truncate(ROOMS_MAX);
    let mut rooms: Vec<Room> = projects
        .into_iter()
        .enumerate()
        .map(|(i, (key, name, color, _))| Room {
            project_key: key.clone(),
            key,
            kind: RoomKind::Project,
            name,
            color,
            col: i / 2,
            front: i % 2 == 1,
        })
        .collect();
    let cols = rooms.len().div_ceil(2).max(1);
    if rooms.len() % 2 == 1 || rooms.is_empty() {
        rooms.push(Room {
            key: LOUNGE.into(),
            kind: RoomKind::Lounge,
            name: String::new(),
            color: "#a16207".into(),
            col: cols - 1,
            front: !rooms.is_empty(),
            project_key: String::new(),
        });
        if rooms.len() == 1 {
            // Tanpa project: ruangan atas juga lounge supaya denah tetap utuh.
            rooms.push(Room {
                key: format!("{LOUNGE}2"),
                kind: RoomKind::Lounge,
                name: String::new(),
                color: "#a16207".into(),
                col: 0,
                front: true,
                project_key: String::new(),
            });
        }
    }
    rooms.push(Room {
        key: MEET.into(),
        kind: RoomKind::Meeting,
        name: String::new(),
        color: "#0891b2".into(),
        col: cols,
        front: false,
        project_key: String::new(),
    });
    rooms.push(Room {
        key: PANTRY.into(),
        kind: RoomKind::Pantry,
        name: String::new(),
        color: "#a16207".into(),
        col: cols,
        front: true,
        project_key: String::new(),
    });
    (rooms, hidden)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(k: &str, n: usize) -> (String, String, String, usize) {
        (k.into(), format!("Project {k}"), "#7c3aed".into(), n)
    }

    #[test]
    fn rooms_pair_busiest_projects_and_fill_lounge() {
        let (rooms, hidden) = plan_rooms(vec![p("A", 0), p("B", 5), p("C", 2)]);
        assert_eq!(hidden, 0);
        let keys: Vec<(&str, usize, bool)> = rooms
            .iter()
            .map(|r| (r.key.as_str(), r.col, r.front))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("B", 0, false),
                ("C", 0, true),
                ("A", 1, false),
                (LOUNGE, 1, true),
                (MEET, 2, false),
                (PANTRY, 2, true),
            ]
        );
    }

    #[test]
    fn at_most_eight_project_rooms() {
        let many: Vec<_> = (0..11).map(|i| p(&format!("P{i}"), i)).collect();
        let (rooms, hidden) = plan_rooms(many);
        assert_eq!(hidden, 3);
        assert_eq!(
            rooms.iter().filter(|r| r.kind == RoomKind::Project).count(),
            8
        );
        assert_eq!(rooms.last().unwrap().col, 4);
    }

    #[test]
    fn empty_office_still_has_a_full_plan() {
        let (rooms, _) = plan_rooms(vec![]);
        assert_eq!(rooms.len(), 4);
        assert!(rooms.iter().any(|r| r.key == MEET));
    }
}
