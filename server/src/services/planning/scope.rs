//! Log isi sprint (`sprint_scope_events`). Dipanggil di transaksi yang sama dengan perubahannya:
//! pindah sprint (`sprints::move_ticket`, ubah tiket, bulk edit), masuk/keluar Selesai
//! (`tickets::stages::enter_stage`), dan poin berubah (`tickets::update`). Laporan burndown/
//! burnup mereplay log ini ([`replay`]).
use std::collections::HashMap;

use chrono::NaiveDate;
use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

use crate::models::{
    sprint_scope_events::{self, ScopeKind},
    tickets,
    workspaces::WorkspaceId,
};

/// Tiket pindah sprint (`from` → `to`). Tidak menulis apa pun bila sama.
///
/// # Errors
///
/// Error database.
pub async fn sprint_changed<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    from: Option<Uuid>,
    to: Option<Uuid>,
) -> Result<(), DbErr> {
    if from == to {
        return Ok(());
    }
    if let Some(old) = from {
        sprint_scope_events::ActiveModel::log(db, wid, old, ticket.id, ScopeKind::Removed, None)
            .await?;
    }
    if let Some(new) = to {
        sprint_scope_events::ActiveModel::log(
            db,
            wid,
            new,
            ticket.id,
            ScopeKind::Added,
            ticket.story_points,
        )
        .await?;
        if ticket.status == "done" {
            sprint_scope_events::ActiveModel::log(db, wid, new, ticket.id, ScopeKind::Done, None)
                .await?;
        }
    }
    Ok(())
}

/// Tiket masuk (`done = true`) atau keluar dari kategori Selesai.
///
/// # Errors
///
/// Error database.
pub async fn done_changed<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    done: bool,
) -> Result<(), DbErr> {
    let Some(sprint) = ticket.sprint_id else {
        return Ok(());
    };
    let kind = if done {
        ScopeKind::Done
    } else {
        ScopeKind::Reopened
    };
    sprint_scope_events::ActiveModel::log(db, wid, sprint, ticket.id, kind, None).await?;
    Ok(())
}

/// Story point tiket berubah.
///
/// # Errors
///
/// Error database.
pub async fn points_changed<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    ticket: &tickets::Model,
    points: Option<i32>,
) -> Result<(), DbErr> {
    let Some(sprint) = ticket.sprint_id else {
        return Ok(());
    };
    sprint_scope_events::ActiveModel::log(db, wid, sprint, ticket.id, ScopeKind::Points, points)
        .await?;
    Ok(())
}

/// Satu kejadian log yang sudah dibaca (tanggal WIB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeEvent {
    pub ticket: Uuid,
    pub kind: ScopeKind,
    pub points: i32,
    pub at: chrono::DateTime<chrono::Utc>,
}

impl ScopeEvent {
    #[must_use]
    pub fn of(m: &sprint_scope_events::Model) -> Self {
        Self {
            ticket: m.ticket_id,
            kind: m.kind(),
            points: m.points,
            at: m.created_at.with_timezone(&chrono::Utc),
        }
    }
}

/// Keadaan satu tiket saat replay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TicketState {
    pub inside: bool,
    pub points: i32,
    pub done: bool,
}

/// Ringkasan isi sprint pada satu titik waktu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Total poin tiket di sprint.
    pub scope: i32,
    /// Poin tiket yang sudah selesai.
    pub done: i32,
    /// Jumlah tiket di sprint.
    pub tickets: usize,
}

impl Snapshot {
    #[must_use]
    pub fn remaining(&self) -> i32 {
        self.scope - self.done
    }
}

/// Replay log sampai `until` (inklusif).
#[must_use]
pub fn state_at(
    events: &[ScopeEvent],
    until: chrono::DateTime<chrono::Utc>,
) -> HashMap<Uuid, TicketState> {
    let mut out: HashMap<Uuid, TicketState> = HashMap::new();
    for e in events.iter().filter(|e| e.at <= until) {
        let s = out.entry(e.ticket).or_default();
        match e.kind {
            ScopeKind::Added => {
                s.inside = true;
                s.points = e.points;
            }
            ScopeKind::Removed => s.inside = false,
            ScopeKind::Done => s.done = true,
            ScopeKind::Reopened => s.done = false,
            ScopeKind::Points => s.points = e.points,
        }
    }
    out
}

#[must_use]
pub fn snapshot(states: &HashMap<Uuid, TicketState>) -> Snapshot {
    states
        .values()
        .filter(|s| s.inside)
        .fold(Snapshot::default(), |mut acc, s| {
            acc.scope += s.points;
            acc.tickets += 1;
            if s.done {
                acc.done += s.points;
            }
            acc
        })
}

/// Akhir hari `day` (WIB) dalam UTC.
#[must_use]
pub fn end_of_day(day: NaiveDate) -> chrono::DateTime<chrono::Utc> {
    let wib = day
        .succ_opt()
        .unwrap_or(day)
        .and_hms_opt(0, 0, 0)
        .unwrap_or_default();
    wib.and_utc() - chrono::Duration::hours(7) - chrono::Duration::seconds(1)
}

/// Poin yang ditambahkan setelah `start` (tiket masuk sprint setelah mulai).
#[must_use]
pub fn added_after(events: &[ScopeEvent], start: chrono::DateTime<chrono::Utc>) -> (i32, usize) {
    let before = state_at(events, start);
    let mut seen: Vec<Uuid> = Vec::new();
    let mut points = 0;
    for e in events
        .iter()
        .filter(|e| e.at > start && e.kind == ScopeKind::Added)
    {
        if before.get(&e.ticket).is_some_and(|s| s.inside) || seen.contains(&e.ticket) {
            continue;
        }
        seen.push(e.ticket);
        points += e.points;
    }
    (points, seen.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(1_800_000_000 + h * 3600, 0).unwrap()
    }

    fn ev(t: u128, kind: ScopeKind, points: i32, h: i64) -> ScopeEvent {
        ScopeEvent {
            ticket: Uuid::from_u128(t),
            kind,
            points,
            at: at(h),
        }
    }

    #[test]
    fn replay_tracks_scope_done_and_points() {
        let log = vec![
            ev(1, ScopeKind::Added, 5, 0),
            ev(2, ScopeKind::Added, 3, 0),
            ev(1, ScopeKind::Done, 0, 10),
            ev(3, ScopeKind::Added, 2, 20),
            ev(2, ScopeKind::Points, 8, 30),
            ev(1, ScopeKind::Reopened, 0, 40),
            ev(3, ScopeKind::Removed, 0, 50),
        ];
        assert_eq!(
            snapshot(&state_at(&log, at(1))),
            Snapshot {
                scope: 8,
                done: 0,
                tickets: 2
            }
        );
        assert_eq!(snapshot(&state_at(&log, at(25))).done, 5);
        assert_eq!(snapshot(&state_at(&log, at(25))).scope, 10);
        assert_eq!(snapshot(&state_at(&log, at(35))).remaining(), 10);
        let end = snapshot(&state_at(&log, at(60)));
        assert_eq!((end.scope, end.done, end.tickets), (13, 0, 2));
        assert_eq!(added_after(&log, at(1)), (2, 1));
    }
}
