//! `GET /w/{slug}/people?q=`: opsi combobox orang (komponen `combo` dengan `src`). Terbuka untuk
//! semua anggota workspace (extractor `Tenant`: bukan anggota → 404). Membalas hanya `<option>`
//! (partial `workspaces/people_options.html`), maksimal [`LIMIT`] orang, urut nama.
//! `not_in_team=<id>` membuang orang yang sudah di tim itu (form tambah anggota tim). Tidak
//! pernah menulis.
use loco_rs::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    controllers::extractors::Tenant,
    models::{
        team_members,
        workspace_members::{self, MemberQuery, MemberSort},
    },
    views::{self, ui::ComboOption},
};

/// Batas opsi per pencarian; ketik lebih spesifik untuk mempersempit.
pub const LIMIT: u64 = 20;

#[derive(Debug, Default, Deserialize)]
pub struct PeopleQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    not_in_team: Option<String>,
}

#[debug_handler]
pub(super) async fn options(
    State(ctx): State<AppContext>,
    ViewEngine(v): ViewEngine<TeraView>,
    tenant: Tenant,
    Query(query): Query<PeopleQuery>,
) -> Result<Response> {
    let wid = tenant.workspace_id();
    let exclude: Vec<Uuid> = match query
        .not_in_team
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok())
    {
        Some(team_id) => team_members::Model::of_team(&ctx.db, wid, team_id)
            .await?
            .into_iter()
            .map(|(m, _)| m.user_id)
            .collect(),
        None => Vec::new(),
    };
    let member_query = MemberQuery {
        q: Some(query.q.as_str()),
        role_ids: Vec::new(),
        team_ids: Vec::new(),
        two_factor: None,
        sort: MemberSort::Name,
        desc: false,
        offset: 0,
        // Ambil lebih banyak supaya setelah membuang anggota tim tetap ada LIMIT opsi.
        limit: LIMIT + u64::try_from(exclude.len()).unwrap_or(0),
    };
    let (rows, _) = workspace_members::Model::page_in(&ctx.db, wid, &member_query).await?;
    let options: Vec<ComboOption> = rows
        .iter()
        .filter(|(_, user, _)| !exclude.contains(&user.id))
        .take(usize::try_from(LIMIT).unwrap_or(20))
        .map(|(_, user, _)| ComboOption::person(user.id, &user.name, &user.email))
        .collect();
    views::workspaces::people_options(&v, &options)
}
