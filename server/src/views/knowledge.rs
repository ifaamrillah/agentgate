//! View knowledge base (M8): status dokumen, baris dokumen, kartu kelompok, antrean usulan,
//! donut kesehatan, versi. Data dihitung controller; di sini hanya bentuk tampilannya.
use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use super::{nav::NavItem, tickets::PersonView, time::wib_datetime};
use crate::models::{
    knowledge_catalog::{self, Group},
    knowledge_docs, knowledge_proposals,
    knowledge_proposals::ProposalTrigger,
    knowledge_versions, users,
};

/// Status tampilan dokumen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocState {
    Fresh,
    Proposal,
    Stale,
}

impl DocState {
    #[must_use]
    pub fn of(doc: &knowledge_docs::Model, has_proposal: bool) -> Self {
        if has_proposal {
            Self::Proposal
        } else if doc.is_stale() {
            Self::Stale
        } else {
            Self::Fresh
        }
    }

    #[must_use]
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Fresh => "kb-state-fresh",
            Self::Proposal => "kb-state-proposal",
            Self::Stale => "kb-state-stale",
        }
    }

    /// Varian badge.
    #[must_use]
    pub const fn variant(self) -> &'static str {
        match self {
            Self::Fresh => "success",
            Self::Proposal => "info",
            Self::Stale => "gate",
        }
    }

    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Fresh => "circle-check",
            Self::Proposal => "git-pull-request",
            Self::Stale => "clock",
        }
    }
}

/// Satu dokumen di daftar.
#[derive(Debug, Clone, Serialize)]
pub struct DocRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub href: String,
    pub group: String,
    pub type_key: String,
    pub state: DocState,
    pub state_key: &'static str,
    pub state_variant: &'static str,
    pub state_icon: &'static str,
    pub stale_reason: String,
    pub path: String,
    pub version: i32,
    pub owner: Option<PersonView>,
    pub updated: String,
    /// Usulan pending (id) untuk tombol Tinjau.
    pub proposal: Option<Uuid>,
}

impl DocRow {
    #[must_use]
    pub fn of(
        doc: &knowledge_docs::Model,
        base: &str,
        proposal: Option<Uuid>,
        people: &HashMap<Uuid, users::Model>,
    ) -> Self {
        let state = DocState::of(doc, proposal.is_some());
        Self {
            id: doc.id,
            slug: doc.slug.clone(),
            title: doc.title.clone(),
            href: format!("{base}/{}", doc.slug),
            group: doc.group_key.clone(),
            type_key: doc.type_key.clone(),
            state,
            state_key: state.label_key(),
            state_variant: state.variant(),
            state_icon: state.icon(),
            stale_reason: doc.stale_reason.clone(),
            path: doc.repo_path(),
            version: doc.version,
            owner: doc
                .owner_id
                .or(doc.updated_by)
                .and_then(|u| people.get(&u))
                .map(PersonView::of),
            updated: wib_datetime(&doc.updated_at),
            proposal,
        }
    }
}

/// Kartu kelompok di beranda.
#[derive(Debug, Clone, Serialize)]
pub struct GroupCard {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub desc: &'static str,
    pub href: String,
    pub total: usize,
    pub fresh: usize,
    pub proposal: usize,
    pub stale: usize,
    /// Empat dokumen pertama.
    pub docs: Vec<DocRow>,
    pub more: usize,
    pub owners: Vec<PersonView>,
    /// Teks pencarian (judul kelompok + dokumen), huruf kecil.
    pub search: String,
}

impl GroupCard {
    #[must_use]
    pub fn of(group: &'static Group, rows: &[DocRow], base: &str) -> Self {
        let mine: Vec<&DocRow> = rows.iter().filter(|r| r.group == group.key).collect();
        let count = |s: DocState| mine.iter().filter(|r| r.state == s).count();
        let mut owners: Vec<PersonView> = Vec::new();
        for r in &mine {
            if let Some(o) = &r.owner
                && !owners.iter().any(|x| x.id == o.id)
                && owners.len() < 3
            {
                owners.push(o.clone());
            }
        }
        let search = std::iter::once(group.name.to_string())
            .chain(mine.iter().map(|r| r.title.clone()))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        Self {
            key: group.key,
            name: group.name,
            icon: group.icon,
            desc: group.desc,
            href: format!("{base}/g/{}", group.key),
            total: mine.len(),
            fresh: count(DocState::Fresh),
            proposal: count(DocState::Proposal),
            stale: count(DocState::Stale),
            docs: mine.iter().take(4).map(|r| (*r).clone()).collect(),
            more: mine.len().saturating_sub(4),
            owners,
            search,
        }
    }
}

