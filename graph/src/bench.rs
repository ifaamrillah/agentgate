//! Benchmark call graph: korpus kecil per bahasa prioritas dengan ground truth edge yang ditulis
//! tangan. Recall = edge benar yang ditemukan / semua edge benar; presisi = edge benar / semua edge
//! yang ditemukan. Dijalankan di CI (tes) dan di halaman Status index → Benchmark.
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::{
    build::{EdgeKind, SourceFile, build},
    extract,
};

/// Satu kasus: file + edge `stem:qualified -> stem:qualified`.
pub struct Case {
    pub lang: &'static str,
    pub files: &'static [(&'static str, &'static str)],
    pub expected: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LangResult {
    pub lang: String,
    pub expected: u32,
    pub found: u32,
    pub correct: u32,
    pub missed: Vec<String>,
    pub wrong: Vec<String>,
    pub micros: u64,
}

impl LangResult {
    #[must_use]
    pub fn recall(&self) -> f64 {
        if self.expected == 0 {
            100.0
        } else {
            f64::from(self.correct) * 100.0 / f64::from(self.expected)
        }
    }

    #[must_use]
    pub fn precision(&self) -> f64 {
        if self.found == 0 {
            100.0
        } else {
            f64::from(self.correct) * 100.0 / f64::from(self.found)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub langs: Vec<LangResult>,
    pub recall: f64,
    pub precision: f64,
    pub files: u32,
    pub micros: u64,
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split_once('.').map_or(name, |(s, _)| s)
}

/// Jalankan semua kasus.
#[must_use]
pub fn run() -> Report {
    let mut langs = Vec::new();
    let mut files_n = 0u32;
    let t0 = Instant::now();
    for case in CASES {
        let t = Instant::now();
        let files: Vec<SourceFile> = case
            .files
            .iter()
            .filter_map(|(p, s)| {
                extract(p, s).ok().map(|facts| SourceFile {
                    path: (*p).to_string(),
                    facts,
                })
            })
            .collect();
        files_n += u32::try_from(files.len()).unwrap_or(0);
        let g = build(&files, None);
        let label = |n: u32| {
            let node = &g.nodes[n as usize];
            format!(
                "{}:{}",
                stem(&files[node.file as usize].path),
                node.qualified
            )
        };
        let mut found: Vec<String> = g
            .edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::Calls
                    && e.from != e.to
                    && g.nodes[e.from as usize].symbol.is_some()
            })
            .map(|e| format!("{} -> {}", label(e.from), label(e.to)))
            .collect();
        found.sort();
        found.dedup();
        let expected: Vec<String> = case.expected.iter().map(|s| (*s).to_string()).collect();
        let correct = found.iter().filter(|f| expected.contains(f)).count();
        let missed: Vec<String> = expected
            .iter()
            .filter(|e| !found.contains(e))
            .cloned()
            .collect();
        let wrong: Vec<String> = found
            .iter()
            .filter(|f| !expected.contains(f))
            .cloned()
            .collect();
        langs.push(LangResult {
            lang: case.lang.to_string(),
            expected: u32::try_from(expected.len()).unwrap_or(0),
            found: u32::try_from(found.len()).unwrap_or(0),
            correct: u32::try_from(correct).unwrap_or(0),
            missed,
            wrong,
            micros: u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX),
        });
    }
    let (e, f, c) = langs.iter().fold((0u32, 0u32, 0u32), |a, l| {
        (a.0 + l.expected, a.1 + l.found, a.2 + l.correct)
    });
    Report {
        recall: if e == 0 {
            100.0
        } else {
            f64::from(c) * 100.0 / f64::from(e)
        },
        precision: if f == 0 {
            100.0
        } else {
            f64::from(c) * 100.0 / f64::from(f)
        },
        langs,
        files: files_n,
        micros: u64::try_from(t0.elapsed().as_micros()).unwrap_or(u64::MAX),
    }
}

