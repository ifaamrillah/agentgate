# Rust Loco Project Rules

Konvensi untuk project Loco (Rust) fullstack dengan server-side rendering.
Taruh file ini sebagai `CLAUDE.md` (atau `docs/conventions.md`), lalu sesuaikan per project.

**Cara memakai dokumen ini**
- Ganti placeholder `<app>` dengan nama aplikasi (huruf kecil, `[a-z0-9_]`), mis. untuk nama
  database (`<app>_dev`), folder cache (`~/.cache/<app>/`), dan binary CLI (`<app>-cli`).
- Bagian yang diawali **> Opsional** hanya berlaku bila project memakai fiturnya. Hapus bagian itu
  beserta rujukannya (ditandai "bila memakai modul ...") bila tidak dipakai. Modul opsional:
  **Vector** (pgvector dan embedding), **Storage** (upload file disk/S3), **Worktree** (kerja
  paralel dengan git worktree).
- Versi yang disebut (Loco, SeaORM, Postgres, HTMX, Tailwind, Basecoat) adalah baseline saat dokumen
  ini ditulis. Project baru memakai versi terbaru yang kompatibel dan memperbarui angka di sini.

Prinsip: ikuti idiom Loco dulu, tambahkan aturan hanya kalau ada alasan konkret.
Setiap aturan yang ada karena sesuatu pernah rusak menyebutkan alasannya, karena aturan
tanpa alasan akan dihapus oleh orang berikutnya.

**Loco dipakai sebagai toolkit, tidak semua idiomnya diikuti.** Yang dipakai: routing dan
controller, view engine Tera, queue Postgres, worker, scheduler, mailer, storage, tasks, config
per environment, CLI migrasi, dan helper tes. Yang sengaja tidak dipakai: auth bawaan (JWT), id
integer hasil generator, dan generator scaffold/controller (sejak Loco 1.0 selalu menghasilkan
JSON API atau frontend React). Penyimpangan ini disengaja; bagian terkait menjelaskan alasannya.

**Dokumen ini menang atas panduan bawaan Loco.** `loco new` menulis `AGENTS.md` dan skill
`.claude/skills/loco/` ke dalam project. Isinya berguna untuk API Loco dan SeaORM, tetapi bila
bertentangan dengan dokumen ini (JWT, id integer, scaffold, frontend React), dokumen ini yang
berlaku.

---

## 1. Stack dan toolchain

- **Loco 1.3.x + SeaORM 2.0 + Axum, PostgreSQL 18, Rust edition 2024.** Pin toolchain di
  `rust-toolchain.toml` (versi stable spesifik, bukan `stable`, minimal 1.94 karena itu MSRV
  Loco 1.x) dan samakan dengan versi di `Dockerfile`. `Cargo.lock` di-commit. Starter `loco new`
  1.3 masih menulis `edition = "2021"`; ubah ke `2024` saat project dibuat.
- **Build khusus Postgres.** `loco-rs` dengan `default-features = false` dan hanya feature yang
  dipakai. Feature `db-sqlite` (SQLite) dan `worker_redis` **tidak** diaktifkan: SQLite dan Redis
  tidak dipakai di environment mana pun. Auth memakai feature `auth_session` (sesi server-side di
  database, §7), bukan JWT. Tanpa `db-sqlite`, build juga tidak butuh `libsqlite3-sys` dan
  toolchain C-nya.
- **Fullstack server-side rendering dengan navigasi ala SPA.** Project dibuat dengan `loco new`
  memakai aset server-side. Seluruh HTML dirender di server dengan Tera 2, dan aset statis
  disajikan dari `assets/static`.
  - Navigasi terasa seperti SPA (tanpa full page reload) lewat HTMX 4: `hx-boost` di layout, swap
    parsial untuk interaksi kecil. Browser hanya menukar HTML yang dikirim server; ia tidak merender
    halaman sendiri.
  - **Yang tidak dipakai:** framework client-side rendering (React/Vue/Svelte), termasuk frontend
    React yang ditawarkan generator Loco; SSR dengan hydration ala Next.js/Nuxt (butuh runtime JS
    di server); bundler JS; dan Node (di dev, CI, maupun image).
  - **Satu-satunya langkah build frontend adalah CSS**: Tailwind CSS v4 lewat *standalone CLI*
    (satu binary, tanpa Node). JavaScript tidak di-build; semua file JS dimuat apa adanya dari
    `assets/static`.
  - **UI kit default: Basecoat 1.1.x** (komponen ala shadcn/ui berbasis class CSS). **Ikon default:
    Tabler Icons** (outline, grid 24×24, stroke 2px). Project boleh memilih UI kit berbasis class
    CSS atau set ikon SVG lain; aturan lapisan, vendoring, dan pemakaian di §6 tetap berlaku sama,
    hanya nama class dan file yang berganti. Satu project memakai satu UI kit dan satu set ikon.
  - **HTMX 4.x adalah satu-satunya library navigasi dan partial.** Turbo tidak dipakai, termasuk
    bersamaan dengan HTMX: keduanya mencegat link, form, dan history browser, sehingga terjadi
    navigasi ganda. Kecepatan bukan alasan memilih Turbo; prefetch tersedia lewat extension
    `hx-preload` HTMX (§6).
  - **Semua aset pihak ketiga di-vendor dengan versi dipin**: HTMX dan extension-nya, JS UI kit
    (`assets/static/vendor/`), dan CSS sumber UI kit (`assets/css/vendor/`), dengan versi di nama
    file atau folder (`htmx-4.x.y.min.js`, `hx-preload-4.x.y.js`, `basecoat-1.1.x/`). Tidak dari
    CDN, supaya aplikasi tidak bergantung pada host pihak ketiga dan versi tidak berubah diam-diam.
    Upgrade dilakukan dengan mengganti file dan nama versinya dalam satu commit.
  - JS vanilla hanya untuk perilaku kecil yang tidak bisa dilakukan HTMX (aturan file di §6).
- **Postgres di semua environment**, termasuk dev dan test. Jangan pakai SQLite di dev lalu
  Postgres di prod: perbedaan perilaku (tipe, constraint, `uuid`, `jsonb`) baru ketahuan di produksi.
  Dev memakai image Postgres dengan major version yang sama dengan production.
- **Bila memakai modul Vector**: pgvector terpasang di setiap environment sejak modul itu dipakai.
  Dev memakai image `pgvector/pgvector:pg18` (bukan `postgres:18`); staging dan production memakai
  layanan Postgres yang menyediakan extension `vector`. Database tanpa pgvector gagal di migrasi
  yang memasangnya, bukan di fitur pertama yang memakai vektor. Aturan pemakaiannya di §4.
- **Tanpa Redis.** Background job memakai queue Postgres (`queue.kind: Postgres`), cache memakai
  driver in-memory atau null, rate limit login memakai tabel Postgres (§7).
- **Setiap dependency di `Cargo.toml` punya komentar satu baris** tentang kegunaannya.
  Crate yang tidak bisa dijelaskan, dihapus. Aktifkan hanya feature yang dipakai
  (`default-features = false` bila masuk akal).
- Dev tools wajib: `cargo fmt`, `cargo clippy`, `cargo deny` (advisory keamanan, lisensi, crate
  duplikat, dan crate terlarang dalam satu tool, diatur di `deny.toml`), `cargo insta` (snapshot
  test), `watchexec` (restart otomatis), `cargo nextest` (tes paralel, §9), `cargo hakari`
  (feature dependency seragam, §14), dan `sccache` bila memakai modul Worktree. Alasan dan
  pengaturan kecepatan build di §14.

## 2. Lint dan gaya kode

- `rustfmt` default, tanpa override. `clippy` dijalankan dengan `-D warnings`.
- Di `Cargo.toml`:
  ```toml
  [lints.clippy]
  unwrap_used = "deny"   # panic di request handler = 500 tanpa konteks
  expect_used = "deny"
  ```
  Boleh di-`allow` di `tests/` saja.
- Jangan `unwrap()`/`expect()` di kode non-test. Propagasi dengan `?` ke `loco_rs::Result`.
- Error domain didefinisikan dengan `thiserror` di modul yang memilikinya. Konversi dari error lain
  memakai `#[from]`, bukan `impl From` yang ditulis tangan berulang. Pemetaan ke HTTP ditulis
  **sekali** per error domain (§6), bukan di setiap handler. Jangan mengembalikan `String` sebagai error.
- **`Display` sebuah error adalah pesan log** (bahasa Inggris, untuk developer), bukan teks yang
  dilihat user. Teks untuk user selalu lewat i18n (§6).
- **Satu aturan, satu implementasi.** Kode yang harus selalu sama di dua tempat dijadikan satu
  fungsi, bukan dua salinan yang dijaga komentar "keep in step": salinan semacam itu pasti
  menyimpang, dan biasanya yang menyimpang duluan adalah teks yang dilihat user.
- **Jelas lebih penting daripada cerdik.** Tidak ada macro atau trik generik tanpa kebutuhan yang
  terukur; `match` biasa lebih mudah dibaca dan dirawat.
- Enum yang butuh konversi ke/dari string atau daftar semua variannya memakai derive (`strum`:
  `EnumString`, `IntoStaticStr`, `EnumIter`; untuk enum database, `DeriveActiveEnum` + `EnumIter`).
  Tidak ada `as_str`/`parse`/`ALL` yang ditulis tangan sebagai `match` paralel: menambah varian
  lalu lupa satu `match` adalah bug yang compiler tidak bisa tangkap.

## 3. Layout repository

```
models/               crate lapisan data (agentgate-models, §14 "Memecah crate"); di-re-export
  src/common/         oleh crate server sebagai `crate::common` dan `crate::models`
  src/models/         (src/models di crate server adalah symlink ke sini, untuk `bin/loco db entities`)
    _entities/        DIGENERATE oleh `bin/loco db entities`, JANGAN diedit
    <name>.rs         logika satu entitas (impl Model/ActiveModel)
src/
  app.rs              Hooks: routes, workers, tasks, seed, truncate, initializers
  controllers/        adapter HTTP tipis (lihat §6)
  services/<domain>/  workflow lintas beberapa model (lihat §5), opsional
  services/auth/      password, sesi, reset password, rate limit login (lihat §7)
  services/embeddings/ client provider embedding (modul Vector, lihat §4)
  views/              fungsi render: siapkan data dan pilih template
  mailers/            mailer Loco dan template email (lihat §8)
  workers/            background job (termasuk pengiriman email)
  tasks/              operasional one-off / CLI
  initializers/       termasuk view engine (Tera)
  common/             helper netral tanpa ketergantungan ke layer lain (teks, id, settings §10)
  fixtures/           seed data dev saja
assets/
  views/<resource>/   template Tera per halaman
  views/layouts/      layout dasar
  views/components/   komponen Tera 2 untuk UI yang berulang (lihat §6)
  views/dev/          katalog komponen `/_dev/ui` (lihat §6)
  css/app.css         entry CSS: Tailwind + UI kit + token tema + import components/
  css/components/     override class UI kit dan class `app-*`, satu file per komponen
  css/vendor/         CSS sumber UI kit (versi dipin, tidak pernah diedit)
  icons/              SVG ikon yang dipakai saja + file VERSION (lihat §6)
  static/             output CSS (DIGENERATE, gitignored), gambar
  static/js/          perilaku JS sendiri, satu file per perilaku (lihat §6)
  static/vendor/      HTMX, extension HTMX, JS UI kit (versi dipin)
  i18n/               file terjemahan
storage/              file upload di development (modul Storage, gitignored, lihat §8)
migration/            migrasi SeaORM
config/               development.yaml, test.yaml, staging.yaml, production.yaml
tests/                requests/, models/, workers/, tasks/, support/ (builder data tes)
bin/                  dev, test, loco, ci, tailwind (+ worktree, worktree-id bila memakai modul Worktree)
.claude/skills/loco/  skill Loco bawaan `loco new` (dokumen ini menang bila bertentangan)
.worktreeinclude      modul Worktree: file gitignored yang di-copy ke worktree baru
.worktree             modul Worktree: identitas dan port worktree (gitignored)
worktrees/            modul Worktree: folder semua worktree (gitignored, lihat §11)
```

- **`bin/dev`, `bin/test`, dan `bin/loco` adalah pintu masuk resmi**, bukan `cargo loco` /
  `cargo test` langsung, karena ketiganya yang men-set environment (`DATABASE_URL`, `PORT`, suffix
  cookie) dari satu env loader yang sama: dari `.env` lokal, atau dari `.worktree` bila memakai
  modul Worktree (§11). `bin/dev` juga menjalankan Tailwind dalam mode `--watch` bersamaan dengan
  server, dan me-restart server saat kode Rust, config, ikon, atau terjemahan berubah (§14).
- **`bin/tailwind`** mengunduh Tailwind standalone CLI dengan versi yang dipin di script itu (satu
  tempat) ke cache per user di luar repo (`~/.cache/<app>/tailwind-<versi>`), memverifikasi
  checksum-nya, lalu menjalankannya. Semua checkout memakai binary yang sama tanpa unduh ulang;
  versi di nama file membuat branch yang meng-upgrade Tailwind tidak mengganggu branch lain.
  `bin/dev`, `bin/ci`, dan Dockerfile memanggil script yang sama.
