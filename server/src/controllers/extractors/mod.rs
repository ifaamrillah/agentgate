//! Extractor aplikasi (rules §6): langkah yang berulang di banyak handler ditulis sekali di sini.
pub mod client_ip;
pub mod current_user;
pub mod id_or_404;
pub mod project_access;
pub mod request_id;
pub mod require;
pub mod tenant;
pub mod valid_form;

pub use client_ip::ClientIp;
pub use current_user::{CurrentUser, MaybeCurrentUser, sign_in_redirect};
pub use id_or_404::{IdOr404, form_id};
pub use project_access::{ProjectAccess, ProjectAdmin};
pub use request_id::RequestId;
pub use require::{PermissionMarker, Require};
pub use tenant::Tenant;
pub use valid_form::ValidForm;
