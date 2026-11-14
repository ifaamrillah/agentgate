//! Katalog dokumen knowledge bawaan (spec §"Knowledge base → Katalog dokumen"): 10 kelompok dan
//! jenis dokumennya. Data murni; dipakai service (scan, relevansi), view (label, ikon), dan MCP.
//! Jenis custom per workspace ada di `knowledge_types` dan selalu memakai salah satu kelompok ini.

/// Kelompok dokumen. `key` = nilai kolom `group_key` dan segmen URL `/knowledge/g/{key}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    /// Deskripsi singkat untuk kepala halaman kelompok.
    pub desc: &'static str,
}

/// Jenis dokumen bawaan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocType {
    pub key: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    /// Ringkasan isi (dipakai prompt scan dan dialog Dokumen baru).
    pub hint: &'static str,
    /// Satu dokumen per modul/service (bisa lebih dari satu dokumen jenis ini).
    pub per_module: bool,
}

pub const GROUPS: &[Group] = &[
    Group {
        key: "gambaran-umum",
        name: "Gambaran umum",
        icon: "layout-dashboard",
        desc: "Ringkasan untuk anggota baru dan agent AI.",
    },
    Group {
        key: "arsitektur",
        name: "Arsitektur",
        icon: "hierarchy-2",
        desc: "Keputusan dan struktur sistem yang jarang berubah.",
    },
    Group {
        key: "domain-bisnis",
        name: "Domain & bisnis",
        icon: "building",
        desc: "Aturan bisnis yang harus dipatuhi kode.",
    },
    Group {
        key: "backend-kode",
        name: "Backend & kode",
        icon: "server",
        desc: "Struktur kode, data, API, dan proses latar.",
    },
    Group {
        key: "frontend",
        name: "Frontend",
        icon: "layout-sidebar",
        desc: "Halaman, komponen, dan state aplikasi web.",
    },
    Group {
        key: "mobile",
        name: "Mobile",
        icon: "device-desktop",
        desc: "Screen, navigasi, dan rilis aplikasi mobile.",
    },
    Group {
        key: "kualitas",
        name: "Kualitas",
        icon: "shield-check",
        desc: "Standar kualitas yang dicek di setiap PR.",
    },
    Group {
        key: "operasional",
        name: "Operasional",
        icon: "settings",
        desc: "Cara menjalankan dan merawat sistem.",
    },
    Group {
        key: "data",
        name: "Data",
        icon: "database",
        desc: "Pipeline data, event analytics, dan laporan.",
    },
    Group {
        key: "riwayat",
        name: "Riwayat",
        icon: "history",
        desc: "Catatan yang dihasilkan dari tiket Done dan rilis.",
    },
];

const fn t(
    key: &'static str,
    name: &'static str,
    group: &'static str,
    hint: &'static str,
) -> DocType {
    DocType {
        key,
        name,
        group,
        hint,
        per_module: false,
    }
}

