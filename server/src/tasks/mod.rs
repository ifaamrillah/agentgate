//! Task CLI (`bin/loco task <nama> ...`). Badan task tipis; logika ada di model/service
//! (rules §8). Output: banner fase, baris `[OK ]` / `[FAIL]` / `[SKIP]`, dan hitungan akhir.
pub mod cleanup;
#[cfg(debug_assertions)]
pub mod dev_login;
pub mod export_sweep;
pub mod knowledge_audit;
pub mod user_create;
pub mod user_set_password;

use loco_rs::{Error, Result, task::Vars};

/// Argumen `key:value` wajib. Spasi di tepi dibuang.
fn required<'a>(vars: &'a Vars, key: &str) -> Result<&'a str> {
    let value = vars.cli_arg(key)?.trim();
    if value.is_empty() {
        return Err(Error::string(&format!("the argument {key} is empty")));
    }
    Ok(value)
}

fn banner(title: &str) {
    println!("== {title} ==");
}

fn ok(message: &str) {
    println!("[OK ] {message}");
}

fn fail(message: &str) {
    println!("[FAIL] {message}");
}

fn skip(message: &str) {
    println!("[SKIP] {message}");
}

/// Baris hitungan akhir.
fn summary(ok: usize, failed: usize, skipped: usize) {
    println!("{ok} ok, {failed} failed, {skipped} skipped");
}
