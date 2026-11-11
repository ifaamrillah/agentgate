#![allow(unsafe_code)] // set_var/remove_var di tes kontrak config: tes berjalan satu thread (bin/test --test-threads=1).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tes boleh panic (rules §2).
mod contracts;
mod mailers;
mod models;
mod requests;
mod support;