- `bin/ci` menjalankan, berurutan: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo deny check`, cek arah dependensi (di bawah), build CSS (`bin/tailwind --minify`), `bin/test`.
  CI remote memanggil script yang sama, bukan mendefinisikan ulang langkahnya.

**Arah dependensi antar-layer**

Semua kode ada di satu crate, jadi Rust tidak mencegah import ke arah mana pun. Arah ini dijaga
aturan dan dicek otomatis:

| Layer | Boleh memakai |
|---|---|
| `controllers/` | `services`, `models`, `views`, `common` |
| `services/`, `workers/`, `tasks/`, `mailers/` | `services` lain, `models`, `common` |
| `views/` | `models`, `common` |
| `models/` | `common` |
| `common/` | tidak ada layer aplikasi |

- **Konsep domain tinggal di `models/`**, bukan di service yang kebetulan pertama memakainya.
  Contoh: peran user beserta izinnya adalah konsep domain; `services/auth` hanya me-resolve user
  dari sesi. Model yang butuh tipe dari `services/` adalah tanda tipe itu salah tempat.
- **Tipe yang dibutuhkan view tidak didefinisikan di controller.** Parameter filter yang dipakai
  untuk merender (rentang waktu, urutan) tinggal di `views/` atau `common/`; extractor seperti
  `CurrentUser` tetap di `controllers/`, dan view menerima data biasa (struct view), bukan extractor.
- Helper yang dipakai lebih dari satu layer (encode komponen URL, normalisasi teks) tinggal di `common/`.
- `bin/ci` menjalankan pengecekan arah dependensi berbasis pencarian teks: `crate::controllers`
  tidak boleh muncul di `models/`, `services/`, `views/`, `common/`; `crate::services` tidak boleh
  di `models/`, `views/`, `common/`; `crate::views` tidak boleh di `models/`, `services/`,
  `common/`. Pola mencari path di mana pun di baris kode (termasuk di dalam ekspresi dan
  `pub use`), juga path relatif `super::...::controllers`, dan mengabaikan baris komentar.
  Melanggar berarti CI merah, bukan catatan review.
- Bila pengecekan salah tangkap atau ada pelanggaran yang lolos, **perbaiki polanya**, jangan
  matikan pengecekannya. Jaminan penuh dari compiler baru didapat dengan memecah project menjadi
  beberapa crate dalam satu workspace; itu sengaja belum dilakukan sampai anggaran waktu build
  terlampaui (§14).

- README singkat berisi: cara menjalankan Postgres lokal (image dengan pgvector bila memakai modul
  Vector), mail sink lokal (§8), `bin/loco db migrate`, `bin/loco db seed`, `bin/dev`,
  `bin/loco doctor` untuk cek koneksi, cara login di dev (§7), dan `bin/worktree new` bila memakai
  modul Worktree.

## 4. Database dan migrasi

**Primary key: UUID v7 di semua tabel**
- Setiap tabel memakai `id uuid PRIMARY KEY DEFAULT uuidv7()`. Tidak ada id integer, tidak ada
  kolom `pid` terpisah. UUID v7 time-sortable, aman ditampilkan di URL, dan tidak membocorkan
  jumlah baris. Yang **ikut terbaca** dari id adalah waktu pembuatan baris
  (`uuid_extract_timestamp()`); bila waktu pembuatan suatu entitas rahasia, entitas itu tidak
  menampilkan id-nya di URL.
- Generator Loco (`generate model`/`migration`) menghasilkan id `BIGINT` auto-increment dan
  reference `BIGINT`. **Setiap migrasi hasil generator diedit sebelum dijalankan**: `id` menjadi
  UUID, semua kolom `*_id` menjadi `uuid`.
  ```rust
  ColumnDef::new(Posts::Id).uuid().not_null().primary_key()
      .default(Expr::cust("uuidv7()"))
  ```
- Di model, `id` juga diisi di `before_save` saat insert dengan `Uuid::now_v7()`, sehingga
  instance selalu punya id sebelum disimpan. Default DB tetap ada untuk insert lewat raw SQL.
- Tabel `users` dari starter ikut dikonversi: `id` menjadi UUID, `pid` dan `api_key` dihapus, dan
  kolom token starter (token reset password, token verifikasi email, magic link) dihapus dan
  diganti tabel `user_tokens` yang hanya menyimpan digest (§7).
- Jangan bergantung pada urutan UUID: order selalu pada kolom waktu eksplisit dengan `id` sebagai tiebreaker.

**Kolom**
- Timestamp selalu `timestamptz` (`tstz!` / `tstz`), simpan dalam UTC. `created_at`/`updated_at`
  ada di setiap tabel (default generator).
- Timestamp domain berakhiran `_at`, nullable, tanpa default. NULL berarti "belum terjadi".
  Lebih baik timestamp nullable daripada boolean flag, karena juga menyimpan *kapan*
  (`email_verified_at`, `disabled_at`).
- Boolean selalu NOT NULL dengan default eksplisit. Counter: integer, default 0, NOT NULL.
- Enum disimpan sebagai `string` NOT NULL dengan CHECK constraint (lihat di bawah), dipetakan ke
  Rust enum (SeaORM `DeriveActiveEnum` dengan `rs_type = "String"`). Jangan integer-backed:
  database tetap terbaca dan backfill SQL bisa membandingkan literal. **Tanpa pengecualian**:
  tidak ada kolom enum yang disimpan sebagai `String` dengan `key()`/`parse()` manual. Dua pola
  untuk hal yang sama berarti dua tempat untuk bug.
- `jsonb` NOT NULL default `'{}'`, selalu di-deserialize ke struct bertipe di model, bukan
  `serde_json::Value` mentah di logika bisnis.

**Index dan constraint**
- Setiap reference punya foreign key; `ON DELETE` ditulis eksplisit bila bukan RESTRICT, dan
  komentarnya menyebut kenapa.
- Index mengikuti bentuk query, termasuk arah sort. Partial index untuk predikat timpang dan
  invariant bisnis ("satu baris aktif per parent").
- Keunikan case-insensitive pakai expression index `lower(col)`, dipasangkan dengan normalisasi di model.
- **Kolom enum punya CHECK constraint** `CHECK (status IN ('pending', 'active', 'rejected'))`.
  Alasannya: raw SQL, backfill, dan perbaikan data manual melewati validator Rust, dan nilai yang
  salah eja di kolom enum membuat setiap pembacaan baris itu gagal deserialize. Menambah atau
  menghapus varian berarti satu migrasi yang mengganti constraint-nya, di commit yang sama dengan
  perubahan enum Rust. Ini salah satu hal "dua tempat harus sepakat" yang memang tidak bisa
  disatukan (§13), jadi dijaga tes: untuk setiap enum database, satu tes menyimpan setiap varian
  dari `EnumIter` dan satu literal tak dikenal lewat raw SQL yang harus ditolak.
- CHECK hanya untuk himpunan nilai enum. Aturan nilai lain (panjang, format, rentang) ada di
  validator; invariant lintas baris di unique/partial index.

> **Opsional, modul Vector (pgvector dan embedding).** Hapus subbagian ini bila project tidak
> memakai embedding atau pencarian vektor.

**Vector dan embedding (pgvector)**
- Extension `vector` dipasang oleh migrasi (`CREATE EXTENSION IF NOT EXISTS vector`), sebaiknya
  migrasi pertama bila sejak awal sudah direncanakan. Database test yang dibuat kosong
  mendapatkannya lewat migrasi itu. Di staging dan production, bila user aplikasi tidak berhak
  membuat extension, extension dipasang sekali oleh pemilik database sebelum migrasinya; migrasinya
  tetap memakai `IF NOT EXISTS`.
- SeaORM diaktifkan dengan feature `postgres-vector` (tipe `PgVector`). `sea-orm-cli` yang dipakai
  `bin/loco db entities` juga di-install dengan feature itu: tanpanya kolom vector ter-generate
  sebagai `String`, dan kesalahannya baru terlihat saat query.
- **Dimensi selalu eksplisit** di migrasi (`vector(1536)`), bukan `vector` tanpa angka. Index butuh
  dimensi tetap, dan vektor berdimensi salah ditolak database saat insert, bukan tersimpan diam-diam.
  Index HNSW untuk `vector` dibatasi 2.000 dimensi; model dengan dimensi lebih besar butuh
  `halfvec`, dan dukungan SeaORM untuk tipe itu dicek sebelum modelnya dipilih.
- **Setiap baris embedding menyimpan model yang membuatnya** (`embedding_model`, string) dan
  `embedded_at`. Embedding dari dua model berbeda tidak bisa dibandingkan; kolom ini yang
  menunjukkan baris mana yang harus di-embed ulang saat model diganti. Query similarity selalu
  memfilter model yang sama dengan model vektor query.
- Embedding disimpan di tabel sendiri (mis. `document_chunks`: `document_id`, teks potongan,
  `embedding`), bukan kolom di tabel domain utama: satu dokumen biasanya menjadi banyak potongan,
  dan membaca baris domain tidak perlu ikut membawa vektor besar.
- **Pembuatan embedding adalah efek samping "wajib terjadi"** (§5): dikerjakan worker, tidak pernah
  di dalam request; `embedded_at IS NULL` menandai yang tertunda; job idempoten; scheduler menyapu
  baris yang tertahan. Request user tidak pernah menunggu provider embedding.
- Client provider embedding tinggal di `services/embeddings/`, diinjeksi lewat konstruktor, dengan
  timeout dari config dan API key hanya dari env var. Tes memakai double yang menghasilkan vektor
  deterministik; tidak ada tes yang memanggil provider sungguhan.
- **Index HNSW memakai operator class yang sama dengan operator query**: `vector_cosine_ops` untuk
  `<=>`, `vector_l2_ops` untuk `<->`, `vector_ip_ops` untuk `<#>`. Operator yang tidak cocok dengan
  index membuat Postgres diam-diam melakukan scan penuh. Index pada tabel yang sudah berisi dibuat
  `CONCURRENTLY`.
- Query similarity ditulis sekali di model (mis. `nearest_chunks(embedding, model, limit)`), dengan
  raw SQL mengikuti aturan SeaORM 2.0 di bawah; vektor query selalu parameter bind.

**Migrasi**
- Migration-first: ubah schema lewat `bin/loco g migration`, jalankan `bin/loco db migrate`,
  lalu `bin/loco db entities`. Hasil generate entities di-commit bersama migrasinya.
- Nama migrasi mengikuti pola Loco (`CreatePosts`, `AddPublishedAtToPosts`, `AddUserRefToPosts`)
  atau menyebut niat, bukan mekanisme (`AllowNullTicketCreatedBy`).
- `down` selalu ada. Kalau sengaja tidak bisa di-rollback, kembalikan error yang menjelaskan kenapa.
- Backfill yang terikat schema ditulis sebagai raw SQL, bukan lewat model: struct entity akan
  berubah, backfill harus tetap melakukan hal yang sama seperti saat pertama dijalankan. Backfill
  tabel kecil boleh di dalam migrasi. **Backfill tabel besar adalah task** (§8) yang memproses per
  batch (mis. 1.000 baris per transaksi), idempoten, dan bisa dilanjutkan bila terhenti: satu
  `UPDATE` besar di migrasi mengunci baris lama, membengkakkan tabel, dan menahan deploy.

**Migrasi tanpa downtime**

Migrasi dijalankan sebelum container baru naik (§12), jadi selama deploy **kode rilis lama berjalan
di atas skema baru**. Setiap migrasi wajib kompatibel dengan kode rilis sebelumnya:
- **Expand/contract.** Perubahan yang merusak kode lama dipecah ke beberapa rilis:
  - rename kolom: tambah kolom baru → kode menulis ke keduanya dan backfill → kode membaca kolom
    baru → rilis berikutnya menghapus kolom lama;
  - hapus kolom: rilis pertama berhenti memakainya (entity tidak lagi membacanya), rilis berikutnya
    baru drop;
  - kolom `NOT NULL` baru: tambah sebagai nullable atau dengan default, backfill, baru pasang
    `NOT NULL`;
  - menyempitkan CHECK enum: kode berhenti menulis varian itu dan datanya dimigrasi dulu.
- **Setiap migrasi memasang `SET lock_timeout = '5s'`** (dan `statement_timeout` yang wajar) di
  awal. DDL yang menunggu lock memblokir semua query lain di tabel itu di belakangnya; lebih baik
  migrasi gagal cepat lalu diulang daripada aplikasi berhenti.
- Foreign key dan CHECK baru di tabel besar dipasang `NOT VALID`, lalu `VALIDATE CONSTRAINT` di
  migrasi terpisah: validasi tidak mengunci tulis.
- Review migrasi menjawab satu pertanyaan tertulis: "apakah kode rilis sebelumnya tetap jalan
  setelah migrasi ini?"
- **Raw SQL di SeaORM 2.0**: method statement mentah berakhiran `_raw`, dan string SQL yang
  dibentuk saat runtime harus dibungkus `AssertSqlSafe`. Bungkus itu adalah klaim bahwa string
  tidak mengandung input user; nilai dari user selalu lewat parameter bind, tidak pernah lewat
  `format!`.
- `auto_migrate: false` di production; migrasi dijalankan sebagai langkah deploy eksplisit.
- Index pada tabel besar dibuat `CONCURRENTLY` (raw SQL, di luar transaksi) dengan komentar alasannya.

**Seed**
- `src/fixtures/*.yaml` + hook `seed` hanya untuk data dev yang realistis (bukan satu baris).
  Id di fixture ditulis sebagai UUID literal. **Seed tidak berisi password maupun hash password**:
  user seed dimasuki lewat `dev_login` (§7), dan developer yang ingin mencoba form login memberi
  password ke user seed lewat task `user:set_password`. Seed menyediakan user dev dengan peran yang
  umum dipakai (bila project punya peran); README menyebut email user seed.

## 5. Models dan services

Loco menganut *fat models, slim controllers*. Aturan pembagiannya:

- **Logika satu entitas** (create, find, validasi, transisi status) ada di `src/models/<name>.rs`
  sebagai `impl Model` / `impl ActiveModel`. Ini default.
- **Workflow lintas beberapa entitas** atau yang menyentuh dunia luar (panggil API eksternal,
  kirim email, enqueue job) masuk `src/services/<domain>/`. Jangan buat service untuk satu query
  sederhana.
- Urutan isi file model: struct validator → `impl Validatable` → `impl ActiveModelBehavior`
  (hooks) → `impl Model` (query/kelas) → `impl ActiveModel` (mutasi) → fungsi privat.
- **Normalisasi di model**, tidak di controller: trim nama, `trim().to_lowercase()` email,
  dilakukan di `before_save` atau constructor.
- **Validasi** pakai `validator` + `Validatable`. Satu field per aturan. Validator dan error domain
  mengembalikan **kode dan argumen** (`field = "name"`, `code = "name-taken"`, `{ scope = ... }`),
  bukan kalimat; view menerjemahkan kode itu lewat i18n (§6). Keunikan selalu didukung unique index di DB.
- **Aturan yang sama untuk entitas berbeda memakai satu validator berparameter**: panjang nama dan
  keterangan, karakter tak terlihat, normalisasi teks opsional, pola pencarian `ILIKE` ada di
  `common/text.rs`, dengan nama entitas sebagai parameter. Entitas baru memakai yang sudah ada,
  tidak menyalin. Batas panjang adalah konstanta di satu tempat.
- **Query bernama sesuai filter**, bukan pemanggil: `find_active`, `newest_first`,
  `assigned_to(user_id)`. Setiap order untuk paginasi punya total order (`created_at` lalu `id`).
- Paginasi: `query::paginate` (offset) cukup untuk halaman admin; keyset
  `(created_at, id) < ($1, $2)` untuk feed/infinite scroll.
- **Efek samping yang keluar proses** (panggilan API eksternal, email, job) dilakukan **setelah**
  `txn.commit()` berhasil, tidak di dalam transaksi atau hook `before_save`. Karena enqueue bisa
  gagal setelah data tersimpan, setiap efek samping masuk salah satu dari dua kelas, dan kelasnya
  ditulis di komentar:
  - **Wajib terjadi** (mis. memproses dokumen yang diupload, membuat embedding): statusnya disimpan
    di baris domain itu sendiri (`processed_at IS NULL`), job-nya idempoten, dan job scheduler
    berkala menyapu baris yang tertahan lebih lama dari batas wajar lalu meng-enqueue ulang.
    Enqueue yang gagal diselamatkan sapuan berikutnya, bukan hilang diam-diam.
  - **Best-effort** (mis. memperbarui ringkasan statistik, email reset password yang bisa diminta
    ulang user): kegagalan enqueue dicatat di log dengan konteks (entitas, id, aksi) dan tidak
    menggagalkan request.
- **Race guard**: efek yang harus terjadi sekali di-claim dengan update bersyarat
  (`update_many().filter(claimed_at.is_null())`) dan cek `rows_affected == 1`, bukan `if` pada field.
- Biarkan DB menegakkan invariant: tangkap `DbErr::sql_err() == Some(SqlErr::UniqueConstraintViolation(_))`
  lalu resolve ulang, bukan SELECT dulu (ada race window).
- Transaksi eksplisit dan jarang, hanya bila beberapa baris harus masuk bersamaan. Loop atas banyak
  record independen: satu transaksi per item.
- Soft delete (jika perlu): satu kolom `deleted_at` nullable, query `kept`/`trashed` eksplisit,
  partial index di sisi yang jarang.
- Service: struct dengan dependency di field, satu method publik `call`/`run`. Client eksternal
  diinjeksi lewat konstruktor (trait atau struct) supaya tes bisa mengganti dengan double.
- Setiap service dan model non-trivial dibuka dengan doc comment 3–10 baris: apa, kapan dipanggil,
  trade-off yang tidak jelas.

## 6. Controllers, routes, views (server-side rendering)

