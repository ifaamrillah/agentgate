//! Satu tempat yang membangun data sidebar (`views::layout::AppShell`) untuk halaman ber-layout
//! aplikasi (rules §6: data layout lewat satu fungsi, bukan dirakit di setiap handler).
//!
//! Saat ini hanya berisi user. Daftar workspace, workspace aktif, izin Pengaturan (M1 Task 9),
//! dan project yang bisa diakses (Task 12) diisi di sini lewat setter `AppShell::with_*`.
use crate::{models::users, views::layout::AppShell};

#[must_use]
pub fn app_shell(user: &users::Model) -> AppShell {
    AppShell::for_user(user)
}
