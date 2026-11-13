//! Halaman tiket (M2): board, daftar, form tiket, detail dan aktivitas, sampah, Tiket saya,
//! kotak masuk, pencarian, dan pengaturan tipe/field/kode tiket.
//!
//! Struct di sini berisi nilai polos untuk template. Data yang dirujuk tiket (tipe, orang,
//! project) dimuat controller sekali per halaman lalu dirangkai lewat [`Lookup`].

use std::collections::HashMap;

use axum::http::StatusCode;
use chrono::NaiveDate;
use loco_rs::{prelude::*, validation::ModelValidationErrors};
use serde::Serialize;
use uuid::Uuid;

use super::{
    datatable::{ColumnSpec, FilterSpec, TableSpec},
    form::{FieldErrors, field_errors},
    layout::{AppShell, app_page},
    palette,
    time::wib_datetime,
};
use crate::models::{
    custom_fields::{self, FieldKind},
    ticket_types,
    tickets::{self, TicketPriority, TicketStatus},
    users,
};

pub const FLASH_TRASHED: &str = "flash-ticket-trashed";
pub const FLASH_RESTORED: &str = "flash-ticket-restored";
pub const FLASH_PURGED: &str = "flash-ticket-purged";
pub const FLASH_SAVED: &str = "flash-ticket-saved";
pub const FLASH_TYPE_SAVED: &str = "flash-ticket-type-saved";
pub const FLASH_TYPE_DELETED: &str = "flash-ticket-type-deleted";
pub const FLASH_FIELD_SAVED: &str = "flash-custom-field-saved";
pub const FLASH_FIELD_DELETED: &str = "flash-custom-field-deleted";
pub const FLASH_CODE_SAVED: &str = "flash-ticket-code-saved";
/// Kunci Fluent yang dikirim sebagai variabel ke template (dijaga tes i18n).
pub const MESSAGE_KEYS: &[&str] = &[
    FLASH_TRASHED,
    FLASH_RESTORED,
    FLASH_PURGED,
    FLASH_SAVED,
    FLASH_TYPE_SAVED,
    FLASH_TYPE_DELETED,
    FLASH_FIELD_SAVED,
    FLASH_FIELD_DELETED,
    FLASH_CODE_SAVED,
];

/// Field form tiket (error per field).
pub const TICKET_FIELDS: &[&str] = &[
    "title",
    "description",
    "type_id",
    "status",
    "priority",
    "assignee",
    "parent",
    "labels",
    "due_on",
    "story_points",
];
pub const TYPE_FIELDS: &[&str] = &["key", "name", "description", "icon", "color", "type"];
pub const FIELD_FIELDS: &[&str] = &["name", "kind", "options"];
pub const CODE_FIELDS: &[&str] = &["template", "scope", "team_code"];

/// Render halaman ber-layout aplikasi.
///
/// # Errors
///
/// Error render template.
pub fn render<T: Serialize>(
    v: &impl ViewRenderer,
    shell: &AppShell,
    template: &str,
    status: StatusCode,
    page: T,
) -> Result<Response> {
    format::render()
        .status(status)
        .view(v, template, app_page(shell, page))
}

// ---------------------------------------------------------------------------------------------
// Path

/// Path halaman satu tiket (`/w/{slug}/t/{code}`) dan aksinya.
#[derive(Debug, Clone, Serialize)]
pub struct TicketPaths {
    pub show: String,
    pub activity: String,
    pub status: String,
    pub comments: String,
    pub links: String,
    pub attachments: String,
    pub watch: String,
    pub delete: String,
    /// Aksi stage (M3).
    pub complete: String,
    pub approve: String,
    pub reject: String,
    pub decide: String,
    pub admin_move: String,
    pub people: String,
    pub artifacts: String,
}

impl TicketPaths {
    #[must_use]
    pub fn new(slug: &str, code: &str) -> Self {
        let show = ticket_href(slug, code);
        Self {
            activity: format!("{show}/activity"),
            status: format!("{show}/status"),
            comments: format!("{show}/comments"),
            links: format!("{show}/links"),
            attachments: format!("{show}/attachments"),
            watch: format!("{show}/watch"),
            delete: format!("{show}/delete"),
            complete: format!("{show}/stage/complete"),
            approve: format!("{show}/stage/approve"),
            reject: format!("{show}/stage/reject"),
            decide: format!("{show}/stage/decide"),
            admin_move: format!("{show}/stage/admin-move"),
            people: format!("{show}/stage/people"),
            artifacts: format!("{show}/artifacts"),
            show,
        }
    }
}

