//! @mention di komentar dan deskripsi tiket (spec "@mention orang dan tiket").
//!
//! Sasaran mention hanya orang yang punya akses ke project tiket (`users_with_access`), supaya
//! notifikasi tidak membocorkan tiket ke orang yang tidak boleh membukanya. Seseorang
//! disebut bila teks memuat `@` diikuti email lengkapnya, bagian email sebelum `@` (mis.
//! `@dewi`), atau nama lengkapnya (mis. `@Dewi Dev`), tanpa membedakan huruf besar, dan karakter
//! sesudahnya bukan huruf/angka. Nama dipakai apa adanya karena editor menyisipkan nama orang.

use sea_orm::{ConnectionTrait, DbErr};
use uuid::Uuid;

use crate::models::{project_access, users, workspaces::WorkspaceId};

/// Batas kandidat yang diperiksa per teks.
const CANDIDATES: u64 = 2000;

/// User yang disebut di `body` (unik, urut nama).
///
/// # Errors
///
/// Error database.
pub async fn mentioned<C: ConnectionTrait>(
    db: &C,
    wid: WorkspaceId,
    project_id: Uuid,
    body: &str,
) -> Result<Vec<users::Model>, DbErr> {
    if !body.contains('@') {
        return Ok(Vec::new());
    }
    let lower = body.to_lowercase();
    let candidates =
        project_access::users_with_access(db, wid, project_id, None, CANDIDATES).await?;
    Ok(candidates
        .into_iter()
        .filter(|u| is_mentioned(&lower, u))
        .collect())
}

fn is_mentioned(lower_body: &str, user: &users::Model) -> bool {
    let email = user.email.to_lowercase();
    let local = email.split('@').next().unwrap_or("").to_string();
    let name = user.name.trim().to_lowercase();
    [email, local, name]
        .iter()
        .filter(|handle| !handle.is_empty())
        .any(|handle| contains_handle(lower_body, handle))
}

/// `@handle` muncul dengan batas kata sesudahnya: akhir teks, spasi/tanda baca, atau titik
/// akhir kalimat (titik yang diikuti huruf berarti email/nama lain yang lebih panjang).
fn contains_handle(body: &str, handle: &str) -> bool {
    let needle = format!("@{handle}");
    body.match_indices(&needle).any(|(i, _)| {
        let rest = &body[i + needle.len()..];
        let mut chars = rest.chars();
        match chars.next() {
            None => true,
            Some('.') => chars.next().is_none_or(|c| !c.is_alphanumeric()),
            Some(c) => !c.is_alphanumeric() && !matches!(c, '@' | '_' | '-'),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_needs_word_boundary() {
        assert!(contains_handle("halo @dewi, cek ya", "dewi"));
        assert!(contains_handle("halo @dewi dev.", "dewi dev"));
        assert!(contains_handle("@dewi", "dewi"));
        assert!(!contains_handle("halo @dewianti", "dewi"));
        assert!(!contains_handle("halo @dewi.dev@x.id", "dewi"));
        assert!(contains_handle("selesai @dewi.", "dewi"));
    }
}
