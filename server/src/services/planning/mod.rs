//! Planning & laporan (M9): sprint (backlog, mulai/selesai, kapasitas), log scope sprint untuk
//! burndown/burnup, estimasi AI/manusia + kalibrasi, laporan, roadmap/kalender, rilis + release
//! notes, components, query ala JQL, saved filter, dan bulk edit.
//!
//! Izin: membaca `ViewProject`; merencanakan (sprint, pindah tiket, estimasi, rilis, release notes)
//! `EditProject`; components `AdministerProject`. Saved filter milik user (dibagikan ke tim/
//! workspace); mengubah/menghapus hanya pemiliknya.
pub mod components;
pub mod estimates;
pub mod refs;
pub mod reports;
pub mod scope;
pub mod sprints;

use chrono::{Datelike, NaiveDate, Weekday};

pub use super::tickets::TicketFailure as Failure;

/// Nama kosong atau terlalu panjang.
pub const NAME_INVALID: &str = "planning-name-invalid";
/// Nama sudah dipakai di project ini.
pub const NAME_TAKEN: &str = "planning-name-taken";
/// Teks (tujuan, deskripsi) terlalu panjang.
pub const TEXT_TOO_LONG: &str = "planning-text-too-long";
/// Tanggal tidak valid atau selesai sebelum mulai.
pub const DATES_INVALID: &str = "planning-dates-invalid";
/// Kapasitas bukan angka 0–999.
pub const CAPACITY_INVALID: &str = "planning-capacity-invalid";
/// Sprint tidak bisa diubah pada keadaannya sekarang (mis. mulai sprint yang sudah selesai).
pub const SPRINT_STATE: &str = "planning-sprint-state";
/// Sprint/rilis tujuan tidak valid.
pub const TARGET_INVALID: &str = "planning-target-invalid";
/// Estimasi tidak valid (poin/jam di luar batas, stage tidak dikenal).
pub const ESTIMATE_INVALID: &str = "planning-estimate-invalid";
/// Release notes terlalu panjang.
pub const NOTES_TOO_LONG: &str = "planning-notes-too-long";
/// Isi sama dengan versi sekarang.
pub const UNCHANGED: &str = "planning-unchanged";
/// Rilis sudah dirilis/diarsipkan.
pub const RELEASE_STATE: &str = "planning-release-state";
/// Pemilik component harus bisa membuka project.
pub const OWNER_INVALID: &str = "planning-owner-invalid";
/// Query JQL tidak bisa dibaca.
pub const QUERY_INVALID: &str = "planning-query-invalid";
/// Tim saved filter tidak valid (bukan anggota tim).
pub const TEAM_INVALID: &str = "planning-team-invalid";
/// Tidak ada tiket terpilih / tidak ada perubahan di bulk edit.
pub const BULK_EMPTY: &str = "planning-bulk-empty";

pub const ERROR_CODES: &[&str] = &[
    NAME_INVALID,
    NAME_TAKEN,
    TEXT_TOO_LONG,
    DATES_INVALID,
    CAPACITY_INVALID,
    SPRINT_STATE,
    TARGET_INVALID,
    ESTIMATE_INVALID,
    NOTES_TOO_LONG,
    UNCHANGED,
    RELEASE_STATE,
    OWNER_INVALID,
    QUERY_INVALID,
    TEAM_INVALID,
    BULK_EMPTY,
];

/// Jam kerja per hari kerja (perkiraan selesai dari estimasi).
pub const HOURS_PER_DAY: f64 = 8.0;

/// Hari kerja (Senin–Jumat).
#[must_use]
pub fn is_workday(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Hari kerja di rentang `[from, to]` (inklusif), urut.
#[must_use]
pub fn workdays(from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    from.iter_days()
        .take_while(|d| *d <= to)
        .filter(|d| is_workday(*d))
        .collect()
}

/// Tanggal setelah menambah `days` hari kerja ke `from` (`days` = 0 → `from` atau hari kerja
/// berikutnya bila `from` akhir pekan).
#[must_use]
pub fn add_workdays(from: NaiveDate, days: u32) -> NaiveDate {
    let mut d = from;
    while !is_workday(d) {
        d = d.succ_opt().unwrap_or(d);
    }
    let mut left = days;
    while left > 0 {
        d = d.succ_opt().unwrap_or(d);
        if is_workday(d) {
            left -= 1;
        }
    }
    d
}

/// Hari ini (WIB, UTC+7) untuk tampilan dan perhitungan tanggal.
#[must_use]
pub fn today() -> NaiveDate {
    (chrono::Utc::now() + chrono::Duration::hours(7)).date_naive()
}

/// Tanggal WIB dari waktu UTC.
#[must_use]
pub fn wib_date(at: &chrono::DateTime<chrono::FixedOffset>) -> NaiveDate {
    (at.with_timezone(&chrono::Utc) + chrono::Duration::hours(7)).date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn workdays_skip_weekends() {
        // 2026-11-13 Jumat, 16 Senin.
        assert_eq!(
            workdays(d("2026-11-13"), d("2026-11-17")),
            vec![d("2026-11-13"), d("2026-11-16"), d("2026-11-17")]
        );
        assert_eq!(add_workdays(d("2026-11-13"), 1), d("2026-11-16"));
        assert_eq!(add_workdays(d("2026-11-14"), 0), d("2026-11-16"));
        assert_eq!(add_workdays(d("2026-11-12"), 3), d("2026-11-17"));
    }
}