**Controllers**
- Controller adalah adapter tipis: ekstrak input → panggil model/service → render atau redirect.
  Tidak ada query SeaORM kompleks di controller.
- **Controller HTML ditulis tangan.** `generate scaffold`/`controller` di Loco 1.x hanya
  menghasilkan controller JSON API (atau frontend React), jadi tidak dipakai. Resource baru
  mengikuti pola resource yang sudah ada: controller, fungsi view, template, `_form.html`, dan tes.
- **"Tipis" ditegakkan lewat extractor dan konversi error, bukan niat baik.** Langkah yang
  berulang di banyak handler tidak ditulis ulang di setiap handler:
  - user saat ini → extractor `CurrentUser`;
  - izin → extractor guard (mis. `Require<ManageRoles>`) yang diletakkan **sebelum** extractor
    body. Axum menjalankan extractor berurutan, jadi pengecekan izin dijamin terjadi sebelum body
    dibaca dan divalidasi: user tanpa izin tidak pernah melihat pesan validasi;
  - id di path → extractor `IdOr404` (UUID tidak valid menjadi 404, sama seperti id yang tidak ada,
    bukan 400);
  - form yang tidak bisa di-parse → satu extractor `ValidForm<T>` yang memetakan rejection secara
    seragam, bukan `.map_err(...)` di setiap handler;
  - data layout (menu, user, flash) → satu fungsi render yang dipakai semua halaman, bukan
    dimuat manual di setiap handler;
  - error domain → HTTP lewat `impl From<DomainError> for loco_rs::Error` (atau `IntoResponse`)
    sekali per error, bukan `match` yang ditulis ulang di setiap handler.
- Handler hanya memuat data yang dipakai halaman atau tab yang dirender. Tidak ada fungsi
  "muat semuanya" yang dipakai bersama oleh semua tab.
- Input form selalu struct bertipe (`ValidForm<Params>`, `Query<Filter>`, `IdOr404`) dengan
  `serde` + `validator`. Nilai turunan (user saat ini, nilai tetap) di-set di kode, tidak pernah
  diterima dari request.
- Filter query-string divalidasi ke enum/himpunan yang dikenal; nilai tak dikenal diabaikan,
  bukan menghasilkan list kosong.
- **Alur tulis standar (POST-redirect-GET)**: sukses → redirect `303 See Other` ke halaman tujuan;
  gagal validasi → render ulang form yang sama dengan pesan error dan nilai yang sudah diisi
  (kecuali field password, §7), status `422`. Jangan redirect saat validasi gagal (input user hilang).
- Mutasi hanya lewat POST (atau method lain non-GET). **GET tidak pernah mengubah data**, termasuk
  logout, "tandai sudah dibaca", dan sejenisnya: aksi seperti itu adalah form POST dengan tombol,
  bukan link. Preload (§6) mengambil link sebelum diklik, sehingga GET yang mengubah data akan
  terpicu tanpa diklik.
- Otorisasi: **satu fungsi per aturan akses** (mis. `ensure_role_editable(&actor, &role)?`), dan
  semua handler yang menyentuh aturan itu memanggil fungsi yang sama, termasuk aksi kecil (hapus
  satu bagian, simpan satu relasi). Tidak ada pengecekan ad hoc yang merangkai kondisi sendiri.
  Halaman yang butuh login me-redirect ke halaman login; resource milik orang lain menghasilkan
  404, bukan 403 (tidak membocorkan keberadaan data). Pertanyaan peran ditanyakan ke model.

**Keamanan**
- Login, sesi, dan cookie diatur di §7. Tidak ada token yang disimpan di `localStorage`.
- **Proteksi CSRF lewat pengecekan asal request**, satu middleware untuk semua route, tanpa token
  per form. Loco tidak menyediakan proteksi CSRF bawaan. Request dengan method selain `GET`,
  `HEAD`, `OPTIONS` ditolak `403` bila:
  - header `Sec-Fetch-Site` ada dan nilainya bukan `same-origin` atau `none`, **termasuk
    `same-site`**; atau
  - `Sec-Fetch-Site` tidak ada, tetapi header `Origin` ada dan host-nya bukan host aplikasi.

  Request tanpa kedua header (klien non-browser) diloloskan. Pola ini sama dengan
  `http.CrossOriginProtection` di standard library Go.
- Kenapa `SameSite=Lax` tidak cukup: SameSite menganggap semua subdomain satu organisasi
  (`*.example.com`) satu situs. Aplikasi lain di subdomain itu yang punya celah XSS bisa mengirim
  POST ke aplikasi ini lengkap dengan cookie sesi. Karena itu `same-site` ditolak.
- Pengecualian middleware (mis. webhook dari server lain) didaftar eksplisit di satu tempat, masing-
  masing dengan komentar alasannya. Daftar ini dimulai kosong.
- Tes penolakan: POST dengan `Sec-Fetch-Site: cross-site` dan `same-site` → `403`; POST dengan
  `Origin` host lain → `403`; POST `same-origin` → lolos.
- **Security headers lewat middleware `secure_headers` Loco**, satu konfigurasi untuk semua route,
  dengan CSP ketat ditulis eksplisit (bukan preset yang mengizinkan `https:` apa pun):
  ```yaml
  middlewares:
    secure_headers:
      preset: github
      overrides:
        Content-Security-Policy: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'"
  ```
  - `script-src 'self'` berarti **tidak ada inline `<script>`, atribut `onclick`, maupun `hx-on`**.
    Semua perilaku JS ada di file `assets/static/js/` dengan atribut `data-*` (lihat di bawah); data
    untuk JS dikirim lewat atribut `data-*` atau `<script type="application/json">`. Inilah yang
    membuat XSS yang lolos autoescape tetap tidak bisa menjalankan skrip.
  - `style-src 'unsafe-inline'` diizinkan karena HTMX dan UI kit menyisipkan style inline; risikonya
    jauh lebih kecil daripada skrip.
  - `Strict-Transport-Security` hanya di staging dan production (development memakai http).
  - Menambah sumber eksternal (font, analytics, `form-action` ke identity provider saat SSO)
    berarti mengubah CSP di config ini, dengan komentar alasannya.
  - Satu tes request memeriksa header CSP ada di respons halaman.
- Tera autoescape aktif untuk `.html`. **Jangan pernah pakai filter `| safe` pada konten dari user.**
- Halaman error 404/500 adalah template sendiri berupa halaman lengkap dengan layout, karena HTMX 4
  juga men-swap respons error ke halaman (lihat navigasi di bawah). Production tidak pernah
  menampilkan backtrace.

**Routes**
- Dikelompokkan per audiens dengan `Routes::new().prefix(...)`; `bin/loco routes` harus
  terbaca rapi. Route non-obvious diberi komentar pemanggil dan alasannya.
- Pertahankan `/_ping` dan `/_health` bawaan Loco untuk health check.

**Views dan template (Tera 2)**
- `src/views/<resource>.rs` berisi fungsi render per halaman yang memilih template
  `assets/views/<resource>/<action>.html` dan menyiapkan datanya.
- **Jangan kirim entity utuh ke template.** Kirim struct view atau `data!` berisi field yang
  dipakai saja (mencegah bocornya `password_hash`, token, atau data sensitif lain ke HTML).
- Satu layout dasar per audiens (publik, aplikasi, admin); halaman `extends` layout dan
  mengisi block `title`.
- Partial diberi awalan `_` (`_form.html`, `_pagination.html`) dan dibuka dengan komentar yang
  menyebut variabel yang dibutuhkan.
- **Tera 2 gagal pada variabel yang tidak didefinisikan** (Tera 1 merender string kosong). Nilai
  yang memang opsional ditulis eksplisit dengan `| default(value="")`. Komentar pembuka partial
  adalah kontrak yang benar-benar ditegakkan.
- Contoh template Tera 1 (`{% macro %}`, `{% import %}`, akses array `v.0`) tidak dipakai, termasuk
  yang disarankan agent atau ditemukan di internet. Tera 2 memakai components dan `v[0]`.
- Respons partial HTMX (lihat navigasi di bawah) mengembalikan partial atau komponen yang sama
  dengan yang dipakai halaman penuh (`TeraView::render_component` untuk satu komponen), supaya
  markup tidak terduplikasi.

**UI: lapisan dan komponen**

UI kit (default Basecoat) adalah titik awal, bukan tampilan akhir: seiring waktu tampilan aplikasi
akan menyimpang darinya. Supaya penyimpangan itu tidak menjadi salinan markup dan class yang
tersebar di banyak halaman, UI dibagi menjadi lapisan, dan setiap jenis perubahan punya satu tempat:

| Lapisan | Lokasi | Isi | Diubah saat |
|---|---|---|---|
| 1. UI kit | `assets/css/vendor/`, `assets/static/vendor/` | file asli UI kit | upgrade versi saja |
| 2. Token tema | `assets/css/app.css` | warna, radius, font | identitas visual berubah |
| 3. CSS komponen | `assets/css/components/<nama>.css` | override class UI kit, class `app-*` | bentuk satu komponen berubah |
| 4. Komponen Tera | `assets/views/components/` | markup elemen yang berulang | struktur HTML satu komponen berubah |
| 5. Halaman | `assets/views/<resource>/` | menyusun komponen dan utility layout | isi halaman berubah |

- **File UI kit tidak pernah diedit.** Perubahan di sana hilang atau bentrok saat upgrade. Semua
  penyesuaian ada di lapisan 2 dan 3, yang tetap berlaku setelah upgrade.
- **Urutan saat mengubah tampilan: token dulu, lalu CSS komponen, baru komponen baru.** Contoh:
  semua tombol ingin lebih bulat berarti mengubah token radius, bukan setiap template. Override
  sebuah class UI kit ditulis di file yang dinamai sesuai class-nya
  (`assets/css/components/btn.css`), memakai nama class UI kit yang sama.
- **Satu elemen berulang, satu komponen Tera.** Elemen yang dipakai berulang (tombol, field form
  dengan label dan pesan error, card, tabel, badge status, empty state, pagination) dirender lewat
  komponen di `assets/views/components/`, bukan disalin markup-nya ke setiap halaman. Mengubah satu
  komponen mengubahnya di seluruh aplikasi.
- **Kapan membuat komponen**: saat pola yang sama muncul untuk ketiga kalinya, atau sejak pertama
  bila elemennya punya aturan aksesibilitas yang mudah terlupa (label, `aria-*`). Sebelum itu,
  HTML biasa dengan class UI kit sudah cukup; komponen untuk sesuatu yang dipakai sekali hanya
  menambah lapisan.
- **Varian lewat parameter dengan pilihan tetap** (`variant="primary|outline|ghost"`,
  `size="sm|md"`), dipetakan ke nama class utuh di dalam komponen. Parameter `class` dari luar
  hanya untuk layout (margin, lebar, posisi grid), tidak untuk warna, ukuran teks, atau border:
  kalau tampilan bisa diatur dari luar, setiap halaman perlahan punya versi komponennya sendiri.
  Kebutuhan tampilan baru berarti varian baru di komponennya.
- **Pengulangan diselesaikan di template, bukan dengan `@apply`.** Kombinasi utility yang berulang
  dijadikan komponen Tera. `@apply` hanya dipakai di lapisan 3 untuk menyesuaikan class UI kit.
- **Class komponen sendiri diberi prefiks `app-`** (`app-stat-card`, `app-chat-bubble`); class
  UI kit tetap memakai namanya sendiri. Prefiks mencegah tabrakan saat versi UI kit baru
  menambah class dengan nama sama, dan menunjukkan sekilas mana yang milik project.
- Setiap komponen dibuka dengan komentar yang menyebut parameternya, varian yang tersedia, dan
  nilai default-nya. Template bawaan UI kit untuk engine lain (mis. Jinja/Nunjucks di Basecoat)
  tidak disalin; sintaksnya berbeda dengan Tera.
- **Katalog komponen `/_dev/ui`**: satu halaman yang merender setiap komponen di
  `assets/views/components/` dengan semua variannya, memakai data contoh statis. Template-nya
  `assets/views/dev/ui.html`. Komponen atau varian baru ditambahkan ke katalog di commit yang sama.
  - Sebelum membuat komponen baru, cek katalog dulu: komponen yang mirip diperluas dengan varian,
    bukan dibuat ulang dengan nama lain.
  - Route ini hanya **dikompilasi** bila `cfg(debug_assertions)` dan hanya **didaftarkan** bila
    `LOCO_ENV` adalah `development` atau `test`, jadi tidak ada di image release. Ia tidak
    menyentuh database dan tidak memasang sesi, dan ditandai publik secara eksplisit (di luar gate
    login), sehingga agent bisa membukanya tanpa login.
  - Satu tes request merender katalog. Karena Tera 2 gagal pada variabel yang tidak didefinisikan,
    tes ini menangkap komponen yang rusak sebelum dipakai di halaman.

**UI: Tailwind, UI kit, ikon**
- `assets/css/app.css` adalah satu-satunya entry point CSS: `@import "tailwindcss"`, CSS UI kit
  dari `assets/css/vendor/`, token tema, lalu import setiap file di `assets/css/components/`.
  Output-nya `assets/static/css/app.css`, tidak di-commit.
- `@source` di `app.css` menunjuk ke `assets/views/` dan `src/` (class yang ditulis di kode Rust,
  mis. flash message).
- **Nama class Tailwind selalu ditulis utuh**, tidak pernah dirangkai (`"bg-" ~ color`,
  `format!("text-{}", ...)`). Tailwind memindai teks secara statis; class rangkaian tidak ikut
  ter-build dan tampil tanpa style hanya di halaman yang memakainya. Untuk varian, petakan nilai ke
  class utuh (`match status { Active => "badge", Archived => "badge-outline" }`).
- **Pakai komponen UI kit lebih dulu** (`btn`, `card`, `input`, `badge`, `table`, dll.) sebelum
  menyusun komponen sendiri dari utility. Utility Tailwind untuk layout dan spasi.
- **Warna lewat token tema, bukan nilai mentah**: `bg-primary`, `text-muted-foreground`,
  `border-border`, bukan `bg-blue-600` atau hex di template. Token didefinisikan sekali di `app.css`
  (format tema shadcn/ui, `:root` dan `.dark`), sehingga ganti tema dan dark mode hanya satu tempat.
- **JS UI kit (Basecoat)**: runtime dan komponen interaktif yang dipakai dimuat dari layout (atau
  `all.min.js` bila banyak dipakai). Basecoat menginisialisasi DOM baru dari swap HTMX dengan
  sendirinya, termasuk saat tombol Back, karena HTMX 4 mengambil ulang halaman dari server
  (lihat navigasi di bawah). Tidak ada listener inisialisasi ulang di layout. UI kit lain harus
  dicek perilaku yang sama sebelum dipakai; bila tidak menginisialisasi DOM hasil swap sendiri,
  inisialisasinya dipasang sekali lewat `htmx.onLoad`.
  - Pengecualian: bila suatu saat extension `hx-history-cache` dipasang (history dipulihkan dari
    `sessionStorage`), HTML yang dipulihkan masih membawa penanda "sudah diinisialisasi" padahal
    objek JS-nya sudah tidak ada, sehingga dropdown mati setelah Back. Saat itu, pasang
    `window.basecoat.initAll({ force: true })` (atau padanannya di UI kit lain) setelah history
    dipulihkan (nama event dari dokumentasi HTMX 4, bukan nama HTMX 2 `htmx:historyRestore`).
