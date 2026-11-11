//! Helper bersama tes (rules §9): builder data dan helper login ditambahkan di sini, bukan
//! disalin per file tes. Builder datang bersama model-nya (M1 Task 3 dan seterusnya).
#![allow(dead_code)] // helper dipakai sebagian tes saja; yang belum dipakai tidak boleh memicu warning.

/// Password tes yang memenuhi aturan panjang (12–128 karakter). Satu nilai untuk semua tes
/// supaya `sign_in_as` dan builder user tidak berbeda pendapat.
pub const TEST_PASSWORD: &str = "correct horse battery staple";