#[must_use]
pub fn ticket_href(slug: &str, code: &str) -> String {
    format!("/w/{slug}/t/{code}")
}

/// Path tiket di dalam project (`/w/{slug}/p/{key}/...`).
#[derive(Debug, Clone, Serialize)]
pub struct BoardPaths {
    pub board: String,
    pub list: String,
    pub new_ticket: String,
    pub trash: String,
    pub people: String,
}

impl BoardPaths {
    #[must_use]
    pub fn new(home: &str) -> Self {
        Self {
            board: format!("{home}/board"),
            list: format!("{home}/list"),
            new_ticket: format!("{home}/tickets/new"),
            trash: format!("{home}/trash"),
            people: format!("{home}/people"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Nilai tampil

/// Status (kolom board) untuk tampilan.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct StatusView {
    pub value: &'static str,
    pub label_key: &'static str,
    pub icon: &'static str,
    /// Varian `badge`.
    pub variant: &'static str,
}

/// Urutan kolom board.
pub const STATUSES: [TicketStatus; 4] = [
    TicketStatus::Backlog,
    TicketStatus::Todo,
    TicketStatus::InProgress,
    TicketStatus::Done,
];

#[must_use]
pub fn status_view(status: TicketStatus) -> StatusView {
    match status {
        TicketStatus::Backlog => StatusView {
            value: "backlog",
            label_key: "ticket-status-backlog",
            icon: "circle-dashed",
            variant: "neutral",
        },
        TicketStatus::Todo => StatusView {
            value: "todo",
            label_key: "ticket-status-todo",
            icon: "circle",
            variant: "info",
        },
        TicketStatus::InProgress => StatusView {
            value: "in_progress",
            label_key: "ticket-status-in-progress",
            icon: "circle-half-2",
            variant: "primary",
        },
        TicketStatus::Done => StatusView {
            value: "done",
            label_key: "ticket-status-done",
            icon: "circle-check",
            variant: "success",
        },
    }
}

/// Semua status untuk pilihan form.
#[must_use]
pub fn status_options() -> Vec<StatusView> {
    STATUSES.iter().map(|s| status_view(*s)).collect()
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PriorityView {
    pub value: &'static str,
    /// 1–4 (P1 paling mendesak).
    pub rank: u8,
    pub label_key: &'static str,
}

#[must_use]
pub fn priority_view(priority: TicketPriority) -> PriorityView {
    let (value, label_key) = match priority {
        TicketPriority::Urgent => ("urgent", "ticket-priority-urgent"),
        TicketPriority::High => ("high", "ticket-priority-high"),
        TicketPriority::Medium => ("medium", "ticket-priority-medium"),
        TicketPriority::Low => ("low", "ticket-priority-low"),
    };
    PriorityView {
        value,
        rank: priority.rank(),
        label_key,
    }
}

#[must_use]
pub fn priority_options() -> Vec<PriorityView> {
    [
        TicketPriority::Urgent,
        TicketPriority::High,
        TicketPriority::Medium,
        TicketPriority::Low,
    ]
    .into_iter()
    .map(priority_view)
    .collect()
}

/// Orang (penerima tugas, pelapor, penulis komentar).
#[derive(Debug, Clone, Serialize)]
pub struct PersonView {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub initials: String,
    pub color: &'static str,
}

impl PersonView {
    #[must_use]
    pub fn of(user: &users::Model) -> Self {
        Self {
            id: user.id,
            name: if user.name.is_empty() {
                user.email.clone()
            } else {
                user.name.clone()
            },
            email: user.email.clone(),
            initials: palette::initials(&user.name, &user.email),
            color: palette::color_for(&user.email),
        }
    }
}

/// Tipe tiket untuk ikon dan pilihan.
#[derive(Debug, Clone, Serialize)]
pub struct TypeView {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub builtin: bool,
}

impl TypeView {
    #[must_use]
    pub fn of(t: &ticket_types::Model) -> Self {
        Self {
            id: t.id,
            key: t.key.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            icon: t.icon.clone(),
            color: t.color.clone(),
            builtin: t.builtin,
        }
    }

    fn unknown() -> Self {
        Self {
            id: Uuid::nil(),
            key: String::new(),
            name: String::new(),
            description: String::new(),
            icon: "square-check".into(),
            color: "#64748b".into(),
            builtin: false,
        }
    }
}

/// Data rujukan tiket yang dimuat sekali per halaman.
#[derive(Debug, Default)]
pub struct Lookup {
    pub slug: String,
    pub types: HashMap<Uuid, TypeView>,
    pub people: HashMap<Uuid, PersonView>,
    /// id project → (nama, key).
    pub projects: HashMap<Uuid, (String, String)>,
    pub today: Option<NaiveDate>,
}

impl Lookup {
    #[must_use]
    pub fn new(slug: &str, types: &[ticket_types::Model], people: &[users::Model]) -> Self {
        Self {
            slug: slug.to_string(),
            types: types.iter().map(|t| (t.id, TypeView::of(t))).collect(),
            people: people.iter().map(|u| (u.id, PersonView::of(u))).collect(),
            projects: HashMap::new(),
            today: Some(chrono::Utc::now().date_naive()),
        }
    }

    #[must_use]
    pub fn with_projects<'a>(
        mut self,
        projects: impl IntoIterator<Item = (Uuid, &'a str, &'a str)>,
    ) -> Self {
        self.projects = projects
            .into_iter()
            .map(|(id, name, key)| (id, (name.to_string(), key.to_string())))
            .collect();
        self
    }

    #[must_use]
    pub fn person(&self, id: Option<Uuid>) -> Option<PersonView> {
        id.and_then(|id| self.people.get(&id).cloned())
    }

    #[must_use]
    pub fn ticket_type(&self, id: Uuid) -> TypeView {
        self.types
            .get(&id)
            .cloned()
            .unwrap_or_else(TypeView::unknown)
    }

    /// Ringkasan tiket untuk kartu board dan baris tabel.
    #[must_use]
    pub fn row(&self, t: &tickets::Model) -> TicketRow {
        let status = t.status().unwrap_or(TicketStatus::Todo);
        let paths = TicketPaths::new(&self.slug, &t.code);
        TicketRow {
            id: t.id,
            code: t.code.clone(),
            title: t.title.clone(),
            href: paths.show,
            status_action: paths.status,
            ticket_type: self.ticket_type(t.type_id),
            status: status_view(status),
            priority: priority_view(t.priority().unwrap_or(TicketPriority::Medium)),
            labels: t.labels.clone(),
            points: t.story_points,
            due: t.due_on.map(format_date).unwrap_or_default(),
            overdue: status != TicketStatus::Done
                && t.due_on
                    .zip(self.today)
                    .is_some_and(|(due, today)| due < today),
            assignee: self.person(t.assignee_id),
            updated: wib_datetime(&t.updated_at),
            project: self
                .projects
                .get(&t.project_id)
                .map(|(name, _)| name.clone())
                .unwrap_or_default(),
            stage_key: t.stage_key.clone(),
            stage_state: t.stage_state.clone(),
        }
    }
}

/// `dd/mm/yyyy`.
#[must_use]
pub fn format_date(date: NaiveDate) -> String {
    date.format("%d/%m/%Y").to_string()
}

/// Kartu board dan baris tabel tiket.
#[derive(Debug, Clone, Serialize)]
pub struct TicketRow {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub href: String,
    /// POST pindah status (menu kartu dan drag-drop).
    pub status_action: String,
    pub ticket_type: TypeView,
    pub status: StatusView,
    pub priority: PriorityView,
    pub labels: Vec<String>,
    pub points: Option<i32>,
    /// `dd/mm/yyyy`; `""` = tanpa tenggat.
    pub due: String,
    pub overdue: bool,
    pub assignee: Option<PersonView>,
    pub updated: String,
    /// Nama project (Tiket saya, pencarian); `""` di halaman project.
    pub project: String,
    /// Kunci stage sekarang dan keadaannya (`active`/`review`/`decision`).
    pub stage_key: String,
    pub stage_state: String,
}

// ---------------------------------------------------------------------------------------------
// Board

#[derive(Debug, Serialize)]
pub struct BoardColumn {
    pub status: StatusView,
    pub cards: Vec<TicketRow>,
}

/// Kolom board per stage (M3): kunci stage, nama, kategori (ikon/warna status), kartu.
#[derive(Debug, Clone, Serialize)]
pub struct StageColumn {
    pub key: String,
    pub name: String,
    pub status: StatusView,
    pub cards: Vec<TicketRow>,
}

/// Kolom board dari gabungan stage semua workflow yang dipakai project: kunci sama = satu kolom
/// (nama dari workflow pertama), urut kategori lalu posisi di workflow-nya. Tiket masuk kolom
/// kunci stage-nya; stage yang sudah dihapus (belum dimigrasi) masuk kolom pertama kategorinya.
#[must_use]
pub fn stage_columns(
    lookup: &Lookup,
    definitions: &[agentgate_core::workflow::Definition],
    tickets: &[tickets::Model],
) -> Vec<StageColumn> {
    let mut seen: Vec<(String, String, agentgate_core::workflow::Category, usize)> = Vec::new();
    for d in definitions {
        for (i, s) in d.stages.iter().enumerate() {
            if !seen.iter().any(|(k, ..)| *k == s.key) {
                seen.push((s.key.clone(), s.name.clone(), s.category, i));
            }
        }
    }
    seen.sort_by_key(|(_, _, c, i)| (*c, *i));
    let mut columns: Vec<StageColumn> = seen
        .into_iter()
        .map(|(key, name, category, _)| StageColumn {
            key,
            name,
            status: status_view(
                TicketStatus::parse(category.as_str()).unwrap_or(TicketStatus::Todo),
            ),
            cards: Vec::new(),
        })
        .collect();
    for t in tickets {
        let i = columns
            .iter()
            .position(|c| c.key == t.stage_key)
            .or_else(|| columns.iter().position(|c| c.status.value == t.status));
        match i {
            Some(i) => columns[i].cards.push(lookup.row(t)),
            None => columns.push(StageColumn {
                key: t.stage_key.clone(),
                name: t.stage_key.clone(),
                status: status_view(t.status().unwrap_or(TicketStatus::Todo)),
                cards: vec![lookup.row(t)],
            }),
        }
    }
    columns
}

/// Kolom board dari tiket yang sudah urut posisi.
#[must_use]
pub fn board_columns(lookup: &Lookup, tickets: &[tickets::Model]) -> Vec<BoardColumn> {
    STATUSES
        .iter()
        .map(|status| BoardColumn {
            status: status_view(*status),
            cards: tickets
                .iter()
                .filter(|t| t.status().ok() == Some(*status))
                .map(|t| lookup.row(t))
                .collect(),
        })
        .collect()
}

/// Filter cepat board (GET biasa).
#[derive(Debug, Serialize)]
pub struct BoardFilter {
    pub q: String,
    pub mine: bool,
    pub unassigned: bool,
    /// Tautan untuk menyalakan/mematikan filter cepat (mempertahankan yang lain).
    pub mine_href: String,
    pub unassigned_href: String,
    pub clear_href: String,
    pub total: usize,
}

// ---------------------------------------------------------------------------------------------
// Tabel

const LIST_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec::new("ticket-column-code").sort("code"),
    ColumnSpec::new("ticket-column-title").sort("title"),
    ColumnSpec::new("ticket-column-status").sort("status"),
    ColumnSpec::new("ticket-column-assignee").hide("md"),
    ColumnSpec::new("ticket-column-priority").sort("priority"),
    ColumnSpec::new("ticket-column-points").hide("md").right(),
    ColumnSpec::new("ticket-column-updated")
        .sort("updated")
        .hide("lg"),
];
const LIST_FILTERS: &[FilterSpec] = &[
    FilterSpec {
        key: "status",
        label_key: "ticket-column-status",
        icon: "progress",
    },
    FilterSpec {
        key: "priority",
        label_key: "ticket-column-priority",
        icon: "antenna-bars-5",
    },
    FilterSpec {
        key: "type",
        label_key: "ticket-column-type",
        icon: "category-2",
    },
    FilterSpec {
        key: "who",
        label_key: "ticket-column-assignee",
        icon: "users",
    },
];

/// Daftar tiket project.
pub const LIST_TABLE: TableSpec = TableSpec {
    id: "tickets",
    columns: LIST_COLUMNS,
    filters: LIST_FILTERS,
    default_sort: "updated",
    default_desc: true,
    default_per: 25,
    search_key: Some("ticket-search-placeholder"),
};

const CROSS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec::new("ticket-column-code").sort("code"),
    ColumnSpec::new("ticket-column-title").sort("title"),
    ColumnSpec::new("ticket-column-project").hide("md"),
    ColumnSpec::new("ticket-column-status").sort("status"),
    ColumnSpec::new("ticket-column-priority").sort("priority"),
    ColumnSpec::new("ticket-column-updated")
        .sort("updated")
        .hide("lg"),
];
const CROSS_FILTERS: &[FilterSpec] = &[
    FilterSpec {
        key: "status",
        label_key: "ticket-column-status",
        icon: "progress",
    },
    FilterSpec {
        key: "priority",
        label_key: "ticket-column-priority",
        icon: "antenna-bars-5",
    },
];

/// Tiket saya (lintas project).
pub const MY_TABLE: TableSpec = TableSpec {
    id: "my-tickets",
    columns: CROSS_COLUMNS,
    filters: CROSS_FILTERS,
    default_sort: "updated",
    default_desc: true,
    default_per: 25,
    search_key: Some("ticket-search-placeholder"),
};

/// Pencarian (lintas project).
pub const SEARCH_TABLE: TableSpec = TableSpec {
    id: "search",
    columns: CROSS_COLUMNS,
    filters: CROSS_FILTERS,
    default_sort: "updated",
    default_desc: true,
    default_per: 25,
    search_key: Some("search-placeholder"),
};

// ---------------------------------------------------------------------------------------------
// Form tiket

/// Satu field custom di form tiket.
#[derive(Debug, Clone, Serialize)]
pub struct CustomFieldInput {
    pub id: Uuid,
    /// Nama input dan kunci error (`custom_<id>`).
    pub name: String,
    pub label: String,
    pub kind: &'static str,
    pub options: Vec<SelectOption>,
    pub required: bool,
    pub value: String,
    /// Id tipe yang memakai field ini (dipisah spasi; `ticket-form.js` menyembunyikan sisanya).
    pub type_ids: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectOption {
    pub id: String,
    pub name: String,
}

#[must_use]
pub fn field_kind_key(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "text",
        FieldKind::Number => "number",
        FieldKind::Select => "select",
        FieldKind::Date => "date",
        FieldKind::Url => "url",
    }
}

/// Field custom untuk form: semua field yang dipakai setidaknya satu tipe.
#[must_use]
pub fn custom_inputs(
    fields: &[custom_fields::Model],
    links: &[(Uuid, Uuid)],
    values: &HashMap<Uuid, String>,
    errors: Option<&ModelValidationErrors>,
) -> Vec<CustomFieldInput> {
    fields
        .iter()
        .filter_map(|f| {
            let type_ids: Vec<String> = links
                .iter()
                .filter(|(_, field)| *field == f.id)
                .map(|(t, _)| t.to_string())
                .collect();
            if type_ids.is_empty() {
                return None;
            }
            let name = format!("custom_{}", f.id);
            let error = errors
                .and_then(|e| e.errors.get(&name))
                .and_then(|l| l.first())
                .map(|e| e.code.clone())
                .unwrap_or_default();
            Some(CustomFieldInput {
                id: f.id,
                label: f.name.clone(),
                kind: f.kind().map_or("text", field_kind_key),
                options: f
                    .options
                    .iter()
                    .map(|o| SelectOption {
                        id: o.clone(),
                        name: o.clone(),
                    })
                    .collect(),
                required: f.required,
                value: values.get(&f.id).cloned().unwrap_or_default(),
                type_ids: type_ids.join(" "),
                error,
                name,
            })
        })
        .collect()
}

/// Isian form tiket (baru atau ubah).
#[derive(Debug, Clone, Serialize, Default)]
pub struct TicketFormView {
    pub title: String,
    pub description: String,
    pub type_id: String,
    pub status: String,
    pub priority: String,
    pub assignee: String,
    pub parent: String,
    pub labels: String,
    pub due_on: String,
    pub story_points: String,
    pub errors: FieldErrors,
}

impl TicketFormView {
    #[must_use]
    pub fn with_errors(mut self, errors: Option<&ModelValidationErrors>) -> Self {
        self.errors = field_errors(TICKET_FIELDS, errors);
        self
    }

