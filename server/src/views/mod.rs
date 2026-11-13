//! Fungsi render per halaman: menyiapkan data view dan memilih template.
pub mod account;
pub mod audit;
pub mod auth;
pub mod datatable;
pub mod dev;
pub mod device;
pub mod drafts;
pub mod errors;
pub mod flash;
pub mod form;
pub mod invitations;
pub mod job_titles;
pub mod layout;
pub mod members;
pub mod nav;
pub mod oauth;
pub mod palette;
pub mod personas;
pub mod projects;
pub mod roles;
pub mod skills;
pub mod teams;
pub mod tickets;
pub mod time;
pub mod ui;
pub mod workflow;
pub mod workspaces;

/// Header untuk halaman yang membawa token di URL (reset, verifikasi, undangan, login dengan
/// `return_to` halaman undangan): `Referer` ke situs lain tidak boleh membawanya (rules §7).
pub(crate) const NO_REFERRER: (&str, &str) = ("referrer-policy", "no-referrer");