/// Label + ikon pemicu usulan.
#[must_use]
pub fn trigger_view(t: ProposalTrigger) -> (&'static str, &'static str) {
    (
        match t {
            ProposalTrigger::Scan => "kb-trigger-scan",
            ProposalTrigger::Done => "kb-trigger-done",
            ProposalTrigger::Merge => "kb-trigger-merge",
            ProposalTrigger::Audit => "kb-trigger-audit",
            ProposalTrigger::Manual => "kb-trigger-manual",
            ProposalTrigger::Init => "kb-trigger-init",
            ProposalTrigger::Repo => "kb-trigger-repo",
        },
        t.icon(),
    )
}

/// Satu usulan di antrean/kartu tinjau.
#[derive(Debug, Clone, Serialize)]
pub struct ProposalRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub new_doc: bool,
    pub group: String,
    pub reason: String,
    pub trigger_key: &'static str,
    pub trigger_icon: &'static str,
    pub trigger_ref: String,
    pub persona: String,
    pub tokens: Option<i64>,
    pub created: String,
    pub added: usize,
    pub removed: usize,
    pub href: String,
    pub doc_href: String,
    pub status: String,
    pub decision_note: String,
    pub decided_by: Option<PersonView>,
}

impl ProposalRow {
    #[must_use]
    pub fn of(
        p: &knowledge_proposals::Model,
        base: &str,
        stat: (usize, usize),
        people: &HashMap<Uuid, users::Model>,
    ) -> Self {
        let (trigger_key, trigger_icon) = trigger_view(p.trigger());
        Self {
            id: p.id,
            slug: p.slug.clone(),
            title: p.title.clone(),
            new_doc: p.base_version == 0,
            group: p.group_key.clone(),
            reason: p.reason.clone(),
            trigger_key,
            trigger_icon,
            trigger_ref: p.trigger_ref.clone(),
            persona: p.persona.clone(),
            tokens: p.tokens,
            created: wib_datetime(&p.created_at),
            added: stat.0,
            removed: stat.1,
            href: format!("{base}/proposals?p={}", p.id),
            doc_href: format!("{base}/{}", p.slug),
            status: p.status.clone(),
            decision_note: p.decision_note.clone(),
            decided_by: p
                .decided_by
                .and_then(|u| people.get(&u))
                .map(PersonView::of),
        }
    }
}

/// Satu versi dokumen.
#[derive(Debug, Clone, Serialize)]
pub struct VersionRow {
    pub number: i32,
    pub short: String,
    pub title: String,
    pub message: String,
    pub source_key: String,
    pub persona: String,
    pub author: Option<PersonView>,
    pub created: String,
    pub href: String,
    pub current: bool,
}

impl VersionRow {
    #[must_use]
    pub fn of(
        v: &knowledge_versions::Model,
        base: &str,
        current: i32,
        people: &HashMap<Uuid, users::Model>,
    ) -> Self {
        Self {
            number: v.number,
            short: v.short(),
            title: v.title.clone(),
            message: v.message.clone(),
            source_key: format!("kb-source-{}", v.source),
            persona: v.persona.clone(),
            author: v.author_id.and_then(|u| people.get(&u)).map(PersonView::of),
            created: wib_datetime(&v.created_at),
            href: format!("{base}?v={}", v.number),
            current: v.number == current,
        }
    }
}

/// Satu segmen donut (lingkaran r=36, keliling ≈ 226,19).
#[derive(Debug, Clone, Serialize)]
pub struct DonutSeg {
    pub tone: &'static str,
    pub dash: String,
    pub offset: String,
}

/// Donut kesehatan dokumen: persen terbaru di tengah, segmen terbaru/usulan/basi.
#[derive(Debug, Clone, Serialize)]
pub struct Donut {
    pub center: String,
    pub segments: Vec<DonutSeg>,
    pub legend: Vec<LegendItem>,
}

/// Satu baris legenda donut.
#[derive(Debug, Clone, Serialize)]
pub struct LegendItem {
    pub state: DocState,
    pub label_key: &'static str,
    pub count: usize,
}

impl Donut {
    #[must_use]
    pub fn of(fresh: usize, proposal: usize, stale: usize) -> Self {
        let total = fresh + proposal + stale;
        let c = 2.0 * std::f64::consts::PI * 36.0;
        let mut off = 0.0;
        let mut segments = Vec::new();
        for (tone, v) in [("success", fresh), ("accent", proposal), ("warn", stale)] {
            if v == 0 || total == 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let len = v as f64 / total as f64 * c;
            segments.push(DonutSeg {
                tone,
                dash: format!("{:.2} {c:.2}", (len - 2.0).max(0.5)),
                offset: format!("{:.2}", -off),
            });
            off += len;
        }
        #[allow(clippy::cast_precision_loss)]
        let pct = if total == 0 {
            0
        } else {
            ((fresh as f64 / total as f64) * 100.0).round() as i64
        };
        Self {
            center: format!("{pct}%"),
            segments,
            legend: [
                (DocState::Fresh, fresh),
                (DocState::Proposal, proposal),
                (DocState::Stale, stale),
            ]
            .into_iter()
            .map(|(state, count)| LegendItem {
                state,
                label_key: state.label_key(),
                count,
            })
            .collect(),
        }
    }
}

