//! Tabel data server-side (komponen `datatable`): pencarian, filter, sort, halaman, dan jumlah
//! per halaman dibaca dari query string, divalidasi terhadap `TableSpec` (nilai tak dikenal
//! diabaikan, rules §6), lalu dirangkai menjadi `TableView` berisi semua tautan siap pakai.
//!
//! Alur pemakaian di controller:
//! 1. `let state = SPEC.parse(&pairs)` dari `Query<Vec<(String, String)>>`;
//! 2. query model dengan `state` (`q`, `filter("role")`, `sort`, `desc`, `offset()`, `per`);
//! 3. `TableView::new(&SPEC, &state, base_path, total)` → kirim ke template bersama baris.
use serde::Serialize;

/// Pilihan jumlah baris per halaman.
pub const PER_PAGE: &[u64] = &[10, 25, 50, 100];

/// Kolom tabel. `sort` = kunci sort yang diterima query (`None` = tidak bisa di-sort).
#[derive(Debug, Clone, Copy)]
pub struct ColumnSpec {
    pub label_key: &'static str,
    pub sort: Option<&'static str>,
    /// Sembunyikan di layar kecil (`"md"`) atau sedang (`"lg"`).
    pub hide: Option<&'static str>,
    pub align_right: bool,
}

impl ColumnSpec {
    #[must_use]
    pub const fn new(label_key: &'static str) -> Self {
        Self {
            label_key,
            sort: None,
            hide: None,
            align_right: false,
        }
    }

    #[must_use]
    pub const fn sort(mut self, key: &'static str) -> Self {
        self.sort = Some(key);
        self
    }

    #[must_use]
    pub const fn hide(mut self, at: &'static str) -> Self {
        self.hide = Some(at);
        self
    }

    #[must_use]
    pub const fn right(mut self) -> Self {
        self.align_right = true;
        self
    }
}

/// Kelompok filter (mis. Role). Opsi dikirim terpisah ke `TableView::with_filter_options`
/// karena sering berasal dari database.
#[derive(Debug, Clone, Copy)]
pub struct FilterSpec {
    pub key: &'static str,
    pub label_key: &'static str,
    pub icon: &'static str,
}

/// Definisi tabel: id DOM, kolom, filter, sort bawaan.
#[derive(Debug, Clone, Copy)]
pub struct TableSpec {
    pub id: &'static str,
    pub columns: &'static [ColumnSpec],
    pub filters: &'static [FilterSpec],
    pub default_sort: &'static str,
    pub default_desc: bool,
    pub default_per: u64,
    /// Placeholder kotak cari (Fluent); `None` = tanpa pencarian.
    pub search_key: Option<&'static str>,
}

/// Keadaan tabel hasil parse query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableState {
    pub q: String,
    pub sort: &'static str,
    pub desc: bool,
    pub page: u64,
    pub per: u64,
    /// Nilai filter per kunci `FilterSpec::key` (kosong = tidak difilter).
    pub filters: Vec<(&'static str, Vec<String>)>,
}

impl TableState {
    /// Nilai filter `key` (kosong = semua).
    #[must_use]
    pub fn filter(&self, key: &str) -> &[String] {
        self.filters
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(&[], |(_, v)| v.as_slice())
    }

    /// Kata kunci pencarian (sudah di-trim; `None` bila kosong).
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        Some(self.q.as_str()).filter(|q| !q.is_empty())
    }

    #[must_use]
    pub fn offset(&self) -> u64 {
        (self.page - 1) * self.per
    }

    fn is_filtered(&self) -> bool {
        !self.q.is_empty() || self.filters.iter().any(|(_, v)| !v.is_empty())
    }

    /// Pasangan query string untuk keadaan ini (hanya nilai yang bukan bawaan).
    fn pairs(&self, spec: &TableSpec) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if !self.q.is_empty() {
            out.push(("q".into(), self.q.clone()));
        }
        for (key, values) in &self.filters {
            out.extend(values.iter().map(|v| ((*key).to_string(), v.clone())));
        }
        if self.sort != spec.default_sort || self.desc != spec.default_desc {
            out.push(("sort".into(), self.sort.into()));
            out.push(("dir".into(), if self.desc { "desc" } else { "asc" }.into()));
        }
        if self.per != spec.default_per {
            out.push(("per".into(), self.per.to_string()));
        }
        if self.page > 1 {
            out.push(("page".into(), self.page.to_string()));
        }
        out
    }
}