- **Perilaku JS sendiri** ditulis satu file per perilaku di `assets/static/js/` (mis.
  `copy-button.js`), dimuat dari layout, dan dipasang lewat atribut `data-*`
  (`data-copy-target="#token"`), bukan lewat id atau class styling, supaya mengganti tampilan tidak
  mematikan perilakunya. Inisialisasi lewat `htmx.onLoad`, dan aman bila terpanggil dua kali untuk
  elemen yang sama (elemen yang sudah diinisialisasi ditandai), karena swap HTMX bisa memproses
  ulang elemen yang sama.
- **Checklist upgrade UI kit atau HTMX** (ditulis di commit upgrade), dijalankan di `/_dev/ui`:
  buka dropdown, pindah halaman, tekan Back, buka dropdown lagi; lakukan juga untuk select, tabs,
  dan dialog. Upgrade UI kit juga memeriksa setiap file di `assets/css/components/` yang
  meng-override class UI kit, karena struktur class aslinya bisa berubah.
- **Ikon sebagai inline SVG**, bukan webfont (webfont memuat ribuan ikon demi beberapa yang
  dipakai dan ikon sempat kosong sebelum font termuat). Hanya SVG yang dipakai disalin ke
  `assets/icons/<nama>.svg` (datar, tanpa subfolder), dengan nama sama seperti di set ikon asalnya
  (default Tabler, varian outline). Hanya satu set ikon; tidak ada set ikon kedua.
- Versi set ikon asal SVG ditulis di `assets/icons/VERSION` (satu baris, mis. `tabler 3.x.y`). Ikon
  baru disalin dari versi yang sama; upgrade mengganti semua SVG yang dipakai beserta `VERSION`
  dalam satu commit, supaya ketebalan dan bentuk ikon tidak bercampur antarversi.
- Ikon **tidak** ditaruh di `assets/static/`: browser tidak pernah mengunduhnya sebagai file.
  Fungsi `icon()` membaca seluruh `assets/icons/` sekali saat boot ke memori, lalu menyisipkan SVG
  ke HTML. Karena dibaca saat boot, `assets/icons/` ikut masuk image runtime bersama `assets/views/`.
- Ikon dirender lewat satu fungsi Tera yang didaftarkan di initializer view engine:
  `{{ icon(name="trash", class="size-4") }}`. Fungsi memakai `currentColor` (warna ikut teks dan tema)
  dan **error untuk nama yang tidak dikenal**, sehingga ikon yang salah ketik gagal di tes, bukan
  tampil kosong di produksi.
- Fungsi Tera custom (termasuk `icon`) didaftarkan di closure
  `TeraView::build_with_post_process` dengan signature Tera 2. Closure itu berjalan **sebelum**
  template dimuat, jadi setiap fungsi yang dipanggil template wajib didaftarkan di sana; yang
  tidak terdaftar membuat pemuatan template gagal saat boot.
- Ikon yang hanya dekoratif diberi `aria-hidden="true"`; tombol yang hanya berisi ikon wajib punya
  `aria-label`.
- Ikon yang diulang ratusan kali di satu halaman (tabel besar) memakai SVG sprite dengan `<use>`.
  Tambahkan sprite hanya saat kasus itu ada.

**Navigasi ala SPA (HTMX 4: `hx-boost` + `hx-preload`)**
- **HTMX 4 tidak mewariskan atribut secara default.** Atribut yang harus berlaku untuk elemen
  turunan ditulis dengan sufiks `:inherited` (`hx-boost:inherited`, `hx-target:inherited`,
  `hx-confirm:inherited`). Tanpa sufiks, atribut hanya berlaku di elemen itu sendiri dan tidak ada
  error yang memberi tahu. Ini sumber bug termudah saat menyalin contoh HTMX 2.
- **Contoh HTMX 2 tidak dipakai**, termasuk yang disarankan agent: nama event lama
  (`htmx:afterSwap`, `htmx:load`), atribut `hx-ext`, `hx-vars`, dan atribut `preload`. Perhatian
  khusus: `hx-disable` di HTMX 4 berarti "nonaktifkan elemen selama request"; peran lamanya
  ("jangan proses elemen ini") kini `hx-ignore`. Rujukan nama selalu dokumentasi HTMX 4.
- `hx-boost:inherited="true"` dipasang sekali di `<body>` layout, bukan per link. Link dan form
  biasa otomatis menjadi navigasi tanpa reload, dan tetap berfungsi normal bila JS gagal dimuat.
- Extension dimuat lewat tag `<script>` di layout setelah `htmx.js`. HTMX 4 tidak memakai atribut
  `hx-ext`.
- Setiap URL tetap mengembalikan halaman HTML lengkap saat dibuka langsung (refresh, bookmark,
  link dari luar). Navigasi boosted hanya mempercepat; ia tidak boleh menjadi satu-satunya jalan ke
  sebuah halaman.
- **Halaman lengkap atau partial ditentukan di satu helper** di `controllers/`. Request boosted juga
  membawa header `HX-Request`, padahal ia butuh halaman lengkap; pembedanya adalah header
  `HX-Request-Type` (`partial` → partial, selain itu termasuk `full` dan tanpa header → halaman
  lengkap). Helper ini dites untuk ketiga kasus.
- Redirect setelah POST memakai redirect HTTP standar (`303`); HTMX mengikutinya dan memperbarui URL.
  Jangan membuat jalur redirect khusus HTMX kecuali memang perlu (`HX-Redirect`).
- **Form atau link yang berakhir di domain lain diberi `hx-boost="false"`**: redirect ke domain lain
  harus menjadi navigasi browser penuh, bukan request `fetch()` HTMX (`htmx.config.mode` default
  `same-origin` menolaknya).
- Block `title` wajib diisi di setiap halaman, karena HTMX memperbarui judul tab dari `<title>`
  respons boosted.
- **Respons error ikut di-swap.** HTMX 4 men-swap semua respons kecuali `204` dan `304`. Respons
  `422` (validasi gagal) langsung tampil tanpa konfigurasi tambahan. Konsekuensinya: setiap
  respons error adalah HTML yang layak di-swap (halaman error lengkap atau partial form), tidak
  pernah teks mentah. Jangan memasang `htmx.config.noSwap` untuk mengembalikan perilaku HTMX 2.
- **Back mengambil ulang halaman dari server**, bukan dari snapshot. Ini membuat komponen JS
  (UI kit) selalu benar setelah Back, dengan syarat handler GET cepat dan tidak mengubah data
  (sudah menjadi aturan). `hx-history-cache` tidak dipasang tanpa kebutuhan terukur (lihat JS
  UI kit di atas).
- JS yang perlu jalan setelah navigasi atau swap dipasang lewat `htmx.onLoad(fn)`, bukan
  `DOMContentLoaded` (yang hanya terpicu sekali pada full page load).
- CSS dan JS global dimuat di layout, tidak di dalam konten halaman, supaya tidak dimuat ulang
  atau terduplikasi setiap navigasi.
- HTMX 4 memakai timeout default 60 detik per request. Aksi yang wajar lebih lama dari itu
  (upload besar, laporan berat) tidak dijalankan sebagai request HTMX: jadikan job (§8) atau
  navigasi biasa.
- **Prefetch**: extension `hx-preload` dimuat di layout. Saat dimuat, setiap link boosted otomatis
  di-preload saat `mousedown`. Di kontainer navigasi (sidebar, menu, daftar item) dipasang
  `hx-preload:inherited="mouseover"`, sehingga halaman tujuan sudah diambil sebelum diklik dan
  perpindahan terasa instan.
- **Link ke unduhan, export, atau laporan berat diberi `hx-boost="false"`**: tidak di-preload,
  tidak di-swap ke halaman, dan browser menanganinya sebagai navigasi atau unduhan biasa.
- Handler GET tidak mencatat aktivitas atau menghitung view; preload membuat angka itu menggelembung.
  Request preload membawa header `HX-Preloaded: true` bila perlu dibedakan di log.

**Teks untuk user (i18n)**
- **Semua teks yang dilihat user ada di file Fluent** (`assets/i18n/<locale>/main.ftl`, mis. `id`)
  sejak hari pertama, walaupun aplikasi baru satu bahasa: label, tombol, pesan validasi, flash,
  pesan error, dan teks email. Template memakai `t()` (dari `fluent-templates` 0.15 ke atas, versi
  yang mendukung Tera 2). Hasilnya satu tempat untuk semua teks, satu gaya bahasa, dan bahasa kedua
  nanti cukup menambah file.
- Kode Rust tidak berisi kalimat untuk user. Validator, error domain, dan flash mengirim kode
  plus argumen; view yang menerjemahkan. Komponen Tera field form menerjemahkan kode error validasi.
- Satu tes memastikan setiap kode yang dipakai di kode Rust punya terjemahan di file Fluent.

## 7. Autentikasi (email + password, sesi database)

Login memakai email dan password lokal. Sesi memakai **sesi server-side bawaan Loco** (feature
`auth_session`, `auth.session.store: database`, tersedia sejak Loco 1.3): cookie hanya membawa id
acak yang ditandatangani, dan data sesi tinggal di database, sehingga sesi bisa dimatikan dari
server. JWT tidak dipakai: JWT tidak bisa dicabut sebelum kedaluwarsa, padahal logout, ganti
password, reset password, dan menonaktifkan user harus mengakhiri sesi **seketika**. Controller
auth JWT starter dihapus; config tidak berisi `auth.jwt` maupun `JWT_SECRET`.

**Struktur**
- Sesi (cookie, penyimpanan, rotasi id, revoke) memakai `loco_rs::auth::session` dan
  `loco_rs::auth::session_store`, tidak ditulis ulang. Yang ditulis tangan di `src/services/auth/`:
  password, login, token email, rate limit, dan aturan sesi tambahan di bawah (umur mutlak, user
  dinonaktifkan). Controller auth hanya adapter (§6).
- `loco_rs::hash::hash_password` tidak dipakai: parameternya tetap `Params::default()`, sehingga
  test tidak bisa memakai parameter minimal dan rehash saat parameter naik tidak bisa dikendalikan.
- **Kriptografi tidak ditulis sendiri.** Hashing password memakai crate `argon2` (format PHC
  string), token memakai CSPRNG (`rand::rngs::OsRng` atau setara), digest memakai `sha2`,
  perbandingan rahasia memakai fungsi constant-time (`subtle`).
- Semua setting auth dibaca lewat satu modul config (`settings.auth`) yang memvalidasi **semua key
  sekaligus saat boot**, bukan gagal satu per satu saat pertama dipakai.

**Data**
- `users`: `email` (unik lewat expression index `lower(email)`, dinormalisasi di model),
  `password_hash` (PHC string), `name`, `email_verified_at`, `password_changed_at`, `disabled_at`.
  User yang dinonaktifkan ditandai `disabled_at`, bukan dihapus.
- `loco_sessions` dan `loco_remember_tokens` dibuat dan dikelola Loco sendiri (`CREATE TABLE IF
  NOT EXISTS` saat sesi pertama dipakai), bukan lewat migrasi. **Ini satu-satunya
  pengecualian dari aturan §4** (UUID, `timestamptz`, FK): `user_key` berupa `TEXT` berisi UUID
  user, `expires_at` berupa detik epoch, dan tidak ada FK ke `users`. Tabel ini tidak diubah,
  tidak dibuatkan migrasi, dan tidak di-query langsung; aksesnya hanya lewat API session store.
  Karena tanpa FK, menghapus user wajib memanggil `sign_out_everywhere` lebih dulu (lihat Sesi).
  Hook `truncate` di test ikut mendaftarkan kedua tabel itu.
- `user_tokens` untuk token sekali pakai yang dikirim lewat email: `user_id`, `purpose` (enum:
  `password_reset`, `email_verification`, dengan CHECK constraint §4), `token_digest` (unique),
  `expires_at`, `used_at`. Satu tabel, satu implementasi untuk semua token email; jangan kolom
  token terpisah di `users`.
- `login_throttles` untuk rate limit (§1: tanpa Redis): `key` (mis. `email:<lower>` atau
  `ip:<addr>`), `failures`, `window_started_at`, `locked_until`.

**Password**
- **argon2id** dengan parameter dari config (baseline OWASP: m = 19 MiB, t = 2, p = 1). Karena PHC
  string menyimpan parameternya, parameter bisa dinaikkan kapan saja: saat login berhasil dan hash
  memakai parameter lama, password di-hash ulang dengan parameter baru.
- Hashing dan verifikasi dijalankan di `tokio::task::spawn_blocking`. argon2 sengaja berat di CPU
  dan memori; dijalankan langsung di handler async, ia memblokir thread runtime dan memperlambat
  request lain.
- **Jumlah hash yang berjalan bersamaan dibatasi** satu `tokio::sync::Semaphore` di
  `services/auth` (`settings.auth.max_concurrent_hashes`, default sejumlah core CPU). Pool
  `spawn_blocking` Tokio bisa sampai 512 thread; 512 hash × 19 MiB ≈ 9,7 GB memori. Rate limit per
  email dan per IP tidak menahan serangan dari banyak IP dengan email acak, jadi batas ini yang
  menjaga server tetap hidup. Request yang menunggu izin lebih dari beberapa detik dijawab `503`
  (halaman error biasa), bukan antre tanpa batas.
- Aturan password: panjang minimal 12 karakter, maksimal 128 (konstanta di satu tempat), tanpa
  aturan komposisi (huruf besar/angka/simbol) dan tanpa kedaluwarsa berkala, mengikuti NIST
  SP 800-63B. Batas maksimal mencegah input raksasa dipakai untuk membebani hashing.
- **Password tidak pernah di-log, tidak masuk job args, dan tidak dirender ulang.** Field password
  di struct form bertipe `secrecy::SecretString` (atau tipe dengan `Debug` yang disamarkan), supaya
  `tracing` atau `dbg!` pada struct form tidak membocorkannya. Form yang gagal validasi dirender
  ulang dengan field password kosong.

**Login**
- Form `POST /login` (email, password, `return_to` opsional). Gagal karena alasan apa pun (email
  tidak dikenal, password salah, user dinonaktifkan) menghasilkan **satu pesan umum yang sama**
  dengan status `422`, email tetap terisi. Pesan yang berbeda memberi tahu penyerang email mana yang
  terdaftar.
- **Waktu respons juga tidak membedakan.** Bila email tidak dikenal, password tetap diverifikasi
  terhadap hash dummy (dibuat sekali saat boot dengan parameter yang sama), sehingga durasinya
  setara dengan email yang terdaftar.
- **Rate limit per email dan per IP** (angka di config, mis. 5 gagal per email dalam 15 menit,
  lalu terkunci 15 menit). Kunci berlaku untuk string email apa pun, terdaftar atau tidak, supaya
  pesan "terlalu banyak percobaan" tidak membocorkan keberadaan akun. Login berhasil mereset
  penghitung email itu. Baris kedaluwarsa dibersihkan job harian.
- IP client diambil dari extractor `RemoteIP` milik middleware `remote_ip` Loco, bukan dibaca
  sendiri dari header. Middleware aktif di setiap environment, dengan `source` sesuai cara aplikasi
  dijangkau:
  - tanpa proxy (development, test): `source: ConnectInfo` (peer address);
  - di belakang satu reverse proxy: `RightmostXForwardedFor` (default), dan **hanya bila aplikasi
    tidak bisa dijangkau selain lewat proxy itu**; kalau port aplikasi terbuka langsung, siapa pun
    bisa mengirim `X-Forwarded-For` palsu dan melewati rate limit per IP;
  - di belakang provider tertentu: header khususnya (mis. `CfConnectingIp` untuk Cloudflare).
  Beberapa hop proxy diselesaikan di proxy terdalam (mis. `set_real_ip_from` nginx), bukan di
  aplikasi. `RemoteIP::None` dianggap salah konfigurasi: login ditolak dan dicatat sebagai error,
  bukan rate limit per IP yang diam-diam dilewati.