/// Tab Beranda / Tinjau usulan.
#[must_use]
pub fn tabs(base: &str, current: &str, pending: u64) -> Vec<NavItem> {
    let mut home = NavItem::link("home", "kb-tab-home", "home", base);
    home.current = current == "home";
    let mut review = NavItem::link(
        "proposals",
        "kb-tab-proposals",
        "git-pull-request",
        &format!("{base}/proposals"),
    );
    review.current = current == "proposals";
    if pending > 0 {
        review = review.counted(pending, false);
    }
    vec![home, review]
}

/// Tab kelompok di halaman kelompok (hanya kelompok yang punya dokumen, plus yang sedang dibuka).
#[must_use]
pub fn group_tabs(base: &str, current: &str, rows: &[DocRow]) -> Vec<NavItem> {
    knowledge_catalog::GROUPS
        .iter()
        .filter_map(|g| {
            let n = rows.iter().filter(|r| r.group == g.key).count();
            if n == 0 && g.key != current {
                return None;
            }
            let mut item = NavItem::link(
                g.key,
                group_label_key(g.key),
                g.icon,
                &format!("{base}/g/{}", g.key),
            )
            .counted(n as u64, false);
            item.current = g.key == current;
            Some(item)
        })
        .collect()
}

/// Kunci Fluent nama kelompok.
#[must_use]
pub fn group_label_key(key: &str) -> &'static str {
    match key {
        "gambaran-umum" => "kb-group-gambaran-umum",
        "arsitektur" => "kb-group-arsitektur",
        "domain-bisnis" => "kb-group-domain-bisnis",
        "backend-kode" => "kb-group-backend-kode",
        "frontend" => "kb-group-frontend",
        "mobile" => "kb-group-mobile",
        "kualitas" => "kb-group-kualitas",
        "operasional" => "kb-group-operasional",
        "data" => "kb-group-data",
        "riwayat" => "kb-group-riwayat",
        _ => "kb-group-none",
    }
}

/// Subnav Pengaturan → Knowledge workspace.
#[must_use]
pub fn settings_subnav(base: &str, current: &str) -> Vec<NavItem> {
    super::nav::mark(
        vec![
            NavItem::link("docs", "kb-ws-docs", "file-text", &format!("{base}/docs"))
                .describe("kb-ws-docs-desc"),
            NavItem::link("types", "kb-ws-types", "category", &format!("{base}/types"))
                .describe("kb-ws-types-desc"),
            NavItem::link("auto", "kb-ws-auto", "refresh", &format!("{base}/auto"))
                .describe("kb-ws-auto-desc"),
        ],
        current,
    )
}

/// Kunci Fluent yang dibangun di kode (dites kontrak i18n).
#[must_use]
pub fn label_keys() -> Vec<String> {
    let mut keys: Vec<String> = [DocState::Fresh, DocState::Proposal, DocState::Stale]
        .iter()
        .map(|s| s.label_key().to_string())
        .collect();
    for t in [
        ProposalTrigger::Scan,
        ProposalTrigger::Done,
        ProposalTrigger::Merge,
        ProposalTrigger::Audit,
        ProposalTrigger::Manual,
        ProposalTrigger::Init,
        ProposalTrigger::Repo,
    ] {
        keys.push(trigger_view(t).0.to_string());
    }
    for g in knowledge_catalog::GROUPS {
        keys.push(group_label_key(g.key).to_string());
    }
    for s in ["create", "edit", "proposal", "restore", "repo"] {
        keys.push(format!("kb-source-{s}"));
    }
    for d in ["cepat", "standar", "mendalam"] {
        keys.push(format!("kb-depth-{d}"));
        keys.push(format!("kb-depth-{d}-hint"));
    }
    for s in ["pending", "approved", "rejected", "superseded"] {
        keys.push(format!("kb-status-{s}"));
    }
    for f in [
        "approved",
        "rejected",
        "undone",
        "scan",
        "update",
        "draft",
        "created",
        "owner",
        "saved",
        "fresh",
        "archived",
        "restored",
        "invalid",
        "type-created",
        "type-saved",
        "type-archived",
        "settings",
    ] {
        keys.push(format!("kb-flash-{f}"));
    }
    keys.extend(
        [
            "kb-tab-home",
            "kb-tab-proposals",
            "kb-ws-docs",
            "kb-ws-types",
            "kb-ws-auto",
        ]
        .map(str::to_string),
    );
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donut_segments_sum_to_circle() {
        let d = Donut::of(6, 2, 2);
        assert_eq!(d.center, "60%");
        assert_eq!(d.segments.len(), 3);
        let empty = Donut::of(0, 0, 0);
        assert_eq!(empty.center, "0%");
        assert!(empty.segments.is_empty());
    }
}
