//! Lapisan data Agent Gate: entity SeaORM (`models::_entities`), logika model, dan utilitas
//! `common`. Crate terpisah dari `agentgate-server` supaya mengubah controller, service, atau view
//! tidak meng-compile ulang ~24 ribu baris model, dan supaya arah dependensi `common` → `models`
//! → layer lain dijamin compiler (rules §14 "Memecah crate").
//!
//! Crate server me-re-export kedua modul (`agentgate_server::{common, models}`), jadi path
//! `crate::models::...` dan `crate::common::...` di server tetap berlaku.

pub mod common;
pub mod models;