- **Login selalu membuat sesi baru**: `session.renew().set_user_id(user.id.to_string())`, lalu
  `signed_in_at` disimpan di data sesi (untuk umur mutlak, lihat Sesi). `renew()` membuang id dan
  data lama, sehingga id yang ditanam sebelum login (session fixation) tidak ikut terbawa.
- `return_to` hanya boleh path internal: diawali `/`, bukan `//`, tidak mengandung `\`. Nilai lain
  dibuang, bukan dikoreksi (open redirect).
- User yang sudah login dan membuka `/login` di-redirect ke halaman awal aplikasi.

**Pendaftaran dan verifikasi email**
- Pendaftaran publik adalah keputusan per project (setting `auth.registration_open`). Bila
  tertutup, user dibuat lewat task `user:create` atau oleh admin di UI, lewat satu fungsi model
  yang sama.
- Bila terbuka: respons `POST /register` **sama** untuk email baru dan email yang sudah terdaftar
  ("cek email Anda"). Email baru mendapat link verifikasi; email yang sudah terdaftar mendapat email
  pemberitahuan berisi link reset password. Tanpa ini, form register menjadi alat mengecek email
  mana yang punya akun.
- Link verifikasi memakai `user_tokens` (`purpose = email_verification`). User yang belum
  terverifikasi tidak mendapat sesi; setelah password benar, ia diarahkan ke halaman yang meminta
  verifikasi dengan tombol (POST) kirim ulang, yang ikut rate limit.

**Reset password**
- `POST /password/forgot` selalu menjawab sama, terdaftar atau tidak, dan email dikirim lewat job
  (§8) sehingga durasi respons tidak membedakan. Permintaan ikut rate limit per email dan per IP.
- Token: 32 byte acak, database hanya menyimpan digest SHA-256, umur 30 menit (config), sekali
  pakai. Permintaan baru membatalkan token reset lama milik user itu.
- `GET /password/reset?token=...` hanya menampilkan form dan **tidak mengonsumsi token**: pemindai
  link di layanan email dan preload (§6) membuka link tanpa diklik user. Token dikonsumsi oleh
  `POST`, lewat claim bersyarat (`used_at IS NULL AND expires_at > now()`, cek
  `rows_affected == 1`, §5).
- Halaman yang membawa token di URL mengirim `Referrer-Policy: no-referrer`, dan query string
  route itu tidak ditulis ke log request.
- Reset berhasil: simpan hash baru, set `password_changed_at`, set `email_verified_at` bila masih
  kosong (user terbukti memegang inbox-nya), **revoke semua sesi user**, lalu redirect ke login.

**Ganti password (user sudah login)**
- Wajib memasukkan password saat ini. Berhasil: `sign_out_everywhere` untuk user itu (semua sesi
  dan remember-me), lalu sesi baru untuk perangkat saat ini lewat `session.renew()` di respons yang
  sama, sehingga hanya perangkat yang mengganti password yang tetap login.

**Sesi (bawaan Loco, `store: database`)**
- Atribut cookie dipasang Loco (`HttpOnly`, `SameSite=Lax`, `Path=/`, tanpa `Domain`). Nama cookie
  staging dan production berawalan `__Host-` (`auth.session.cookie_name: __Host-<app>_session`):
  browser menolak cookie itu dari subdomain lain atau tanpa HTTPS. Di development, nama cookie
  diberi suffix worktree bila memakai modul Worktree (§11).
- **`auth.session.secure: true` ditulis eksplisit di staging dan production.** Loco memasang flag
  `Secure` dari config (`server.host` diawali `https://`), bukan dari request. Di belakang proxy
  yang memegang TLS, `server.host` mudah tertulis `http://`; cookie `__Host-` tanpa `Secure` lalu
  ditolak browser dan **tidak ada yang bisa login**. Tes kontrak config (§9) memeriksa nilai ini.
- **Idle timeout** adalah `auth.session.ttl` (detik, bergulir setiap sesi dipakai, default 1 hari).
  Cookie sesi server-side tidak punya `Max-Age`, jadi sesi juga berakhir saat browser ditutup.
- **Umur mutlak tidak disediakan Loco**, jadi ditambahkan aplikasi: `signed_in_at` disimpan di
  data sesi saat login, dan extractor current user menolak serta `clear()` sesi yang lebih tua dari
  `settings.auth.absolute_lifetime_hours`. Tanpa ini, sesi yang terus dipakai tidak pernah
  berakhir.
- **Remember-me** ("ingat saya", `session.remember()` dengan `auth.session.remember_for`) adalah
  keputusan per project dan default-nya mati. Bila dipakai, umurnya menjadi batas mutlak login
  yang diingat.
- Sesi di-resolve di **satu tempat**: satu extractor aplikasi yang membungkus `Session` Loco,
  memuat user, lalu menolak sesi bila user tidak ada, `disabled_at` terisi, atau umur mutlak
  terlewati. Handler tidak memakai `CurrentUser<String>` Loco langsung.
- Job harian memanggil `SessionStore::purge_expired`; Loco hanya membersihkan sesi kedaluwarsa saat
  ada penulisan sesi, jadi tanpa job ini tabel tumbuh saat lalu lintas sepi.
- Revoke memakai `loco_rs::auth::session_store::sign_out_everywhere(&ctx, &user_id)`, yang
  menghapus semua sesi dan remember-me user itu. Dipanggil saat: reset password, ganti password
  (lalu sesi saat ini dibuat ulang), user dinonaktifkan (**setelah** `disabled_at` ter-commit; bila
  revoke gagal, extractor tetap menolak user itu), dan user dihapus (sebelum baris user dihapus,
  karena `loco_sessions` tidak punya FK).

**Gate**
- Gate tertutup secara default: setiap route butuh user yang login dan tidak dinonaktifkan, kecuali
  yang secara eksplisit ditandai publik (login, register, lupa/reset password, verifikasi email,
  `/_ping`, `/_health`). Daftar route publik ada di satu tempat.
- **Logout selalu lolos semua gate**, apa pun state sesinya.

**Logout**
- `POST /logout` (form dengan tombol, bukan link GET): `session.clear()`, lalu redirect `303` ke
  halaman login. Cookie yang
  tidak lagi cocok dengan sesi mana pun tetap dihapus dan redirect tetap terjadi.

**Login development tanpa password**
Agent AI yang mengecek halaman di browser tidak semestinya memegang password, dan seed tidak berisi
password (§4). Tanpa jalan resmi, orang akan membuat jalan pintas sendiri dengan cara yang lebih
buruk. Jalan resminya satu, dan dijaga ketat:
- `bin/loco task dev_login email:<email>` memeriksa bahwa user **sudah ada dan tidak
  dinonaktifkan**, lalu mencetak link `http://localhost:<PORT>/_dev/login?token=...`. Token berisi
  user id dan waktu kedaluwarsa (10 menit), ditandatangani HMAC-SHA256 dengan
  `auth.session.secret`; task tidak menulis apa pun ke database. Task menolak berjalan di
  environment selain `development`.
- Route `/_dev/login` memverifikasi token, membuat sesi lewat jalur yang sama dengan login biasa
  (`renew()`, `set_user_id`, `signed_in_at`), lalu redirect ke `/`. Route ini hanya
  **dikompilasi** bila `cfg(debug_assertions)` dan hanya **didaftarkan** bila `LOCO_ENV=development`.
  Image Docker selalu di-build release, jadi kodenya tidak ada di staging dan production.
- Ini **satu-satunya GET yang menulis data** (membuat sesi), pengecualian dari aturan §6 yang hanya
  ada di build debug. Ia tidak berada di kontainer `hx-preload`, dan link-nya dibuka langsung,
  bukan lewat navigasi boosted.
- Link setara password selama 10 menit: jangan dibagikan atau di-commit. Sesinya berakhir seperti
  sesi lain.
- Tes: di environment `test`, `GET /_dev/login` menghasilkan `404`, dan task menolak berjalan.
- Tidak ada jalan pintas lain: tidak ada flag, password master, atau route lain yang melewati login.

**Konfigurasi**
- `auth.session` (config Loco): `store: database`, `ttl`, `cookie_name`, dan
  `secret: {{ get_env(name="SESSION_SECRET") }}` (64+ karakter acak, mis.
  `openssl rand -base64 64`). Mengganti `SESSION_SECRET` membuat semua user ter-logout.
- `settings.auth` (milik aplikasi) per environment: parameter argon2, `max_concurrent_hashes`,
  `absolute_lifetime_hours`, batas rate limit, umur token email, `registration_open`.
- `config/development.yaml` memakai parameter argon2 yang **sama dengan production**, sehingga
  login dan rehash di dev berperilaku seperti di production. Agar tetap cepat di build debug,
  `argon2` dan `blake2` di-compile dengan `opt-level = 3` (§14): satu hash ~26 ms, bukan ~440 ms.
- `config/test.yaml` memakai parameter argon2 minimal (m = 8 KiB, t = 1), karena suite tes membuat
  banyak user dengan password. Tes kontrak config (§9) memastikan staging dan production tidak
  memakai angka itu.

**Testing auth**
- Helper `sign_in_as(user)` login lewat `POST /login` dengan password tes, lalu memakai cookie
  yang dikembalikan. Parameter argon2 minimal di test membuatnya murah, dan helper tidak
  bergantung pada API internal session store. Tes tidak memakai `/_dev/login`.
- Tes penolakan minimal: email tidak dikenal dan password salah menghasilkan pesan dan status yang
  sama; user dinonaktifkan tidak bisa login dan sesinya yang ada ditolak; rate limit mengunci
  setelah batas, juga untuk email yang tidak terdaftar; cookie sesi sebelum login tidak berlaku
  setelah login (fixation); `return_to` eksternal dibuang; token reset kedaluwarsa, dipakai dua
  kali, dan token lama setelah permintaan baru ditolak; `GET` link reset tidak mengonsumsi token;
  reset dan ganti password me-revoke sesi lain; ganti password tanpa password lama ditolak; sesi
  melewati idle timeout dan umur mutlak ditolak; `GET /logout` tidak me-logout; logout tetap
  berhasil dengan cookie yang sudah tidak valid; register dengan email terdaftar menjawab sama
  dengan email baru.

## 8. Workers, scheduler, tasks, email, storage

**Workers**
- Worker tipis: deserialize args → panggil model/service. Args adalah struct bertipe berisi `id`
  (UUID), bukan objek penuh (data bisa basi saat job jalan).
- **Jangan taruh secret di job args** (password, token mentah, API key). Args disimpan di tabel
  queue dan bisa muncul di log. Pengecualian yang tidak terhindarkan: token mentah reset/verifikasi
  harus sampai ke email; bila dikirim lewat job, args-nya berumur sependek tokennya, job dihapus
  setelah selesai, dan args tidak di-log.
- Job harus idempoten: aman bila dijalankan dua kali (retry, redelivery, `cargo loco jobs retry`).
- Satu item gagal tidak boleh menggagalkan seluruh batch; log dan lanjutkan.
- Enqueue banyak job sekaligus memakai `perform_all_later` (satu round trip, atomik), bukan loop
  `perform_later` yang bisa meninggalkan queue terisi setengah bila salah satunya gagal.
- Production: proses web dan worker terpisah (`start` dan `start --worker`).
  Dev boleh `--server-and-worker`.
- Mode `ForegroundBlocking` di `config/test.yaml` agar job jalan sinkron saat tes.

**Scheduler**
- Job berkala didefinisikan di config scheduler, setiap entri diberi komentar apa dan kenapa
  jadwalnya segitu.
- **Scheduler berjalan di tepat satu proses.** Dua scheduler berarti setiap job berkala jalan dua
  kali. Di production scheduler ikut di container worker (flag `--scheduler`; cek kombinasi flag
  di `start --help` versi Loco yang terpakai); bila worker diskalakan lebih dari satu, scheduler
  dipindah ke container sendiri yang berjumlah satu.

**Tasks**
- Operasional one-off dan import adalah task (`bin/loco task <name> key:value`), bukan migrasi.
- Task destruktif **dry-run by default**; hanya `confirm:yes` yang memutasi. Dry run diakhiri
  dengan perintah persis untuk menjalankan ulang.
- Badan task tipis; logika ada di model/service yang dites, supaya CLI dan UI tidak berbeda perilaku.
- Output: banner fase, baris status `[OK ]` / `[FAIL]` / `[SKIP]`, dan baris hitungan akhir.
- Task yang menerima password (`user:set_password`) membacanya dari prompt tanpa echo, tidak dari
  argumen CLI: argumen tercatat di history shell dan terlihat di daftar proses.

**Email**
- Email dikirim lewat mailer Loco (`src/mailers/`) dan **selalu lewat job** (`deliver_later`), tidak
  pernah di dalam request: SMTP yang lambat atau mati tidak membuat request ikut lambat atau gagal.
- Subjek dan isi email memakai teks dari file Fluent (§6), dengan versi HTML dan teks biasa.
- Setiap template email dirender oleh satu tes. Tera 2 gagal pada variabel yang tidak didefinisikan,
  dan untuk email kegagalannya baru terjadi saat job mengirim, tanpa ada yang melihat.
- `mailer.stub: true` di `config/test.yaml`; tes memeriksa email yang "terkirim" lewat stub.
- Development mengirim ke mail sink lokal (mis. Mailpit), bukan SMTP sungguhan. Staging yang memakai
  salinan data production juga diarahkan ke sink, supaya user sungguhan tidak menerima email dari
  staging.
- Kredensial SMTP hanya dari env var. Link di email dibentuk dari base URL di config
  (`settings.public_base_url`), tidak dari header `Host` request (host header injection).

> **Opsional, modul Storage (upload file).** Hapus subbagian ini, feature `storage_aws_s3`, folder
> `storage/`, dan rujukan "bila memakai modul Storage" bila aplikasi tidak menyimpan file user.

**Storage file**
- **Driver per environment**, dipilih dari config, kode aplikasi tidak berubah:

  | Environment | Driver | Lokasi |
  |---|---|---|
  | development | disk | `storage/` di root project (gitignored), per worktree bila memakai modul Worktree (§11) |
  | test | in-memory | hilang setelah tes; cepat dan tidak meninggalkan file |
  | staging | S3 | bucket staging sendiri |
  | production | S3 | bucket production sendiri |

- **Semua akses file lewat `ctx.storage`** (subsistem storage Loco). Tidak ada `std::fs` untuk file
  user dan tidak ada S3 SDK yang dipanggil langsung. Hanya dengan begitu disk dan S3 bisa ditukar
  lewat config: kode yang menulis ke disk langsung lolos di development dan rusak di staging.
- Driver dibangun di hook `after_context` dari typed settings `settings.storage`. Driver disk
  dibuat dengan root eksplisit ke folder `storage/` (`local::new_with_prefix`), tidak mengandalkan
  working directory proses. Feature `storage_aws_s3` diaktifkan di `Cargo.toml`.
- **Staging dan production tidak pernah berbagi bucket.** Kredensial S3 hanya dari env var, dengan
  izin terbatas pada bucket environment-nya.
- **Database menyimpan key, bukan URL atau path absolut**: key relatif yang sama berlaku di disk
  maupun S3. Key dibentuk dari id, mis. `documents/<uuid>/original`, bukan dari nama file user
  (path traversal, karakter aneh, tabrakan nama). Nama file asli, content type, dan ukuran disimpan
  sebagai kolom.
