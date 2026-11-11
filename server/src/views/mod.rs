//! Fungsi render per halaman: menyiapkan data view dan memilih template.
pub mod account;
pub mod auth;
pub mod dev;
pub mod errors;
pub mod flash;
pub mod form;
pub mod invitations;
pub mod job_titles;
pub mod layout;
pub mod members;
pub mod roles;
pub mod workspaces;

/// Header untuk halaman yang membawa token di URL (reset, verifikasi, undangan, login dengan
/// `return_to` halaman undangan): `Referer` ke situs lain tidak boleh membawanya (rules §7).
pub(crate) const NO_REFERRER: (&str, &str) = ("referrer-policy", "no-referrer");
