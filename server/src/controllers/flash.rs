//! Flash di sesi (rules §6: data layout lewat satu helper): POST yang berhasil menyimpan satu
//! pesan sebelum redirect `303`, dan GET berikutnya membacanya **sekali** lalu menghapusnya.
//! Isinya kunci Fluent (`views::flash::Flash`), dirender komponen `flash` di layout.

use loco_rs::{Result, auth::session::Session};

use crate::views::flash::Flash;

/// Kunci data sesi untuk pesan flash.
const FLASH: &str = "flash";

/// Simpan `flash` untuk GET berikutnya. Kembalikan sesinya di respons.
///
/// # Errors
///
/// Bila data sesi tidak bisa di-serialize.
pub fn set(session: Session, flash: &Flash) -> Result<Session> {
    session.insert(FLASH, flash)
}

/// Ambil dan hapus flash, bila ada. Kembalikan sesinya di respons, supaya penghapusan
/// tersimpan dan pesan tidak tampil dua kali.
#[must_use]
pub fn take(session: Session) -> (Session, Option<Flash>) {
    match session.get::<Flash>(FLASH) {
        Some(flash) => (session.remove(FLASH), Some(flash)),
        None => (session, None),
    }
}
