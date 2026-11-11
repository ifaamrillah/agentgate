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

/// `dd/mm/yyyy` di WIB.
#[must_use]
pub fn wib_date<Tz: TimeZone>(at: &DateTime<Tz>) -> String {
    at.with_timezone(&WIB).format("%d/%m/%Y").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn formats_in_jakarta_time() {
        let at = Utc.with_ymd_and_hms(2026, 11, 11, 20, 5, 0).unwrap();
        assert_eq!(wib_datetime(&at), "12/11/2026 03:05");
        assert_eq!(wib_date(&at), "12/11/2026");
    }
}