/// Korpus. Pola yang diuji per bahasa: call di file yang sama, import bernama/alias/namespace,
/// `this`/`self`, method statis, konstruktor, paket satu direktori, call ke library luar (tidak
/// boleh menjadi edge), dan nama yang sama di dua file (harus memilih yang diimpor).
pub const CASES: &[Case] = &[
    Case {
        lang: "TypeScript",
        files: &[
            (
                "src/cart/promo.ts",
                r#"import { roundRupiah, capDiscount } from "./money";
import * as api from "../api/promoApi";
import { format } from "../util/format";
import dayjs from "dayjs";

export function calcPromo(items: number[], code: string): number {
  if (!isActive(code)) return 0;
  const total = items.reduce((s, x) => s + applyItem(x), 0);
  api.validate(code);
  log(format(total), dayjs().toString());
  return roundRupiah(capDiscount(total));
}
function applyItem(x: number) { return roundRupiah(x * 0.9); }
function isActive(code: string) { return code.length > 0; }
function log(...a: unknown[]) { console.log(a); }
export class PromoStore {
  private items: number[] = [];
  add(x: number) { this.items.push(x); this.recalc(); }
  recalc() { return calcPromo(this.items, "X"); }
}
"#,
            ),
            (
                "src/cart/money.ts",
                "export function roundRupiah(x: number) { return Math.round(x); }\nexport function capDiscount(x: number) { return Math.min(x, 50000); }\n",
            ),
            (
                "src/api/promoApi.ts",
                "import { http } from './http';\nexport async function validate(code: string) { return http.post(`/v1/promo/${code}`); }\n",
            ),
            (
                "src/api/http.ts",
                "export const http = { post: (u: string) => fetch(u) };\n",
            ),
            (
                "src/util/format.ts",
                "export function format(x: number) { return x.toFixed(0); }\n",
            ),
            (
                "src/legacy/format.ts",
                "export function format(x: number) { return String(x); }\n",
            ),
        ],
        expected: &[
            "promo:calcPromo -> promo:isActive",
            "promo:calcPromo -> promo:applyItem",
            "promo:calcPromo -> promoApi:validate",
            "promoApi:validate -> http:post",
            "promo:calcPromo -> promo:log",
            "promo:calcPromo -> format:format",
            "promo:calcPromo -> money:roundRupiah",
            "promo:calcPromo -> money:capDiscount",
            "promo:applyItem -> money:roundRupiah",
            "promo:PromoStore.add -> promo:PromoStore.recalc",
            "promo:PromoStore.recalc -> promo:calcPromo",
        ],
    },
    Case {
        lang: "JavaScript",
        files: &[
            (
                "api/payment.js",
                r#"const { settle, refund } = require("./ledger");
const notify = require("./notify");
const axios = require("axios");
async function createQris(req) {
  const amount = parseAmount(req.body);
  const qr = await axios.post("https://api.xendit.co/qr", { amount });
  settle(qr.id, amount);
  notify.send(req.user, qr.id);
  return qr;
}
function parseAmount(body) { return Number(body.amount); }
function cancel(id) { return refund(id); }
module.exports = { createQris, cancel };
"#,
            ),
            (
                "api/ledger.js",
                "function settle(id, a) { return write(id, a); }\nfunction refund(id) { return write(id, 0); }\nfunction write(id, a) { return { id, a }; }\nmodule.exports = { settle, refund };\n",
            ),
            (
                "api/notify.js",
                "function send(user, id) { return format(user, id); }\nfunction format(u, i) { return `${u}:${i}`; }\nmodule.exports = { send };\n",
            ),
        ],
        expected: &[
            "payment:createQris -> payment:parseAmount",
            "payment:createQris -> ledger:settle",
            "payment:createQris -> notify:send",
            "payment:cancel -> ledger:refund",
            "ledger:settle -> ledger:write",
            "ledger:refund -> ledger:write",
            "notify:send -> notify:format",
        ],
    },
    Case {
        lang: "Python",
        files: &[
            (
                "app/promo.py",
                r#"from .money import round_rupiah, cap
from . import repo
import requests


class PromoService:
    def __init__(self, store):
        self.store = store

    def apply(self, items, code):
        promo = repo.find(code)
        if not self._active(promo):
            return 0
        total = sum(self._line(i) for i in items)
        requests.post("https://audit.example.com", json={"code": code})
        return round_rupiah(cap(total))

    def _active(self, promo):
        return promo is not None

    def _line(self, item):
        return round_rupiah(item * 0.9)


def build_service(store):
    svc = PromoService(store)
    return svc
"#,
            ),
            (
                "app/money.py",
                "def round_rupiah(x):\n    return round(x)\n\ndef cap(x):\n    return min(x, 50000)\n",
            ),
            (
                "app/repo.py",
                "def find(code):\n    return _load(code)\n\ndef _load(code):\n    return {\"code\": code}\n",
            ),
        ],
        expected: &[
            "promo:PromoService.apply -> repo:find",
            "promo:PromoService.apply -> promo:PromoService._active",
            "promo:PromoService.apply -> promo:PromoService._line",
            "promo:PromoService.apply -> money:round_rupiah",
            "promo:PromoService.apply -> money:cap",
            "promo:PromoService._line -> money:round_rupiah",
            "promo:build_service -> promo:PromoService",
            "repo:find -> repo:_load",
        ],
    },
    Case {
        lang: "Go",
        files: &[
            (
                "internal/payment/qris.go",
                r#"package payment

import (
	"fmt"
	"github.com/acme/kasir-api/internal/ledger"
)

type Service struct{ repo Repo }

func NewService(r Repo) *Service { return &Service{repo: r} }

func (s *Service) CreateQris(amount int64) (string, error) {
	if err := validate(amount); err != nil {
		return "", err
	}
	id := s.nextID()
	ledger.Record(id, amount)
	fmt.Println(id)
	return id, nil
}

func (s *Service) nextID() string { return s.repo.Next() }
"#,
            ),
            (
                "internal/payment/validate.go",
                "package payment\n\nimport \"errors\"\n\nfunc validate(a int64) error {\n\tif a <= 0 {\n\t\treturn errors.New(\"invalid\")\n\t}\n\treturn nil\n}\n",
            ),
            (
                "internal/payment/repo.go",
                "package payment\n\ntype Repo interface{ Next() string }\n",
            ),
            (
                "internal/ledger/ledger.go",
                "package ledger\n\nfunc Record(id string, a int64) { write(id, a) }\n\nfunc write(id string, a int64) {}\n",
            ),
        ],
        expected: &[
            "qris:Service.CreateQris -> validate:validate",
            "qris:Service.CreateQris -> qris:Service.nextID",
            "qris:Service.CreateQris -> ledger:Record",
            "ledger:Record -> ledger:write",
        ],
    },
    Case {
        lang: "Rust",
        files: &[
            (
                "src/promo.rs",
                r#"use crate::money::{round, cap};
use std::collections::HashMap;

pub struct Promo {
    rules: HashMap<String, i64>,
}

impl Promo {
    pub fn new() -> Self {
        Self { rules: HashMap::new() }
    }

    pub fn apply(&self, total: i64) -> i64 {
        let d = self.discount(total);
        round(cap(total - d))
    }

    fn discount(&self, total: i64) -> i64 {
        helper(total)
    }
}

fn helper(x: i64) -> i64 {
    x / 10
}

pub fn default_promo() -> i64 {
    Promo::new().apply(100)
}
"#,
            ),
            (
                "src/money.rs",
                "pub fn round(x: i64) -> i64 { x }\npub fn cap(x: i64) -> i64 { clamp(x, 50_000) }\nfn clamp(x: i64, m: i64) -> i64 { x.min(m) }\n",
            ),
        ],
        expected: &[
            "promo:Promo.apply -> promo:Promo.discount",
            "promo:Promo.apply -> money:round",
            "promo:Promo.apply -> money:cap",
            "promo:Promo.discount -> promo:helper",
            "promo:default_promo -> promo:Promo.new",
            "promo:default_promo -> promo:Promo.apply",
            "money:cap -> money:clamp",
        ],
    },
    Case {
        lang: "Java",
        files: &[
            (
                "src/main/java/id/acme/promo/PromoService.java",
                r#"package id.acme.promo;

import id.acme.money.Money;
import java.util.List;

public class PromoService {
    private final PromoRepo repo;

    public PromoService(PromoRepo repo) { this.repo = repo; }

    public long apply(List<Long> items, String code) {
        Promo p = repo.find(code);
        if (!isActive(p)) return 0;
        long total = sum(items);
        return Money.round(Money.cap(total));
    }

    private boolean isActive(Promo p) { return p != null; }

    private long sum(List<Long> items) { return items.stream().mapToLong(x -> x).sum(); }
}
"#,
            ),
            (
                "src/main/java/id/acme/promo/PromoRepo.java",
                "package id.acme.promo;\n\npublic class PromoRepo {\n    public Promo find(String code) { return new Promo(code); }\n}\n",
            ),
            (
                "src/main/java/id/acme/promo/Promo.java",
                "package id.acme.promo;\n\npublic class Promo {\n    public Promo(String code) {}\n}\n",
            ),
            (
                "src/main/java/id/acme/money/Money.java",
                "package id.acme.money;\n\npublic class Money {\n    public static long round(long x) { return x; }\n    public static long cap(long x) { return Math.min(x, 50000); }\n}\n",
            ),
        ],
        expected: &[
            "PromoService:PromoService.apply -> PromoRepo:PromoRepo.find",
            "PromoService:PromoService.apply -> PromoService:PromoService.isActive",
            "PromoService:PromoService.apply -> PromoService:PromoService.sum",
            "PromoService:PromoService.apply -> Money:Money.round",
            "PromoService:PromoService.apply -> Money:Money.cap",
            "PromoRepo:PromoRepo.find -> Promo:Promo",
        ],
    },
    Case {
        lang: "Kotlin",
        files: &[
            (
                "app/src/main/kotlin/id/acme/cart/Cart.kt",
                "package id.acme.cart\n\nclass Cart(private val items: MutableList<Long>) {\n    fun total(): Long {\n        val sum = subtotal()\n        return roundRupiah(sum - discount(sum))\n    }\n    private fun subtotal(): Long = items.sum()\n    private fun discount(x: Long): Long = Promo.rate() * x / 100\n}\n",
            ),
            (
                "app/src/main/kotlin/id/acme/cart/Money.kt",
                "package id.acme.cart\n\nfun roundRupiah(x: Long): Long = x\n",
            ),
            (
                "app/src/main/kotlin/id/acme/cart/Promo.kt",
                "package id.acme.cart\n\nobject Promo {\n    fun rate(): Long = base() + 1\n    private fun base(): Long = 9\n}\n",
            ),
        ],
        expected: &[
            "Cart:Cart.total -> Cart:Cart.subtotal",
            "Cart:Cart.total -> Cart:Cart.discount",
            "Cart:Cart.total -> Money:roundRupiah",
            "Cart:Cart.discount -> Promo:Promo.rate",
            "Promo:Promo.rate -> Promo:Promo.base",
        ],
    },
    Case {
        lang: "PHP",
        files: &[
            (
                "app/Http/Controllers/PromoController.php",
                r#"<?php
namespace App\Http\Controllers;

use App\Services\PromoService;
use Illuminate\Support\Facades\Log;

class PromoController extends Controller
{
    public function show(string $code)
    {
        $promo = PromoService::find($code);
        $this->audit($code);
        Log::info($code);
        return format_promo($promo);
    }

    private function audit(string $code) { return strtoupper($code); }
}

function format_promo($p) { return json_encode($p); }
"#,
            ),
            (
                "app/Services/PromoService.php",
                "<?php\nnamespace App\\Services;\n\nclass PromoService\n{\n    public static function find(string $code) { return self::load($code); }\n    private static function load(string $code) { return ['code' => $code]; }\n}\n",
            ),
            (
                "app/Http/Controllers/Controller.php",
                "<?php\nnamespace App\\Http\\Controllers;\n\nabstract class Controller {}\n",
            ),
        ],
        expected: &[
            "PromoController:PromoController.show -> PromoService:PromoService.find",
            "PromoController:PromoController.show -> PromoController:PromoController.audit",
            "PromoController:PromoController.show -> PromoController:format_promo",
            "PromoService:PromoService.find -> PromoService:PromoService.load",
        ],
    },
    Case {
        lang: "C#",
        files: &[
            (
                "Kasir.Api/Services/PromoService.cs",
                r#"using Kasir.Api.Money;
using System.Linq;

namespace Kasir.Api.Services
{
    public class PromoService
    {
        private readonly PromoRepo _repo = new PromoRepo();

        public long Apply(long[] items, string code)
        {
            var p = _repo.Find(code);
            if (!IsActive(p)) return 0;
            var total = items.Sum();
            return Rupiah.Round(Rupiah.Cap(total));
        }

        private bool IsActive(object p) => p != null;
    }
}
"#,
            ),
            (
                "Kasir.Api/Services/PromoRepo.cs",
                "namespace Kasir.Api.Services\n{\n    public class PromoRepo\n    {\n        public object Find(string code) => Load(code);\n        private object Load(string code) => code;\n    }\n}\n",
            ),
            (
                "Kasir.Api/Money/Rupiah.cs",
                "namespace Kasir.Api.Money\n{\n    public static class Rupiah\n    {\n        public static long Round(long x) => x;\n        public static long Cap(long x) => System.Math.Min(x, 50000);\n    }\n}\n",
            ),
        ],
        expected: &[
            "PromoService:PromoService -> PromoRepo:PromoRepo",
            "PromoService:PromoService.Apply -> PromoRepo:PromoRepo.Find",
            "PromoService:PromoService.Apply -> PromoService:PromoService.IsActive",
            "PromoService:PromoService.Apply -> Rupiah:Rupiah.Round",
            "PromoService:PromoService.Apply -> Rupiah:Rupiah.Cap",
            "PromoRepo:PromoRepo.Find -> PromoRepo:PromoRepo.Load",
        ],
    },
    Case {
        lang: "Dart",
        files: &[
            (
                "lib/cart/cart.dart",
                "import 'package:kasir/money/rupiah.dart';\nimport 'promo.dart';\n\nclass Cart {\n  final List<int> items;\n  Cart(this.items);\n\n  int total() {\n    final s = subtotal();\n    return roundRupiah(s - Promo.discount(s));\n  }\n\n  int subtotal() => items.fold(0, (a, b) => a + b);\n}\n",
            ),
            (
                "lib/cart/promo.dart",
                "class Promo {\n  static int discount(int x) => _rate() * x ~/ 100;\n  static int _rate() => 10;\n}\n",
            ),
            ("lib/money/rupiah.dart", "int roundRupiah(int x) => x;\n"),
        ],
        expected: &[
            "cart:Cart.total -> cart:Cart.subtotal",
            "cart:Cart.total -> rupiah:roundRupiah",
            "cart:Cart.total -> promo:Promo.discount",
            "promo:Promo.discount -> promo:Promo._rate",
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_meets_threshold() {
        let r = run();
        for l in &r.langs {
            println!(
                "{:<11} recall {:>5.1}% presisi {:>5.1}% ({} / {}) missed {:?} wrong {:?}",
                l.lang,
                l.recall(),
                l.precision(),
                l.correct,
                l.expected,
                l.missed,
                l.wrong
            );
        }
        println!("TOTAL recall {:.1}% presisi {:.1}%", r.recall, r.precision);
        assert!(r.recall >= 95.0, "recall {:.1}%", r.recall);
        assert!(r.precision >= 90.0, "presisi {:.1}%", r.precision);
    }
}