pub const TYPES: &[DocType] = &[
    t(
        "overview",
        "Overview Project",
        "gambaran-umum",
        "Tujuan produk, pengguna, fitur utama, dan cara membaca knowledge ini",
    ),
    t(
        "peta-repo",
        "Peta Repo",
        "gambaran-umum",
        "Repo project, isi folder utama, dan hubungan antar repo",
    ),
    t(
        "tech-stack",
        "Tech Stack & Dependencies",
        "gambaran-umum",
        "Bahasa, framework, dependensi penting beserta versinya",
    ),
    t(
        "glosarium",
        "Glosarium",
        "gambaran-umum",
        "Istilah domain dan singkatan",
    ),
    t(
        "arsitektur",
        "Arsitektur Sistem",
        "arsitektur",
        "Komponen, layanan, dan diagram Mermaid arsitektur",
    ),
    t(
        "alur-data",
        "Alur Data",
        "arsitektur",
        "Perjalanan data antar komponen",
    ),
    t(
        "integrasi",
        "Integrasi Pihak Ketiga",
        "arsitektur",
        "Layanan luar, cara autentikasi, callback, dan retry",
    ),
    t(
        "adr",
        "ADR",
        "arsitektur",
        "Keputusan arsitektur beserta alasan dan alternatifnya",
    ),
    t(
        "alur-bisnis",
        "Alur Bisnis / User Journey",
        "domain-bisnis",
        "Langkah pengguna utama dari awal sampai selesai",
    ),
    t(
        "aturan-bisnis",
        "Aturan Bisnis",
        "domain-bisnis",
        "Aturan yang ditegakkan kode (validasi, perhitungan, batas)",
    ),
    t(
        "siklus-status",
        "Siklus Status",
        "domain-bisnis",
        "Status entitas utama dan transisinya",
    ),
    t(
        "role-permission",
        "Role & Permission aplikasi",
        "domain-bisnis",
        "Peran pengguna aplikasi dan hak aksesnya",
    ),
    DocType {
        key: "modul",
        name: "Modul/Service",
        group: "backend-kode",
        hint: "Tanggung jawab, entry point, dan dependensi satu modul",
        per_module: true,
    },
    t(
        "model-data",
        "Model Data & Database",
        "backend-kode",
        "Tabel/entitas utama dan relasinya",
    ),
    t(
        "migration",
        "Riwayat Migration",
        "backend-kode",
        "Perubahan skema penting dan alasannya",
    ),
    t(
        "api",
        "API",
        "backend-kode",
        "Endpoint, method, autentikasi, dan contoh",
    ),
    t(
        "event-job",
        "Event, Queue, Job & Cron",
        "backend-kode",
        "Event, antrean, worker, dan jadwal",
    ),
    t(
        "shared-library",
        "Shared Library",
        "backend-kode",
        "Kode bersama antar modul/repo",
    ),
    t(
        "error-logging",
        "Error Handling & Logging",
        "backend-kode",
        "Pola error, pesan ke pengguna, dan log",
    ),
    t(
        "routing",
        "Halaman & Routing",
        "frontend",
        "Daftar halaman, rute, dan guard",
    ),
    t(
        "komponen-ui",
        "Komponen UI & Design System",
        "frontend",
        "Komponen, token warna, dan tipografi",
    ),
    t(
        "state",
        "State Management",
        "frontend",
        "Store, cache, dan alur data di klien",
    ),
    t(
        "i18n",
        "i18n",
        "frontend",
        "Bahasa yang didukung dan cara menambah teks",
    ),
    t(
        "screen-navigasi",
        "Screen & Navigasi",
        "mobile",
        "Screen aplikasi mobile dan navigasinya",
    ),
    t(
        "fitur-platform",
        "Fitur khusus platform",
        "mobile",
        "Kemampuan per platform (kamera, notifikasi, dsb.)",
    ),
    t(
        "rilis-store",
        "Build & rilis ke store",
        "mobile",
        "Langkah build dan rilis ke App Store/Play Store",
    ),
    t(
        "testing",
        "Strategi Testing",
        "kualitas",
        "Jenis tes, cara menjalankan, dan cakupan",
    ),
    t(
        "konvensi",
        "Konvensi & Coding Style",
        "kualitas",
        "Konvensi kode, struktur folder, dan cara kerja tim",
    ),
    t(
        "security",
        "Security",
        "kualitas",
        "Autentikasi, otorisasi, data sensitif, dan ancaman",
    ),
    t(
        "performance",
        "Performance",
        "kualitas",
        "Titik lambat, batas, dan cara mengukur",
    ),
    t(
        "technical-debt",
        "Technical Debt",
        "kualitas",
        "Utang teknis, TODO, dan kode mati",
    ),
    t(
        "setup",
        "Setup & Cara Menjalankan",
        "operasional",
        "Prasyarat, instalasi, dan perintah menjalankan",
    ),
    t(
        "konfigurasi",
        "Konfigurasi & Env Var",
        "operasional",
        "Variabel lingkungan dan konfigurasi (tanpa nilai secret)",
    ),
    t(
        "ci-cd",
        "Build & CI/CD",
        "operasional",
        "Pipeline build, tes, dan rilis",
    ),
    t(
        "deployment",
        "Deployment & Infrastruktur",
        "operasional",
        "Lingkungan, infrastruktur, dan cara deploy",
    ),
    t(
        "monitoring",
        "Monitoring",
        "operasional",
        "Metrik, log, dan alert",
    ),
    t(
        "runbook",
        "Runbook",
        "operasional",
        "Langkah menangani insiden yang sering terjadi",
    ),
    t(
        "data-pipeline",
        "Data Pipeline/ETL",
        "data",
        "Sumber, transformasi, dan jadwal data",
    ),
    t(
        "analytics-event",
        "Analytics Event",
        "data",
        "Event analytics dan propertinya",
    ),
    t(
        "reporting",
        "Reporting",
        "data",
        "Laporan dan sumber datanya",
    ),
    t(
        "changelog",
        "Changelog per rilis",
        "riwayat",
        "Perubahan per rilis",
    ),
    t(
        "peta-fitur",
        "Peta Fitur",
        "riwayat",
        "Fitur ↔ kode ↔ tiket",
    ),
];

/// Kelompok berdasarkan kunci.
#[must_use]
pub fn group(key: &str) -> Option<&'static Group> {
    GROUPS.iter().find(|g| g.key == key)
}

/// Jenis bawaan berdasarkan kunci.
#[must_use]
pub fn builtin(key: &str) -> Option<&'static DocType> {
    TYPES.iter().find(|t| t.key == key)
}

/// Jenis bawaan di satu kelompok.
pub fn types_in(group: &str) -> impl Iterator<Item = &'static DocType> + '_ {
    TYPES.iter().filter(move |t| t.group == group)
}

/// Jenis dokumen tempat aturan lama repo diserap (`agentgate init`).
pub const CONVENTIONS: &str = "konvensi";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_has_known_group_and_unique_key() {
        let mut keys = std::collections::HashSet::new();
        for t in TYPES {
            assert!(group(t.group).is_some(), "{}", t.key);
            assert!(keys.insert(t.key), "duplikat {}", t.key);
        }
        for g in GROUPS {
            assert!(types_in(g.key).next().is_some(), "{} kosong", g.key);
        }
        assert!(builtin(CONVENTIONS).is_some());
    }
}
