//! Tes integrasi binary `agentgate` melawan server API palsu (satu binary tes).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tes boleh panic (rules §2).

mod hook;
mod install;
mod login;
mod runner;
mod support;
mod version;