    #[must_use]
    pub fn of(t: &tickets::Model, parent_code: Option<&str>) -> Self {
        Self {
            title: t.title.clone(),
            description: t.description.clone(),
            type_id: t.type_id.to_string(),
            status: t.status.clone(),
            priority: t.priority.clone(),
            assignee: t.assignee_id.map(|id| id.to_string()).unwrap_or_default(),
            parent: parent_code.unwrap_or_default().to_string(),
            labels: t.labels.join(", "),
            due_on: t.due_on.map(|d| d.to_string()).unwrap_or_default(),
            story_points: t.story_points.map(|p| p.to_string()).unwrap_or_default(),
            errors: field_errors(TICKET_FIELDS, None),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Detail tiket

#[derive(Debug, Serialize)]
pub struct CommentView {
    pub id: Uuid,
    pub author: Option<PersonView>,
    pub body: String,
    pub at: String,
    pub edited: bool,
    /// Penulis = user ini (ubah/hapus).
    pub mine: bool,
    pub edit_action: String,
    pub delete_action: String,
    /// Job `@agent` dari komentar ini (M5).
    pub agent_jobs: Vec<AgentJobView>,
}

/// Job `@agent` di bawah komentar: status, tautan, dan penanda runner penulis offline.
#[derive(Debug, Serialize)]
pub struct AgentJobView {
    pub persona: String,
    pub status_key: &'static str,
    pub status_variant: &'static str,
    pub href: String,
    /// Job masih menunggu dan runner pelaku (penulis komentar) sedang offline.
    pub waiting: bool,
}

#[derive(Debug, Serialize)]
pub struct LinkView {
    pub id: Uuid,
    /// Kunci Fluent arah relasi dari sisi tiket ini.
    pub label_key: &'static str,
    /// Relasi pemblokir yang belum selesai (tanda merah).
    pub blocking: bool,
    pub other: TicketRow,
    pub delete_action: String,
}

#[derive(Debug, Serialize)]
pub struct AttachmentView {
    pub id: Uuid,
    pub filename: String,
    pub size: String,
    pub uploader: Option<PersonView>,
    pub at: String,
    pub href: String,
    pub delete_action: String,
}

/// Ukuran berkas ringkas (B, KB, MB).
#[must_use]
pub fn human_size(bytes: i64) -> String {
    let bytes = u64::try_from(bytes).unwrap_or_default();
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        let tenths = (bytes * 10).div_ceil(1024 * 1024);
        format!("{},{} MB", tenths / 10, tenths % 10)
    }
}

/// Nilai field custom di panel Detail.
#[derive(Debug, Serialize)]
pub struct CustomValueView {
    pub label: String,
    pub value: String,
    pub is_url: bool,
}

/// Satu baris riwayat.
#[derive(Debug, Serialize)]
pub struct EventView {
    pub kind: String,
    pub actor: Option<PersonView>,
    pub at: String,
    /// Kunci Fluent kalimat ringkas.
    pub label_key: String,
    /// Perubahan field: `field` = kunci Fluent label field, `from`/`to` teks tampil.
    pub changes: Vec<ChangeView>,
    /// Teks tambahan (kode tiket terkait, nama berkas, nama stage).
    pub detail: String,
    /// Catatan reject/keputusan atau alasan lompat/paksa.
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct ChangeView {
    pub field_key: String,
    pub from: String,
    pub to: String,
    /// `from`/`to` berupa kunci Fluent (status, prioritas).
    pub translate: bool,
}

// ---------------------------------------------------------------------------------------------
// Kotak masuk

#[derive(Debug, Serialize)]
pub struct NotificationView {
    pub id: Uuid,
    pub kind: String,
    /// Kunci Fluent kalimat (`notification-mentioned` dst.).
    pub label_key: String,
    pub actor: Option<PersonView>,
    pub ticket: Option<TicketRow>,
    pub at: String,
    pub unread: bool,
    pub read_action: String,
}

// ---------------------------------------------------------------------------------------------
// Pengaturan

/// Isian form tipe tiket.
#[derive(Debug, Serialize, Default)]
pub struct TypeFormView {
    pub key: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub errors: FieldErrors,
}

impl TypeFormView {
    #[must_use]
    pub fn of(t: &ticket_types::Model) -> Self {
        Self {
            key: t.key.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            icon: t.icon.clone(),
            color: t.color.clone(),
            errors: field_errors(TYPE_FIELDS, None),
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            icon: "bookmark".into(),
            color: "#0891b2".into(),
            errors: field_errors(TYPE_FIELDS, None),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_errors(mut self, errors: Option<&ModelValidationErrors>) -> Self {
        self.errors = field_errors(TYPE_FIELDS, errors);
        self
    }
}

/// Isian form field custom.
#[derive(Debug, Serialize, Default)]
pub struct FieldFormView {
    pub name: String,
    pub kind: String,
    pub options: String,
    pub required: bool,
    pub type_ids: Vec<String>,
    pub errors: FieldErrors,
}

impl FieldFormView {
    #[must_use]
    pub fn of(f: &custom_fields::Model, type_ids: Vec<String>) -> Self {
        Self {
            name: f.name.clone(),
            kind: f.kind.clone(),
            options: f.options.join("\n"),
            required: f.required,
            type_ids,
            errors: field_errors(FIELD_FIELDS, None),
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            kind: "text".into(),
            errors: field_errors(FIELD_FIELDS, None),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_errors(mut self, errors: Option<&ModelValidationErrors>) -> Self {
        self.errors = field_errors(FIELD_FIELDS, errors);
        self
    }
}

/// Pilihan jenis field (kunci Fluent `custom-field-kind-*`).
#[must_use]
pub fn field_kind_options() -> Vec<SelectOption> {
    ["text", "number", "select", "date", "url"]
        .iter()
        .map(|k| SelectOption {
            id: (*k).to_string(),
            name: format!("custom-field-kind-{k}"),
        })
        .collect()
}

/// Pilihan skop penghitung (kunci Fluent `ticket-code-scope-*`).
#[must_use]
pub fn scope_options() -> Vec<SelectOption> {
    ["project", "type", "year"]
        .iter()
        .map(|k| SelectOption {
            id: (*k).to_string(),
            name: format!("ticket-code-scope-{k}"),
        })
        .collect()
}

/// Isian template kode tiket.
#[derive(Debug, Serialize, Default)]
pub struct CodeFormView {
    pub custom: bool,
    pub template: String,
    pub scope: String,
    pub team_code: String,
    /// Contoh kode menurut template tersimpan/isian.
    pub preview: String,
    pub errors: FieldErrors,
}

impl CodeFormView {
    #[must_use]
    pub fn with_errors(mut self, errors: Option<&ModelValidationErrors>) -> Self {
        self.errors = field_errors(CODE_FIELDS, errors);
        self
    }
}

/// Token template kode untuk chip bantuan.
pub const CODE_TOKENS: &[(&str, &str)] = &[
    ("{PROJECT}", "ticket-code-token-project"),
    ("{TEAM}", "ticket-code-token-team"),
    ("{TYPE}", "ticket-code-token-type"),
    ("{YYYY}", "ticket-code-token-year"),
    ("{YY}", "ticket-code-token-year-short"),
    ("{NUM}", "ticket-code-token-num"),
    ("{NUM:4}", "ticket-code-token-num-padded"),
];

#[derive(Debug, Serialize)]
pub struct TokenView {
    pub token: &'static str,
    pub label_key: &'static str,
}

#[must_use]
pub fn code_tokens() -> Vec<TokenView> {
    CODE_TOKENS
        .iter()
        .map(|(token, label_key)| TokenView { token, label_key })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_short() {
        assert_eq!(human_size(10), "10 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(1024 * 1024 * 3 / 2), "1,5 MB");
    }
}