- **Bucket privat.** File diunduh lewat controller yang memeriksa otorisasi lalu men-stream isinya;
  tidak ada URL publik ke bucket. Link unduhan diberi `hx-boost="false"` (§6).
- Upload dibatasi ukurannya (middleware `limit_payload`), dan tipe file divalidasi dari isinya,
  bukan dari ekstensi.
- **Urutan tulis**: upload file dulu, baru simpan baris di database; bila insert gagal, hapus file
  yang baru diupload. Saat baris dihapus, file dihapus **setelah** commit (§5), sebaiknya lewat job.
- **File di storage tidak pernah diubah langsung.** Mengganti isi file berarti upload ke key baru,
  memperbarui baris, lalu menghapus key lama. Aturan ini membuat cache dan URL aman, berlaku sama di
  disk dan S3, dan (bila memakai modul Worktree) membuat salinan hard link `storage/` di worktree
  (§11) aman: hard link berbagi isi file, jadi menulis ulang file di worktree akan ikut mengubah
  file di checkout utama.

## 9. Testing

- Tes dijalankan lewat `bin/test`. Helper `loco_rs::testing`: `boot_test` untuk model/worker,
  `request::<App, Migrator, _, _>` untuk HTTP.
- **Tes berjalan paralel lewat `cargo nextest`** (`bin/test`): setiap tes di prosesnya sendiri, dan
  setiap proses memakai database slot-nya sendiri. `config/test.yaml` membentuk URI
  `<TEST_DATABASE_URL>_s<slot>` dari `NEXTEST_TEST_GLOBAL_SLOT`; nextest menjamin satu slot hanya
  dipakai satu tes pada satu waktu. `bin/test` memigrasi slot 0, lalu membuat ulang slot lain sebagai
  clone-nya (`CREATE DATABASE ... TEMPLATE`) setiap run. Di dalam slot, hook `truncate` tetap
  mengosongkan tabel sebelum setiap tes.
- Jumlah proses paralel: `AG_TEST_THREADS` (default 8). Postgres bersama diset
  `max_connections=300` (`docker/compose.shared.yaml`) untuk menampung pool semua proses.
- Tidak memakai `#[serial]` per tes. Tes yang mengubah env var proses (`std::env::set_var`) aman
  karena setiap tes punya proses sendiri.
- Setiap tes tetap independen (tidak bergantung urutan atau sisa data tes lain), supaya peralihan
  ke tes paralel tidak butuh menulis ulang.
- `dangerously_truncate: true` hanya di `config/test.yaml`, dengan hook `truncate` mendaftarkan tabelnya.
- **Data tes dibangun di dalam tes**, bukan dari fixture data bersama. Fixture YAML hanya untuk
  seed dev. Tes yang bergantung pada data global lulus dengan alasan yang salah.
- **Fungsi pembangun data dipakai bersama**, tidak disalin per file: builder di `tests/support/`
  (mis. `a_user().with_roles(&[admin]).with_password("...").create(&db)`). Datanya dibuat per tes;
  kodenya satu. Fungsi yang sama di dua file tes dipindah ke `tests/support/`.
- File tes dipecah per aksi atau perilaku (`tests/requests/admin/roles/{list,create,update}.rs`),
  bukan satu file per controller yang tumbuh sampai ribuan baris.
- **Halaman HTML dites pada perilaku**, bukan snapshot seluruh markup: status, lokasi redirect,
  dan keberadaan elemen penting (helper selector HTML `assert_css_*` dari feature `testing` Loco,
  bukan `testing-core` bawaan starter). Snapshot HTML penuh
  pecah di setiap perubahan styling.
- **Setiap halaman dirender setidaknya oleh satu tes request.** Karena Tera 2 gagal pada variabel
  yang tidak didefinisikan, tes inilah yang menangkap variabel yang lupa dikirim view, sebelum
  user melihat halaman 500. Katalog `/_dev/ui` dan setiap template email termasuk di dalamnya
  (§6, §8).
- Snapshot (`insta`) hanya untuk struktur data yang stabil (output service, payload job), dengan
  filter cleanup Loco untuk field dinamis (`id`, timestamp). Snapshot di-review sebelum di-accept.
- Nama tes adalah kalimat yang menjelaskan klaim perilaku
  (`reset_token_cannot_be_used_twice`), dengan referensi bug bila ada.
- Tes tidak pernah menyentuh jaringan sungguhan; client eksternal (SMTP lewat stub, dan provider
  embedding bila memakai modul Vector) diinjeksi dan di-stub. Host eksternal di `config/test.yaml`
  memakai nama dummy yang **sengaja tidak resolve** (`*.test`), supaya tes yang tidak sengaja
  menyentuh jaringan langsung gagal. Jangan menambah entri `/etc/hosts` untuknya.
- **Tes cabang error, bukan cuma happy path.** Setiap halaman terproteksi punya tes redirect ke
  login; setiap akses ke data orang lain punya tes 404; setiap form punya tes 422 dengan pesan error.
- Race dites tanpa konkurensi: claim dua kali, assert yang kedua mengembalikan `false`.
- Direktori mengikuti kode: `tests/requests`, `tests/models`, `tests/workers`, `tests/tasks`;
  helper bersama di `tests/support`. Semuanya submodule dari satu file root (`tests/mod.rs`),
  bukan file terpisah di root `tests/`, supaya hanya ada satu binary tes yang di-link (§14).
- Satu tes kontrak config: setiap `config/*.yaml` environment yang di-deploy ter-parse, punya
  bagian wajib (database, queue, mailer, `auth.session` dengan `store: database`, `settings.auth`,
  dan storage bila memakai modul Storage),
  middleware `remote_ip` dan `secure_headers` aktif, dan `settings` bisa di-deserialize ke struct
  bertipe. Tes ini mengecek keberadaan dan bentuk, **bukan nilai**, dengan pengecualian keamanan
  untuk staging dan production: parameter argon2 tidak di bawah baseline, `auth.session.secure`
  bernilai `true`, dan nama cookie berawalan `__Host-` (§7). Config yang hilang di satu
  environment gagal diam-diam di produksi.

## 10. Konfigurasi dan secret

- Satu file per environment di `config/`. Staging adalah file sendiri yang sedekat mungkin dengan production.
- Secret **hanya** dari environment variable lewat `{{ get_env(name="...") }}`, tanpa `default`
  di production (gagal boot lebih baik daripada jalan dengan nilai dummy). Tidak ada secret yang di-commit.
- Di `development.yaml` dan `test.yaml`, database URI, `queue.uri`, port, dan suffix cookie sesi
  dibaca dari env (`DATABASE_URL`, `PORT`, `SESSION_COOKIE_SUFFIX`) yang di-set wrapper `bin/*`
  (§3, §11). **`DATABASE_URL` tanpa default**: tanpa wrapper, aplikasi gagal boot alih-alih memakai
  database yang salah (mis. database checkout utama dari dalam worktree). `queue.uri` memakai
  `DATABASE_URL` yang sama; queue tidak pernah menunjuk ke database bersama.
- File `{env}.local.yaml` (gitignored) di-merge di atas `{env}.yaml` sejak Loco 1.0: hanya key yang
  ditulis di file lokal yang menimpa. Tidak dipakai untuk secret production dan tidak dipakai
  untuk hal yang sudah diatur wrapper `bin/*`.
- Tunable aplikasi di bagian `settings:`, dibaca ke **satu struct bertipe** di
  `src/common/settings.rs` (deserialize sekali saat boot), bukan `serde_json::Value` yang dibaca di banyak tempat.
- Durasi ditulis sebagai angka dengan unit di nama key (`idle_timeout_minutes`). Setiap key diberi
  komentar: apa yang dibatasi, kenapa angkanya segitu, insiden yang menetapkannya.
- Production: `logger.pretty_backtrace: false`, `database.enable_logging: false`,
  `dangerously_*: false`, `auto_migrate: false`.
- Filter data sensitif dari log: password, cookie sesi, `SESSION_SECRET`, token reset dan
  verifikasi email serta token `dev_login` (termasuk
  di query string), password SMTP, API key pihak ketiga, dan OTP tidak pernah di-`tracing` sebagai
  field.
- **`statement_timeout` dipasang di role database aplikasi**
  (`ALTER ROLE <app> SET statement_timeout = '30s'`), bukan diandalkan dari kode. Satu query yang
  lupa index tidak boleh memegang koneksi pool selamanya. Job atau task yang memang butuh lebih lama
  menaikkannya sendiri dengan `SET LOCAL statement_timeout` di transaksinya, dengan komentar alasannya.

**Logging dan observability**
- **Log production berformat JSON** (`logger.format: json`), satu event per baris, supaya bisa
  dicari per field di sistem log. Development memakai `pretty` atau `compact`.
- **Middleware `request_id` Loco aktif di semua environment.** Id request (header `x-request-id`)
  ikut di setiap baris log request itu lewat span `tracing`, dan ditampilkan di halaman 500
  ("kode error: ..."). Laporan user cukup menyebut kode itu untuk menemukan log-nya. Job yang
  di-enqueue dari request membawa id yang sama di args-nya.
- Log memakai field terstruktur (`tracing::info!(user_id = %id, "...")`), bukan nilai yang
  dirangkai ke dalam pesan. Level: `error` untuk yang perlu ditindaklanjuti manusia, `warn` untuk
  yang aneh tapi tertangani, `info` untuk event bisnis penting, `debug` untuk sisanya.
- **Error tracking**: setiap respons 500 dan job yang gagal permanen dikirim ke satu layanan error
  tracking (mis. Sentry atau yang setara) lewat integrasi `tracing`, dengan request id, tanpa data
  sensitif (filter di atas berlaku juga di sana). DSN hanya dari env var; tanpa DSN (dev, test)
  integrasinya mati.
- **Metrik minimal**: jumlah dan durasi request per route dan status, jumlah job per status dan
  umur job tertua di queue, pemakaian pool koneksi database, dan login gagal per menit. Formatnya
  mengikuti platform monitoring yang dipakai (Prometheus, OpenTelemetry). Alert pertama: rasio 5xx,
  umur job tertua, dan pool koneksi yang habis.

> **Opsional, modul Worktree.** Hapus bagian 11 beserta `bin/worktree`, `bin/worktree-id`, `.worktreeinclude`, dan rujukan "bila memakai modul Worktree" bila tim tidak mengerjakan beberapa
> branch secara paralel di satu mesin. Wrapper `bin/dev`, `bin/test`, `bin/loco` tetap dipakai,
> dengan env dari `.env` lokal.

## 11. Kerja paralel dengan git worktree

Tujuannya: beberapa fitur dikerjakan bersamaan, masing-masing di worktree sendiri, tanpa saling
mengganggu dan tanpa ritual setup manual. Satu worktree = satu branch = satu lingkungan lengkap.
`bin/worktree` adalah satu-satunya perintah yang perlu diingat.

**Prinsip**
- **Checkout utama selalu di `staging` dan bersih.** Tidak ada pengembangan fitur di sana. Fungsinya
  dua: sumber data dev untuk worktree baru, dan tempat merge serta migrasi database utama.
- **Satu worktree, satu branch, seumur hidupnya.** Jangan `git checkout` branch lain di dalam
  worktree: database worktree sudah berisi migrasi branch itu, dan branch lain akan bertemu skema
  yang tidak cocok. Fitur lain berarti worktree lain.
- **Worktree berumur pendek.** Dibuat saat fitur mulai, ditutup setelah di-merge.
- **Gagal tertutup.** Bila identitas worktree tidak bisa ditentukan, perintah berhenti, tidak
  pernah jatuh diam-diam ke database checkout utama.
- Setiap worktree berdiri sendiri, jadi aman dikerjakan oleh sesi agent AI yang terpisah. Agent
  bekerja hanya di worktree-nya dan tidak menjalankan perintah di checkout utama.

**Yang diisolasi per worktree, dan alasannya**

| Sumber daya | Per worktree | Alasan |
|---|---|---|
| Folder | `worktrees/<nama>` di dalam checkout utama (gitignored) | mudah ditemukan; dikecualikan dari build Docker dan scan Tailwind |
| Database dev | `<app>_dev_<id>`, **di-clone** dari database dev utama | data dev langsung tersedia; migrasi branch tidak menyentuh database lain |
| Database test | `<app>_test_<id>`, **dibuat kosong** lalu dimigrasi | truncate tes tidak menghapus data tes worktree lain; skema mengikuti migrasi branch |
| Queue job | `queue.uri` = database dev worktree | kalau queue dibagi, worker di worktree B memproses job worktree A dengan kode dan database yang salah |
| Port | port pilihan sendiri; pindah ke port kosong bila terisi | dua server tidak pernah berebut port; URL tetap sama selama port pilihannya kosong |
| Nama cookie sesi | diberi suffix `<id>` di development | browser mengirim cookie `localhost` ke semua port; tanpa suffix, login di satu worktree mengeluarkan sesi di worktree lain |
| Folder `storage/` (bila memakai modul Storage) | salinan hard link dari checkout utama | database hasil clone merujuk file yang harus ada; hard link hampir tanpa biaya disk, dan menghapus file di worktree tidak menghapusnya di checkout utama. Aman karena file storage tidak pernah diubah langsung (§8) |
| `target/` | terpisah (default Cargo); dependency diambil dari cache `sccache` | build paralel tidak saling mengunci dan binary satu branch tidak tertimpa build branch lain; tanpa `sccache`, setiap worktree baru meng-compile ulang semua dependency dari nol (§14) |
| Proses server | PID dicatat di `tmp/worktree.pid` | server dihentikan lewat PID-nya sendiri; mematikan berdasarkan nama proses bisa ikut mematikan server worktree lain |

Yang **dibagi**: binary Tailwind (cache per user, §3), `sccache`, server Postgres, dan mail sink
lokal (§8).

**Identitas worktree**
- `<id>` diturunkan dari nama branch, di **satu tempat** (`bin/worktree-id`): huruf kecil, karakter
  selain `[a-z0-9_]` menjadi `_`, maksimal 38 karakter (lebih panjang dipotong dan diberi hash
  pendek) supaya nama database tidak melewati batas 63 byte Postgres (sesuaikan batas 38 dengan
  panjang `<app>`). Di checkout utama `<id>` kosong; di worktree `<id>` kosong berarti **berhenti**.
- **Nama database** pendek dan seragam: `<app>_dev` dan `<app>_test` di checkout utama (bukan
  `<app>_development` bawaan Loco), `<app>_dev_<id>` dan `<app>_test_<id>` di worktree.
- `<id>` dan port pilihan ditulis sekali ke file `.worktree` (gitignored) di root worktree saat
  dibuat. Semua script membaca file ini, tidak menghitung ulang.
- Nama branch yang ditolak: `staging`, `release`, `main`, `production`, ref git yang tidak valid, dan nama yang
  menghasilkan `<id>` atau folder yang sudah dipakai branch lain (`fitur/a` dan `fitur-a`).

**Port**
- **Port tidak pernah di-hardcode.** Setiap proses di project ini yang membuka port (server Loco,
  dan proses lain yang ditambahkan nanti) mendapat port lewat satu helper yang sama di `bin/`:
  coba port pilihan; bila sudah terisi, naik satu per satu sampai menemukan port kosong. Config
  hanya membaca `PORT` dari env.
- Port pilihan checkout utama adalah **5150** (default Loco). Port pilihan worktree adalah port
  kosong pertama di atas 5150 yang tidak tercatat di worktree lain, disimpan di `.worktree`.