impl TableSpec {
    /// Parse query string. Kunci sort, arah, per halaman, dan kunci filter yang tidak dikenal
    /// diabaikan; nomor halaman < 1 menjadi 1 (yang melewati halaman terakhir dijepit di
    /// `TableView::new`).
    #[must_use]
    pub fn parse(&self, pairs: &[(String, String)]) -> TableState {
        let get = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim());
        let sort = get("sort")
            .and_then(|s| self.columns.iter().find_map(|c| c.sort.filter(|k| *k == s)))
            .unwrap_or(self.default_sort);
        let desc = match get("dir") {
            Some("desc") => true,
            Some("asc") => false,
            _ => {
                if sort == self.default_sort {
                    self.default_desc
                } else {
                    false
                }
            }
        };
        let per = get("per")
            .and_then(|p| p.parse::<u64>().ok())
            .filter(|p| PER_PAGE.contains(p))
            .unwrap_or(self.default_per);
        let page = get("page")
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(1)
            .max(1);
        let filters = self
            .filters
            .iter()
            .map(|f| {
                let mut values: Vec<String> = pairs
                    .iter()
                    .filter(|(k, v)| k == f.key && !v.trim().is_empty())
                    .map(|(_, v)| v.trim().to_string())
                    .collect();
                values.dedup();
                (f.key, values)
            })
            .collect();
        TableState {
            q: get("q").unwrap_or_default().chars().take(200).collect(),
            sort,
            desc,
            page,
            per,
            filters,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnView {
    pub label_key: &'static str,
    /// Tautan sort (`None` = kolom tidak bisa di-sort).
    pub sort_href: Option<String>,
    /// `asc`/`desc` bila tabel sedang di-sort dengan kolom ini.
    pub sorted: Option<&'static str>,
    pub hide: Option<&'static str>,
    pub align_right: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterOption {
    pub value: String,
    /// Label tampil: teks apa adanya, atau kunci Fluent bila `translate`.
    pub label: String,
    pub translate: bool,
    pub checked: bool,
}

impl FilterOption {
    /// Opsi berlabel teks apa adanya (nama tim, nama role custom).
    #[must_use]
    pub fn raw(value: &str, label: &str) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
            translate: false,
            checked: false,
        }
    }

    /// Opsi berlabel kunci Fluent.
    #[must_use]
    pub fn key(value: &str, label_key: &str) -> Self {
        Self {
            translate: true,
            ..Self::raw(value, label_key)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterGroupView {
    pub key: &'static str,
    pub label_key: &'static str,
    pub icon: &'static str,
    pub options: Vec<FilterOption>,
}

/// Chip filter aktif dengan tautan untuk melepasnya.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveFilter {
    /// Nama parameter query filter ini.
    pub key: &'static str,
    pub label_key: &'static str,
    /// Nilai mentah di query string.
    pub raw: String,
    /// Label tampil (nilai mentah sampai `with_filter_options` memberi labelnya).
    pub value: String,
    pub translate: bool,
    pub remove_href: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageLink {
    /// Nomor halaman; `None` = elipsis.
    pub number: Option<u64>,
    pub href: String,
    pub current: bool,
}

/// Data lengkap komponen `datatable`.
#[derive(Debug, Clone, Serialize)]
pub struct TableView {
    pub id: &'static str,
    pub action: String,
    pub search_key: Option<&'static str>,
    pub q: String,
    pub columns: Vec<ColumnView>,
    pub filters: Vec<FilterGroupView>,
    pub active: Vec<ActiveFilter>,
    pub active_count: usize,
    pub clear_href: String,
    /// Isian tersembunyi agar form cari/filter tidak menghilangkan sort dan per halaman.
    pub hidden: Vec<(String, String)>,
    pub total: u64,
    pub from: u64,
    pub to: u64,
    pub page: u64,
    pub pages: u64,
    pub per: u64,
    pub per_options: &'static [u64],
    pub page_links: Vec<PageLink>,
    pub prev_href: Option<String>,
    pub next_href: Option<String>,
    /// Ada pencarian/filter aktif (empty state "tidak ada yang cocok" alih-alih "belum ada").
    pub filtered: bool,
}

impl TableView {
    /// Rangkai tampilan tabel. `base` = path halaman tanpa query; `total` = jumlah baris yang
    /// cocok dengan filter. Halaman di luar jangkauan dijepit ke halaman terakhir.
    #[must_use]
    pub fn new(spec: &TableSpec, state: &TableState, base: &str, total: u64) -> Self {
        let pages = total.div_ceil(state.per).max(1);
        let state = TableState {
            page: state.page.min(pages),
            ..state.clone()
        };
        let href = |s: &TableState| {
            let pairs = s.pairs(spec);
            if pairs.is_empty() {
                base.to_string()
            } else {
                format!(
                    "{base}?{}",
                    serde_urlencoded::to_string(&pairs).unwrap_or_default()
                )
            }
        };
        let first_page = |s: &TableState| TableState {
            page: 1,
            ..s.clone()
        };
        let columns = spec
            .columns
            .iter()
            .map(|c| ColumnView {
                label_key: c.label_key,
                sort_href: c.sort.map(|key| {
                    let desc = state.sort == key && !state.desc;
                    href(&TableState {
                        sort: key,
                        desc,
                        ..first_page(&state)
                    })
                }),
                sorted: c
                    .sort
                    .filter(|k| *k == state.sort)
                    .map(|_| if state.desc { "desc" } else { "asc" }),
                hide: c.hide,
                align_right: c.align_right,
            })
            .collect();
        let page_href = |n: u64| href(&TableState { page: n, ..state.clone() });
        let mut page_links: Vec<PageLink> = Vec::new();
        for n in 1..=pages {
            if n == 1 || n == pages || n.abs_diff(state.page) <= 1 {
                page_links.push(PageLink {
                    number: Some(n),
                    href: page_href(n),
                    current: n == state.page,
                });
            } else if page_links.last().is_some_and(|l| l.number.is_some()) {
                page_links.push(PageLink {
                    number: None,
                    href: String::new(),
                    current: false,
                });
            }
        }
        let mut hidden = Vec::new();
        if state.sort != spec.default_sort || state.desc != spec.default_desc {
            hidden.push(("sort".to_string(), state.sort.to_string()));
            hidden.push((
                "dir".to_string(),
                if state.desc { "desc" } else { "asc" }.to_string(),
            ));
        }
        let from = if total == 0 { 0 } else { state.offset() + 1 };
        Self {
            id: spec.id,
            action: base.to_string(),
            search_key: spec.search_key,
            q: state.q.clone(),
            columns,
            filters: spec
                .filters
                .iter()
                .map(|f| FilterGroupView {
                    key: f.key,
                    label_key: f.label_key,
                    icon: f.icon,
                    options: Vec::new(),
                })
                .collect(),
            active: Vec::new(),
            active_count: 0,
            clear_href: href(&TableState {
                q: String::new(),
                filters: Vec::new(),
                ..first_page(&state)
            }),
            hidden,
            total,
            from,
            to: (state.offset() + state.per).min(total),
            page: state.page,
            pages,
            per: state.per,
            per_options: PER_PAGE,
            prev_href: (state.page > 1).then(|| page_href(state.page - 1)),
            next_href: (state.page < pages).then(|| page_href(state.page + 1)),
            page_links,
            filtered: state.is_filtered(),
        }
        .with_state_filters(spec, &state, &href)
    }

    fn with_state_filters(
        mut self,
        spec: &TableSpec,
        state: &TableState,
        href: &dyn Fn(&TableState) -> String,
    ) -> Self {
        for f in spec.filters {
            for value in state.filter(f.key) {
                let mut rest = state.clone();
                rest.page = 1;
                if let Some((_, values)) = rest.filters.iter_mut().find(|(k, _)| *k == f.key) {
                    values.retain(|v| v != value);
                }
                self.active.push(ActiveFilter {
                    key: f.key,
                    label_key: f.label_key,
                    raw: value.clone(),
                    value: value.clone(),
                    translate: false,
                    remove_href: href(&rest),
                });
            }
        }
        self.active_count = spec
            .filters
            .iter()
            .filter(|f| !state.filter(f.key).is_empty())
            .count();
        self
    }

    /// Pasang opsi filter `key`; opsi yang sedang dipilih dicentang dan chip aktifnya memakai
    /// label opsi (bukan nilai mentah).
    #[must_use]
    pub fn with_filter_options(mut self, key: &str, mut options: Vec<FilterOption>) -> Self {
        let Some(group) = self.filters.iter_mut().find(|g| g.key == key) else {
            return self;
        };
        let label_key = group.label_key;
        for chip in self.active.iter_mut().filter(|a| a.label_key == label_key) {
            if let Some(o) = options.iter().find(|o| o.value == chip.raw) {
                chip.value.clone_from(&o.label);
                chip.translate = o.translate;
            }
        }
        for o in &mut options {
            o.checked = self
                .active
                .iter()
                .any(|a| a.label_key == label_key && a.raw == o.value);
        }
        group.options = options;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLS: &[ColumnSpec] = &[
        ColumnSpec::new("name").sort("name"),
        ColumnSpec::new("joined").sort("joined"),
        ColumnSpec::new("actions"),
    ];
    const FILTERS: &[FilterSpec] = &[FilterSpec {
        key: "role",
        label_key: "role",
        icon: "key",
    }];
    const SPEC: TableSpec = TableSpec {
        id: "t",
        columns: COLS,
        filters: FILTERS,
        default_sort: "name",
        default_desc: false,
        default_per: 10,
        search_key: Some("search"),
    };

    fn pairs(raw: &[(&str, &str)]) -> Vec<(String, String)> {
        raw.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect()
    }

    #[test]
    fn unknown_values_fall_back_to_defaults() {
        let s = SPEC.parse(&pairs(&[("sort", "password"), ("per", "7"), ("page", "-3"), ("dir", "up")]));
        assert_eq!((s.sort, s.desc, s.per, s.page), ("name", false, 10, 1));
    }

    #[test]
    fn filters_and_search_are_kept() {
        let s = SPEC.parse(&pairs(&[("q", " budi "), ("role", "a"), ("role", "b"), ("x", "y")]));
        assert_eq!(s.query(), Some("budi"));
        assert_eq!(s.filter("role"), ["a", "b"]);
    }

    #[test]
    fn view_builds_links_and_clamps_page() {
        let s = SPEC.parse(&pairs(&[("page", "9"), ("role", "a")]));
        let v = TableView::new(&SPEC, &s, "/m", 23);
        assert_eq!((v.page, v.pages, v.from, v.to), (3, 3, 21, 23));
        assert_eq!(v.prev_href.as_deref(), Some("/m?role=a&page=2"));
        assert!(v.next_href.is_none());
        assert_eq!(v.columns[0].sorted, Some("asc"));
        assert_eq!(v.columns[0].sort_href.as_deref(), Some("/m?role=a&sort=name&dir=desc"));
        assert_eq!(v.active[0].remove_href, "/m");
        assert_eq!(v.clear_href, "/m");
    }

    #[test]
    fn filter_options_rename_chips_and_check() {
        let s = SPEC.parse(&pairs(&[("role", "id-1")]));
        let v = TableView::new(&SPEC, &s, "/m", 1)
            .with_filter_options("role", vec![FilterOption::raw("id-1", "Admin"), FilterOption::raw("id-2", "Viewer")]);
        assert_eq!(v.active[0].value, "Admin");
        assert!(v.filters[0].options[0].checked);
        assert!(!v.filters[0].options[1].checked);
    }
}
