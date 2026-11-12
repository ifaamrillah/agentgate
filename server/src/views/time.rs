//! Waktu untuk tampilan: database menyimpan UTC (`timestamptz`), halaman menampilkan WIB
//! (Asia/Jakarta). Jakarta tidak memakai daylight saving, jadi offset tetap +07:00 cukup tanpa
//! database zona waktu.
use chrono::{DateTime, FixedOffset, TimeZone};

/// UTC+07:00. Dievaluasi saat kompilasi: offset di luar ±24 jam gagal build, bukan saat jalan.
const WIB: FixedOffset = match FixedOffset::east_opt(7 * 3600) {
    Some(offset) => offset,
    None => panic!("offset WIB tidak valid"),
};

/// `dd/mm/yyyy HH:MM` di WIB.
#[must_use]
pub fn wib_datetime<Tz: TimeZone>(at: &DateTime<Tz>) -> String {
    at.with_timezone(&WIB).format("%d/%m/%Y %H:%M").to_string()
}

/// Tanggal panjang di WIB untuk template: nama hari dan bulan diterjemahkan Fluent
/// (`weekday-<0..6>` mulai Senin, `month-<1..12>`), salam menurut jam (`greeting-*`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LongDate {
    pub weekday_key: String,
    pub day: u32,
    pub month_key: String,
    pub year: i32,
    pub greeting_key: &'static str,
}

#[must_use]
pub fn wib_long_date<Tz: TimeZone>(at: &DateTime<Tz>) -> LongDate {
    use chrono::{Datelike, Timelike};
    let local = at.with_timezone(&WIB);
    let greeting_key = match local.hour() {
        4..=10 => "greeting-morning",
        11..=14 => "greeting-day",
        15..=17 => "greeting-afternoon",
        _ => "greeting-evening",
    };
    LongDate {
        weekday_key: format!("weekday-{}", local.weekday().num_days_from_monday()),
        day: local.day(),
        month_key: format!("month-{}", local.month()),
        year: local.year(),
        greeting_key,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn formats_in_jakarta_time() {
        let at = Utc.with_ymd_and_hms(2026, 11, 11, 20, 5, 0).unwrap();
        assert_eq!(wib_datetime(&at), "12/11/2026 03:05");
    }

    #[test]
    fn long_date_in_jakarta_time() {
        let at = Utc.with_ymd_and_hms(2026, 11, 12, 5, 0, 0).unwrap();
        let d = wib_long_date(&at);
        assert_eq!(
            (d.weekday_key.as_str(), d.day, d.month_key.as_str(), d.year),
            ("weekday-3", 12, "month-11", 2026)
        );
        assert_eq!(d.greeting_key, "greeting-day");
    }
}