- Bila port pilihan terisi, `bin/dev` memakai port kosong berikutnya **tanpa gagal**, lalu mencetak
  URL yang benar-benar dipakai. Port pilihan di `.worktree` tidak diubah.
- Link yang dibuat aplikasi di development (link di email, link `dev_login`) dibentuk dari `PORT`
  yang terpakai, jadi tetap benar di port berapa pun.

**`bin/worktree`: perintah dan aturannya**

Exit code semua subcommand: **0** berhasil; **1** salah pakai atau prasyarat gagal (tidak ada yang
berubah); **2** gagal di tengah (aman diulang dengan perintah yang sama). Setiap subcommand punya
`--help` yang tidak menjalankan apa pun.

- `bin/worktree new <branch> [--base <branch>] [--no-server]` — **mulai atau lanjutkan** kerja di satu branch. Perintah
  yang sama dipakai keesokan harinya; worktree, database, dan port-nya tetap sama. Urutannya:
  1. validasi nama branch, lalu `doctor --quiet` (prasyarat gagal = berhenti sebelum ada yang dibuat);
  2. worktree untuk branch itu sudah ada: lanjutkan ke langkah 5;
  3. tentukan asal branch: branch lokal ada → pakai; ada di `origin` → lacak `origin/<branch>`;
     tidak ada di mana pun → buat dari `origin/staging` (atau `origin/<base>`; hotfix memakai
     `--base release`) dengan `--no-track` (upstream baru ada setelah
     `git push -u` pertama); `origin` tidak terjangkau → berhenti, supaya tidak menabrak branch
     remote bernama sama;
  4. `git worktree add worktrees/<nama>`, tulis `.worktree`, salin file dari `.worktreeinclude`
     (`.env*`, config lokal, `storage/` dengan hard link bila memakai modul Storage; entri yang
     sudah ada tidak ditimpa);
  5. siapkan database: clone database dev (`createdb -T`, fallback `pg_dump | psql` bila database
     utama sedang dipakai server), buat database test kosong, migrasikan keduanya. Idempoten:
     yang sudah ada dilewati;
  6. cetak ringkasan (branch, folder, URL, nama database, perintah `close`), lalu jalankan `bin/dev`,
     kecuali `--no-server` (untuk agent, otomasi, atau saat hanya mau menjalankan tes).
- `bin/worktree close <branch> [--force] [--keep-db]` — selesai dengan satu branch.
  - Semua cek keselamatan terjadi **sebelum** ada yang diubah. Ditolak bila: dijalankan dari dalam
    worktree yang akan ditutup; ada perubahan belum di-commit (termasuk file untracked); ada commit
    yang belum ada di `origin/<branch>`; branch punya commit sendiri dan belum pernah di-push.
    `--force` melewati cek ini untuk membuang eksperimen.
  - Branch yang remote-nya sudah dihapus (biasanya karena sudah di-merge) **tidak** ditolak: rencana
    menampilkan commit terakhir dan apakah sudah ada di `origin/staging`, lalu meminta konfirmasi.
    Status push dinilai setelah `git fetch --prune`.
  - Rencana (server yang dihentikan, folder, database beserta ukurannya, branch) ditampilkan dulu
    dan diminta konfirmasi.
  - Urutan: hentikan server lewat `tmp/worktree.pid` → drop database dengan `dropdb --force`; **bila
    gagal, berhenti** (exit 2) dan worktree dibiarkan, supaya tidak ada database yatim →
    `git worktree remove` → hapus branch lokal dengan `git branch -d` (squash merge yang ditolak git
    dibiarkan, dengan pesan perintah `-D`).
  - Worktree yang foldernya sudah hilang: `close` hanya membersihkan sisa database-nya.
- `bin/worktree list` — semua worktree: branch, port (`*` bila server sedang jalan), status
  (perubahan, `ter-push`, `N ahead`, `belum di-push`, `remote gone`), ukuran database, ukuran
  `target/`, dan path.
  Hanya membaca.
- `bin/worktree reset-db` — drop lalu clone ulang database dev worktree saat ini dari database
  utama dan jalankan ulang migrasi branch. Dipakai saat data dev kacau atau skema bentrok setelah
  sync dengan `staging`.
- `bin/worktree doctor [--quiet]` — memeriksa prasyarat (git, `origin` terjangkau, `sccache` dan
  `watchexec` terpasang, Postgres jalan,
  koneksi database, extension `vector` tersedia bila memakai modul Vector, file wajib di
  `.worktreeinclude` ada, script berakhiran LF, ukuran `storage/` bila memakai modul Storage) dan
  memberi saran perbaikan untuk tiap temuan. Dipakai di mesin baru atau saat `new` berhenti.
- `bin/worktree prune [--dry-run]` — `git worktree prune`, lalu men-drop database ber-`<id>` yang
  tidak dimiliki worktree mana pun. Tanpa `--force`: database yang masih punya koneksi aktif
  dilewati. Selalu `--dry-run` dulu.
- Membuat dan menutup worktree **selalu** lewat `bin/worktree`. Bila worktree dibuat oleh tool lain
  (agent, aplikasi pengelola worktree), hook setup dan archive tool itu memanggil subcommand
  internal yang sama (`bin/worktree _setup`, `bin/worktree _release`), bukan logika sendiri.

**Perintah sehari-hari: selalu lewat wrapper**
- `bin/dev`, `bin/test`, dan `bin/loco <argumen>` memuat environment dari `.worktree` (`DATABASE_URL`,
  `PORT`, suffix cookie), lalu menjalankan `cargo loco start` / `cargo nextest run` / `cargo loco <argumen>`.
  Ketiganya memakai satu env loader yang sama.
- **Jangan pernah mengetik `cargo loco`, `cargo test`, atau `cargo nextest` langsung**, termasuk di
  checkout utama.
  `development.yaml` dan `test.yaml` sengaja **tidak** memberi default untuk `DATABASE_URL` (§10):
  perintah yang lupa wrapper gagal saat boot, bukan diam-diam memigrasi database utama dengan skema
  branch fitur.

**Siklus kerja**
1. Dari checkout utama: `bin/worktree new feature/document-upload`. Server langsung jalan di
   terminal itu; buka terminal kedua dan editor di folder worktree-nya.
2. Buka URL yang dicetak, lalu masuk dengan `bin/loco task dev_login email:...` (user seed), atau
   lewat form login bila user itu sudah diberi password dengan `user:set_password`.
3. Kerjakan fitur: `bin/loco g migration ...`, `bin/loco db migrate`, `bin/loco db entities`,
   `bin/test <filter>` untuk bagian yang diubah, commit, `git push -u origin feature/document-upload`.
4. Sinkronkan dengan `staging` secara rutin (`git merge origin/staging`), lalu `bin/loco db migrate`. Bila
   skema bentrok atau migrasi gagal, `bin/worktree reset-db`.
5. Sebelum minta review: `bin/ci` hijau di worktree.
6. Merge ke `staging` dengan merge standar **setelah ada go-ahead eksplisit**. Lalu di checkout utama:
   `git pull` dan `bin/loco db migrate`; database utama hanya dimigrasi dari sana.
7. Worktree lain yang masih aktif: `git merge origin/staging` lalu `bin/loco db migrate`.
8. Dari checkout utama: `bin/worktree close feature/document-upload`.

**Script worktree**
- Ditulis dalam bash yang kompatibel bash 3.2 (macOS), tanpa `pkill`/`lsof`; semua path di-quote
  (path dengan spasi pernah membuat file wajib tidak tersalin). `.gitattributes` memaksa LF untuk
  `bin/*`.
- Dites dengan repo git sementara dan Postgres palsu, sehingga tes tidak butuh database sungguhan.
- Script, `.worktreeinclude`, dan perubahannya baru berlaku di worktree setelah di-commit ke
  `staging`, karena worktree dibuat dari commit.
- Seed dev ditambahkan di checkout utama (worktree baru mewarisinya lewat clone), bukan di
  worktree yang akan ditutup.

## 12. Branch, rilis, dan deployment (Docker)

**Branch**

| Branch | Fungsi | Deploy ke |
|---|---|---|
| `staging` | branch integrasi: semua branch fitur berasal dari sini dan di-merge ke sini | staging |
| `release` | hanya berisi yang sudah lolos staging | production |
| `feature/*`, `fix/*` | satu fitur atau perbaikan (satu worktree bila memakai modul Worktree, §11) | tidak di-deploy |

- Tidak ada branch `main`. Branch default repo adalah `staging`.
- **Promosi ke production adalah fast-forward**: `release` dimajukan ke commit `staging` yang sudah
  diuji (`git merge --ff-only`), bukan merge commit baru. Commit SHA production sama persis dengan
  yang diuji di staging, sehingga image Docker yang sama dipakai ulang tanpa build ulang.
- **Hotfix production**: branch `hotfix/*` dari `release` (`bin/worktree new hotfix/x --base release`
  bila memakai modul Worktree), di-merge ke `release`, lalu `release` di-merge balik ke `staging` di
  hari yang sama. Tanpa merge balik, perbaikan hilang di rilis berikutnya dan promosi berikutnya
  tidak bisa fast-forward.
- Tidak ada yang di-push langsung ke `staging` atau `release`; semua lewat merge request.

**Deployment**

- **Satu image, semua environment.** Mulai dari `bin/loco generate deployment` (Docker), lalu
  pakai multi-stage build: stage builder dengan toolchain yang sama dengan `rust-toolchain.toml`
  (bukan versi Rust bawaan Dockerfile hasil generate), stage runtime minimal berisi binary,
  `config/`, dan `assets/`. CSS di-build di stage builder lewat `bin/tailwind --minify`; image
  runtime hanya berisi file CSS hasilnya, tanpa Tailwind CLI dan tanpa `assets/css/`. Environment
  dipilih saat runtime lewat `LOCO_ENV`; image yang sama (di-tag dengan git SHA) dipromosikan
  staging → production mengikuti fast-forward `staging` → `release`.
- **Dua container dari image yang sama**: `web` (`start`) dan `worker` (`start --worker`, ditambah
  scheduler sesuai §8).
- **Migrasi adalah langkah terpisah** sebelum container baru dijalankan
  (`docker run <image> <app>-cli db migrate -e production`), bukan bagian dari startup web.
- `HEALTHCHECK` di Dockerfile memakai `/_health`.
- Secret di-inject sebagai environment variable saat container dijalankan, tidak pernah di-bake ke image.
- Container tidak menyimpan file di disk lokalnya (file user, bila memakai modul Storage, ada di
  S3, §8), sehingga container `web` dan `worker` bisa diganti atau ditambah kapan saja tanpa
  kehilangan data.
- Bila aplikasi berada di belakang reverse proxy, `middlewares.remote_ip.source` diatur sesuai
  proxy itu dan port aplikasi tidak terbuka selain ke proxy (§7). Tanpanya rate limit per IP
  melihat semua request berasal dari satu IP proxy, atau IP bisa dipalsukan.
- Angka non-obvious (pool koneksi × jumlah container vs `max_connections` Postgres) ditulis
  aritmetikanya di komentar.
- Checklist deploy: migrasi kompatibel dengan kode rilis sebelumnya (§4) dan sudah direview;
  build dan push image ber-tag SHA;
  jalankan migrasi; restart `web` dan `worker`; verifikasi `/_health` dan `<app>-cli version`
  (git SHA) di kedua environment.

## 13. Dokumentasi, komentar, commit

- **Komentar menjelaskan kenapa, bukan apa.** Baris non-obvious menyebut aturannya dan apa yang
  rusak tanpanya, dalam beberapa baris, dengan rujukan singkat ke insiden atau bug bila ada.
  **Cerita lengkap** (apa yang diamati, alternatif yang ditolak, sejarah keputusan) **ada di commit
  message**, bukan di komentar.
- **Komentar hanya merujuk hal yang ikut di-commit dan bertahan**: kode lain, tes, atau dokumen di
  repo. Tidak pernah merujuk file yang di-gitignore atau sementara (SPEC, file task, catatan
  diskusi): file itu hilang dan rujukannya menjadi buntu. Tulis aturannya langsung di komentar.
- **Istilah internal didefinisikan sekali** di doc comment modul pemiliknya (`//!`), lalu dipakai
  konsisten. Istilah tanpa definisi ("Pengaman 2") tidak boleh muncul di kode.
- Komentar "dua tempat harus sepakat" hanya untuk hal yang memang tidak bisa disatukan (migrasi
  SQL dan kode Rust, termasuk CHECK constraint enum; kode Rust dan JS; config dan kode). Kode Rust
  yang harus sama disatukan (§2).
- Doc comment (`///`) menempel tepat di atas item yang dijelaskannya. Saat memindah atau menyisipkan
  fungsi, periksa doc comment ikut pindah; doc yang menempel di fungsi yang salah lebih menyesatkan
  daripada tidak ada doc.
- File yang tujuannya tidak jelas dari namanya dibuka dengan doc comment (`//!`).
- Commit message: subjek imperatif yang menjelaskan hasil yang terlihat user, body yang bercerita:
  apa yang diamati, kenapa kode lama begitu, apa yang berubah, apa yang sengaja tidak diubah.
- Checklist review migrasi: kompatibel dengan kode rilis sebelumnya; `lock_timeout` dipasang;
  backfill tabel besar lewat task (§4).
- Checklist review query (singkat): order punya tiebreaker `id`; paginasi feed pakai keyset;
  N count → satu `GROUP BY`; hindari N+1 (`find_with_related`/`load_many`); filter param divalidasi,
  tidak pernah diinterpolasi ke SQL (`AssertSqlSafe` tidak pernah membungkus string berisi input
  user); efek samping setelah commit.

## 14. Kecepatan development

Rust lambat di dua tempat: **build pertama** (ratusan crate dependency, beberapa menit) dan
**siklus edit** (compile ulang crate aplikasi lalu link). Build pertama terjadi sekali per mesin;
yang dioptimasi terus-menerus adalah siklus edit. Targetnya: perubahan template, CSS, JS, dan teks
terlihat tanpa compile; perubahan kode Rust terlihat dalam hitungan detik, bukan menit.

**Ukur dulu, jangan menebak**
- `cargo build --timings` menghasilkan laporan HTML berisi crate yang paling lama di-compile.
  Dijalankan sebelum menambah dependency berat dan saat build terasa melambat.
- **Angka acuan** (starter `loco new` 1.3.1 dengan Postgres, queue Postgres, dan aset
  server-side; Rust 1.99; 4 job paralel; diukur di dalam Docker di laptop 16 core):

  | Langkah | Waktu |
  |---|---|
  | unduh dependency (`cargo fetch`) | ~12 dtk |
  | build pertama (`cargo build`) | ~59 dtk |
  | build tes pertama (`cargo test --no-run`) | ~28 dtk tambahan |
  | rebuild setelah mengubah satu controller | ~1,5 dtk |
  | rebuild tes setelah mengubah satu controller | ~2,2 dtk |
  | `cargo build` tanpa perubahan | ~1,4 dtk |
  | `target/` setelah build dan tes | ~1,9 GB |

  Ini angka aplikasi yang masih kosong. Crate aplikasi yang tumbuh membuat rebuild ikut naik,
  sedangkan build pertama hampir seluruhnya dependency.
- **Anggaran: rebuild incremental setelah mengubah satu handler ≤ 15 detik** di mesin dev yang
  umum di tim (sesuaikan angkanya per tim, lalu tulis di sini). Bila terlampaui secara konsisten,
  lihat "Memecah crate" di bawah.

**Yang tidak butuh compile**
- **Template Tera, CSS, dan JS berubah tanpa restart.** Di build debug, `TeraView` Loco memuat
  ulang template yang berubah (fungsi custom seperti `t` dan `icon` tetap terdaftar), dan Tailwind
  `--watch` membangun ulang CSS dalam milidetik. Browser cukup di-refresh.
- **Terjemahan dimuat saat runtime** (`ArcLoader` dari `fluent-templates` 0.15 dengan feature
  `tera`, sudah dipakai starter `loco new` 1.3 di `src/initializers/view_engine.rs`), bukan
  `static_loader!` yang menanam file `.ftl` ke dalam binary sehingga mengubah satu kata memicu
  compile ulang. Perubahan `.ftl` cukup restart (otomatis, di bawah). `assets/i18n/` ikut masuk
  image runtime (§12), sama seperti `assets/views/` dan `assets/icons/`.
- File Fluent bersama (`shared_resources`) diletakkan di **luar** `assets/i18n/` (starter memakai
  `assets/shared.ftl`): bila di dalamnya, file itu terdaftar dua kali dan boot gagal dengan
  `Failed to add FTL resources to the bundle`.
- Ikon, terjemahan, dan config dibaca sekali saat boot. Perubahannya cukup restart (otomatis, di
  bawah), tanpa compile: cargo hanya memeriksa bahwa binary masih segar.

**Restart otomatis**
- `bin/dev` menjalankan server di bawah `watchexec`:
  ```bash
  watchexec --restart --watch src --watch migration --watch config \
    --watch assets/icons --watch assets/i18n --watch Cargo.toml -- cargo loco start
  ```
  `assets/views/`, `assets/css/`, dan `assets/static/` sengaja tidak di-watch: perubahan di sana
  tidak butuh restart. `cargo-watch` tidak dipakai karena proyeknya sudah tidak dirawat.
- Untuk melihat error compile tanpa menjalankan server, developer boleh memakai `bacon`
  (`cargo check`/`clippy` berjalan terus). Opsional, bukan bagian dari `bin/*`.

**Profil build dev** (di `Cargo.toml`; starter `loco new` 1.3 sudah berisi blok
`[profile.dev.package."*"]`, sisanya ditambahkan. Pada starter yang masih kosong,
`line-tables-only` hanya memangkas rebuild dari ~1,6 ke ~1,4 detik; manfaatnya membesar seiring
crate aplikasi tumbuh.)
```toml
[profile.dev]
# Info debug penuh memperbesar target/ berkali lipat dan memperlambat link.
# line-tables-only tetap memberi nomor baris di backtrace.
debug = "line-tables-only"

[profile.dev.package."*"]
# Dependency tidak di-debug baris per baris; tanpa info debug, link lebih cepat
# dan target/ lebih kecil.
debug = false

[lib]
# Aplikasi tidak memakai doctest; tanpa ini `cargo test` menjalankan rustdoc
# atas seluruh crate setiap kali.
doctest = false
```
- Developer yang butuh debugger dengan info lengkap menimpanya sementara lewat env
  (`CARGO_PROFILE_DEV_DEBUG=true bin/dev`), tidak mengubah `Cargo.toml`.
- **Job paralel: `CARGO_BUILD_JOBS=8`** di image dev (`docker/dev/Dockerfile`), bisa ditimpa per
  mesin lewat `AG_CARGO_JOBS` (`docker/compose.app.yaml`). Angka ini juga membatasi thread codegen
  rustc (jobserver), bukan hanya jumlah crate paralel: dengan 4, compile ulang crate server hanya
  memakai 4 core. Dinaikkan bila RAM lega; link binary tes (~250 MB) butuh beberapa GB.
- **Feature dependency seragam lewat `cargo hakari`** (crate `workspace-hack`, `.config/hakari.toml`).
  Tanpanya feature berbeda antara `cargo loco start` (paket server saja, tanpa dev-dependency),
  `cargo nextest --workspace` (ikut CLI dan dev-dependency), dan clippy: mis. tokio mendapat
  feature `process` dari CLI hanya di build workspace. Setiap perpindahan perintah lalu meng-compile
  ulang dependency dan crate aplikasi. Dengan workspace-hack, `bin/dev` dan `bin/test` memakai
  artifact dependency yang sama. Setelah mengubah dependency:
  `bin/sh cargo hakari generate && bin/sh cargo hakari manage-deps`; `bin/ci` menjalankan
  `cargo hakari verify`. CLI, core, dan graph tidak bergantung pada workspace-hack (CLI dirilis
  lintas OS), dan image produksi mengosongkan workspace-hack (`docker/prod/Dockerfile`) supaya
  feature khusus tes (loco `testing`) tidak ikut ter-compile.
- **Linker tidak dikonfigurasi di repo.** Sejak Rust 1.90, x86_64 Linux sudah memakai `rust-lld`
  secara default (dicek di Rust 1.99: `-fuse-ld=lld`), yang jauh lebih cepat dari linker sistem
  lama. Developer yang mau `mold` atau
  `wild` memasangnya di `~/.cargo/config.toml` miliknya sendiri, supaya repo tidak mewajibkan tool
  yang belum tentu ada di mesin lain dan di image Docker.
- Backend Cranelift tidak dipakai: masih butuh toolchain nightly, sedangkan toolchain dipin ke
  stable (§1).
- `opt-level` dependency tidak dinaikkan secara umum: build pertama jadi lebih lama demi runtime
  dev yang tidak terasa bedanya. **Pengecualiannya argon2**, satu-satunya kode dev yang berat di
  CPU. Tanpa optimasi, satu hash dengan parameter baseline OWASP makan ~440 ms; dengan
  `opt-level = 3` hanya untuk `argon2` dan `blake2`, ~26 ms, setara build release (~24 ms; diukur
  di Rust 1.99, argon2 0.6). Crate-nya kecil, jadi build pertama hampir tidak bertambah:
  ```toml
  [profile.dev.package.argon2]
  # Tanpa optimasi, satu hash baseline OWASP ~440 ms di build debug; dengan ini ~26 ms.
  opt-level = 3
  [profile.dev.package.blake2]
  # Dipakai argon2 secara internal; ikut dioptimasi supaya angka di atas tercapai.
  opt-level = 3
  ```

**Test cepat**
- **Semua integration test masuk satu binary**: satu file root di `tests/` (`tests/mod.rs` bawaan
  Loco) yang men-`mod` subfolder `requests/`, `models/`, `workers/`, `tasks/`, `support/`. Setiap
  file `.rs` di root `tests/` adalah binary tersendiri yang di-link ulang terhadap seluruh aplikasi
  dan dependency-nya; sepuluh file berarti sepuluh kali link.
- **Selama mengerjakan fitur, jalankan tes yang relevan saja**: `bin/test` meneruskan argumen ke
  nextest sebagai filter (`bin/test reset_token`, `bin/test tickets::mockup`). Untuk memeriksa
  compile cukup `bin/sh cargo check -p agentgate-server --tests`. Suite penuh (`bin/test` tanpa
  argumen, ~1–2 menit) dijalankan sekali sebelum commit akhir/merge, dan selalu di `bin/ci`.
  Menjalankan suite penuh setelah setiap perubahan kecil adalah penyebab utama fitur terasa lama.
- Parameter argon2 minimal di `test.yaml` (§7; m = 8 KiB, t = 1: <0,1 ms per hash) dan job
  `ForegroundBlocking` (§8) menjaga tes tetap cepat tanpa mengubah perilaku yang dites.
- Starter memakai feature `testing-core` di dev-dependencies, yang tidak berisi assertion selector
  HTML (`assert_css_*`). Tes halaman di §9 butuh feature `testing`; build tes pertama sedikit
  lebih lama, sekali saja.

**Editor (rust-analyzer)**
- Set `"rust-analyzer.cargo.targetDir": true` di setting editor (boleh di-commit di
  `.vscode/settings.json`). rust-analyzer lalu memakai `target/rust-analyzer/` sendiri, sehingga
  pengecekannya tidak mengunci `target/` dan tidak membatalkan build `bin/dev` setiap kali file
  disimpan.

**Worktree dan disk** (bila memakai modul Worktree, §11)
- **`sccache` wajib.** Env loader `bin/*` men-set `RUSTC_WRAPPER=sccache`, dan `bin/worktree doctor`
  gagal bila `sccache` tidak terpasang. `sccache` menyimpan hasil compile dependency, sehingga
  worktree baru hanya meng-compile crate aplikasi, bukan ratusan dependency dari nol. Crate aplikasi
  sendiri tidak di-cache `sccache` (di-compile incremental), dan itu memang begitu.
- **`target/` tidak dibagi antarworktree** (`CARGO_TARGET_DIR` tidak di-set bersama): build di dua
  worktree saling mengunci, dan binary `target/debug/<app>-cli` satu branch tertimpa build branch
  lain, sehingga server worktree A diam-diam menjalankan kode branch B.
- `target/` satu worktree bisa beberapa GB. Profil di atas menguranginya, `bin/worktree list`
  menampilkan ukurannya, dan worktree ditutup setelah di-merge (§11) supaya disk tidak penuh.
- `target/` checkout utama tidak pernah dihapus otomatis dan cache incremental-nya terus tumbuh
  (pernah mencapai 126 GB, 94 GB di antaranya `debug/incremental`). Cache sebesar itu memperlambat
  I/O build. Periksa dengan `bin/sh du -sh target`; bila di atas ~30 GB, `bin/sh cargo clean`
  (dependency kembali cepat lewat `sccache`).

**Memecah crate**
- **Sudah dilakukan (2026-11-14):** `common/` dan `models/` (beserta `_entities/`) ada di crate
  `agentgate-models` (`server/models/`). Crate server me-re-export keduanya
  (`pub use agentgate_models::{common, models};`), jadi path `crate::models::...` dan
  `crate::common::...` tidak berubah. Mengubah controller, service, atau view tidak lagi
  meng-compile ulang model, dan arah `models` → layer server dijamin compiler. Arah `common` →
  `models` (satu crate) tetap dicek `bin/check-layers server/models/src` di `bin/ci`.
- `bin/loco db entities` menulis ke `src/models/_entities` relatif dari `server/` (path itu
  di-hardcode Loco). `server/src/models` adalah **symlink** ke `server/models/src/models`, jadi hasil
  generate langsung masuk ke crate baru. Symlink itu tidak di-`mod` oleh crate server.
- **Angka acuan setelah perubahan ini** (Agent Gate, ~110 ribu baris; 8 job; Docker, laptop 16 core):
  ubah satu service → build tes ~21 dtk; ubah satu model → ~28 dtk; build `bin/dev` ~18 dtk; suite
  penuh 806 tes paralel ~72 dtk. Sebelumnya (4 job, tanpa workspace-hack, tes berurutan): build
  tes ~3 menit dan suite penuh ~6 menit.
- Pemecahan berikutnya (mis. `services/`) hanya setelah diukur ulang dengan `cargo build --timings`.
  `services/` saat ini tidak bisa dipisah begitu saja: service memanggil worker dan mailer, dan
  worker memanggil service.

**CI dan Docker**
- CI meng-cache `~/.cargo/registry`, `~/.cargo/git`, dan `target/` dengan key dari hash
  `Cargo.lock` dan versi toolchain, supaya CI tidak meng-compile semua dependency di setiap run.
- Dockerfile meng-compile dependency di layer terpisah lewat `cargo-chef` (recipe dibuat dari
  `Cargo.toml` dan `Cargo.lock`). Perubahan kode tanpa perubahan dependency tidak meng-compile ulang
  dependency saat build image.

---

## Ditunda sampai dibutuhkan

Aturan berikut sengaja tidak dimasukkan. Tambahkan hanya saat fitur atau kebutuhannya benar-benar ada:

- **Login SSO/OIDC**: tambahkan saat aplikasi harus login lewat identity provider organisasi.
  Periksa dulu feature `auth_oauth` Loco (dibangun di atas `auth_session`) sebelum menulis sendiri.
  Saat itu: sesi tetap sesi server-side Loco (§7) dan semua provider berakhir di satu entry point
  `auth::sign_in`; tabel `identities` (`user_id`, `provider`, `provider_uid`) dengan unique index
  `(provider, provider_uid)`; login dimulai dengan POST dan form-nya `hx-boost="false"`; transaksi
  (`state`, `nonce`, PKCE `code_verifier`, `return_to`) di cookie terenkripsi berumur pendek yang
  dibaca sekali; `id_token` diverifikasi dengan crate JWT yang matang (algoritma dipin, `iss`,
  `aud`, `exp`, `nonce`), dengan tes penolakan untuk setiap cek; email dari provider hanya
  menautkan ke akun lama bila `email_verified` (account takeover); sesi lokal tidak hidup lebih
  lama dari sesi provider.
- **Autentikasi dua faktor (TOTP atau WebAuthn/passkey)**: saat ada data yang butuh lebih dari
  password. Secret TOTP dienkripsi di aplikasi dengan kunci dari env var, kode pemulihan disimpan
  sebagai digest, dan langkah kedua punya rate limit sendiri.
- **Halaman "sesi aktif"** (daftar perangkat dan tombol revoke per sesi): saat user memintanya.
  Tabel `loco_sessions` milik Loco tidak diubah (§7): user agent dan IP disimpan di data sesi saat
  login, dan daftar sesi per user dibaca lewat API session store bila tersedia di versi Loco yang
  terpakai; bila tidak, project mencatatnya di tabel sendiri.
- **Pemeriksaan password bocor** (mis. API k-anonymity Have I Been Pwned): saat kebijakan keamanan
  memintanya. Panggilan ke API itu best-effort dengan timeout pendek; layanan mati tidak menghalangi
  register atau ganti password.
- **Realtime (WebSocket/push)**: tambahkan aturan channel dan backstop sweep saat fitur realtime
  dibangun. HTMX 4 menyediakan extension `hx-sse` dan `hx-ws` untuk itu.
- **Release notes**: tambahkan saat ada audiens yang membacanya.
- **Audit log** (siapa mengubah apa, kapan): saat ada kebutuhan kepatuhan atau data yang
  perubahannya harus bisa dilacak. Saat itu: satu tabel append-only (`actor_id`, `action`,
  `subject_type`, `subject_id`, `changes` jsonb, `request_id`, `created_at`), ditulis di transaksi
  yang sama dengan perubahannya (bukan lewat job, supaya tidak ada perubahan tanpa jejak), lewat
  satu fungsi di `services/`; aplikasi tidak punya jalur untuk mengubah atau menghapus barisnya.
- **Multi-tenancy** (satu instance untuk banyak organisasi): keputusan arsitektur yang harus
  diambil **sebelum** tabel domain pertama dibuat, karena menambahkannya belakangan berarti
  menyentuh setiap tabel dan query. Bila dibutuhkan: kolom `tenant_id` di setiap tabel domain,
  tenant di-resolve sekali oleh extractor (seperti user), dan setiap query model menerima tenant
  sebagai parameter wajib; pertimbangkan Row Level Security Postgres sebagai lapisan kedua.
- **Presigned URL storage** (modul Storage; `ctx.storage.presign_get`/`presign_put`, Loco 1.2):
  saat ada file yang terlalu besar untuk di-stream lewat aplikasi. URL dibuat hanya setelah cek
  otorisasi yang sama dengan controller unduhan, berumur pendek, dan tidak pernah disimpan atau
  di-log.
- **Data staging di worktree** (modul Worktree; `bin/worktree new --from staging`): tambahkan bila
  dibutuhkan. Bila memakai modul Storage, database staging merujuk file di S3 staging, jadi clone
  database saja menghasilkan dokumen tanpa file.
