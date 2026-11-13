#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Ekstraksi per bahasa prioritas: definisi + induk, call + receiver + scope, import, pewarisan.
use agentgate_graph::{FileFacts, extract};

fn facts(path: &str, src: &str) -> FileFacts {
    extract(path, src).expect("extract")
}

/// `nama:kind[@induk]`
fn defs(f: &FileFacts) -> Vec<String> {
    f.symbols
        .iter()
        .map(|s| {
            let parent = s
                .parent
                .map(|p| format!("@{}", f.symbols[p as usize].name))
                .unwrap_or_default();
            let owner = if s.owner.is_empty() || s.parent.is_some() {
                String::new()
            } else {
                format!("^{}", s.owner)
            };
            format!("{}:{}{parent}{owner}", s.name, s.kind.as_str())
        })
        .collect()
}

/// `scope>receiver.nama`
fn calls(f: &FileFacts) -> Vec<String> {
    let mut v: Vec<String> = f
        .calls
        .iter()
        .map(|c| {
            let scope = c
                .scope
                .map_or("-".to_string(), |s| f.symbols[s as usize].name.clone());
            if c.receiver.is_empty() {
                format!("{scope}>{}", c.name)
            } else {
                format!("{scope}>{}.{}", c.receiver, c.name)
            }
        })
        .collect();
    v.sort();
    v
}

fn inherits(f: &FileFacts) -> Vec<String> {
    let mut v: Vec<String> = f
        .inherits
        .iter()
        .map(|i| format!("{}<{}", i.owner, i.base))
        .collect();
    v.sort();
    v
}

fn imports(f: &FileFacts) -> Vec<String> {
    f.imports
        .iter()
        .map(|i| {
            format!(
                "{}[{}]{}",
                i.source,
                i.names.join(","),
                if i.alias.is_empty() {
                    String::new()
                } else {
                    format!(" as {}", i.alias)
                }
            )
        })
        .collect()
}

#[test]
fn typescript() {
    let f = facts(
        "src/cart/promo.ts",
        r#"import { roundRupiah, capDiscount as cap } from "./money";
import axios from "axios";
import * as api from "../api/promoApi";

/** Hitung promo keranjang. */
export function calcPromo(items: Item[]): number {
  const total = items.map((it) => applyItem(it)).length;
  return roundRupiah(cap(total)) + api.validate(total);
}

const applyItem = (it: Item) => it.price;

export class PromoService extends BaseService implements Service {
  private cache = new Map();
  run(): void { this.load(); calcPromo([]); }
  private load() { return axios.get(`/v1/promo/${id}`); }
}

export interface Service extends Base { run(): void }
"#,
    );
    assert_eq!(
        defs(&f),
        vec![
            "calcPromo:function",
            "applyItem:function",
            "PromoService:class",
            "run:method@PromoService",
            "load:method@PromoService",
            "Service:interface",
            "run:method@Service"
        ]
    );
    assert!(f.symbols[0].exported);
    assert_eq!(f.symbols[0].doc, "Hitung promo keranjang.");
    assert!(!f.symbols[1].exported);
    assert!(!f.symbols[4].exported, "private method");
    assert_eq!(
        calls(&f),
        vec![
            "PromoService>Map",
            "calcPromo>api.validate",
            "calcPromo>applyItem",
            "calcPromo>cap",
            "calcPromo>items.map",
            "calcPromo>roundRupiah",
            "load>axios.get",
            "run>calcPromo",
            "run>this.load",
        ]
    );
    assert_eq!(
        imports(&f),
        vec![
            "./money[roundRupiah,capDiscount]",
            "axios[axios]",
            "../api/promoApi[] as api"
        ]
    );
    assert_eq!(
        inherits(&f),
        vec![
            "PromoService<BaseService",
            "PromoService<Service",
            "Service<Base"
        ]
    );
    assert_eq!(f.http.len(), 1);
    assert_eq!(
        (
            f.http[0].method.as_str(),
            f.http[0].path.as_str(),
            f.http[0].exact
        ),
        ("GET", "/v1/promo/:p", false)
    );
}

#[test]
fn javascript_express_routes_and_events() {
    let f = facts(
        "server/routes.js",
        r#"const express = require("express");
const { settle } = require("./payment");
const router = express.Router();
router.post("/v1/payments/qris", createQris);
router.get("/v1/stock/:sku", (req, res) => res.json(stock(req.params.sku)));
function createQris(req, res) { settle(req.body); bus.publish("payment.settled", req.body); }
eventBus.on("receipt.print", printReceipt);
"#,
    );
    assert_eq!(defs(&f), vec!["createQris:function"]);
    let routes: Vec<_> = f
        .routes
        .iter()
        .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
        .collect();
    assert_eq!(
        routes,
        vec!["POST /v1/payments/qris createQris", "GET /v1/stock/:p "]
    );
    assert_eq!(f.routes[0].handler, Some(0));
    let ev: Vec<_> = f
        .events
        .iter()
        .map(|e| format!("{}{}", if e.publish { "pub " } else { "sub " }, e.topic))
        .collect();
    assert_eq!(ev, vec!["pub payment.settled", "sub receipt.print"]);
    assert_eq!(
        imports(&f),
        vec!["express[] as express", "./payment[settle]"]
    );
}

#[test]
fn python() {
    let f = facts(
        "app/promo.py",
        r#"from .money import round_rupiah, cap as cap_discount
import requests

class PromoService(BaseService):
    """Layanan promo."""
    def run(self, items):
        return self._calc(items) + round_rupiah(1)

    def _calc(self, items):
        return requests.post("https://api.x.id/v1/promo/validate", json=items)

@app.get("/v1/promo/{code}")
def get_promo(code):
    return PromoService().run([])
"#,
    );
    assert_eq!(
        defs(&f),
        vec![
            "PromoService:class",
            "run:method@PromoService",
            "_calc:method@PromoService",
            "get_promo:function"
        ]
    );
    assert_eq!(f.symbols[0].doc, "Layanan promo.");
    assert!(!f.symbols[2].exported);
    assert_eq!(
        calls(&f),
        vec![
            "->app.get",
            "_calc>requests.post",
            "get_promo>PromoService",
            "get_promo>PromoService().run",
            "run>round_rupiah",
            "run>self._calc"
        ]
    );
    assert_eq!(
        imports(&f),
        vec![".money[round_rupiah,cap] as cap_discount", "requests[]"]
    );
    assert_eq!(inherits(&f), vec!["PromoService<BaseService"]);
    assert_eq!(
        f.routes
            .iter()
            .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
            .collect::<Vec<_>>(),
        vec!["GET /v1/promo/:p get_promo"]
    );
    assert_eq!(
        f.http
            .iter()
            .map(|h| format!("{} {} {}", h.method, h.path, h.exact))
            .collect::<Vec<_>>(),
        vec!["POST /v1/promo/validate true"]
    );
}

#[test]
fn go() {
    let f = facts(
        "internal/promo/handler.go",
        r#"package promo

import (
	"net/http"
	m "github.com/acme/api/internal/money"
)

type Handler struct { Base }

// Validate memvalidasi kode promo.
func (h *Handler) Validate(w http.ResponseWriter, r *http.Request) {
	total := calc(r)
	m.Round(total)
	h.log()
}

func (h Handler) log() {}

func calc(r *http.Request) int { return 1 }

func Routes(mux *http.ServeMux, h *Handler) {
	mux.HandleFunc("/v1/promo/validate", h.Validate)
}
"#,
    );
    assert_eq!(
        defs(&f),
        vec![
            "Handler:struct",
            "Validate:method^Handler",
            "log:method^Handler",
            "calc:function",
            "Routes:function"
        ]
    );
    assert!(f.symbols[1].exported && !f.symbols[2].exported);
    assert_eq!(f.symbols[1].doc, "Validate memvalidasi kode promo.");
    assert_eq!(
        calls(&f),
        vec![
            "Routes>mux.HandleFunc",
            "Validate>calc",
            "Validate>h.log",
            "Validate>m.Round"
        ]
    );
    assert_eq!(
        imports(&f),
        vec!["net/http[]", "github.com/acme/api/internal/money[] as m"]
    );
    assert_eq!(inherits(&f), vec!["Handler<Base"]);
    assert_eq!(
        f.routes
            .iter()
            .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
            .collect::<Vec<_>>(),
        vec!["ANY /v1/promo/validate Validate"]
    );
}

#[test]
fn rust() {
    let f = facts(
        "src/promo.rs",
        r#"use crate::money::{round, cap};
use std::fmt;

/// Layanan promo.
pub struct Promo { total: i64 }

impl Promo {
    pub fn new() -> Self { Self { total: helper() } }
    fn apply(&self) -> i64 { round(self.total) + money::cap(1) + Promo::new().total }
}

impl fmt::Display for Promo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "x") }
}

fn helper() -> i64 { 1 }

#[cfg(test)]
mod tests {
    #[test]
    fn works() { super::helper(); }
}

pub fn routes() -> Router { Router::new().route("/v1/promo", get(list)).route("/v1/promo/{id}", post(save)) }
"#,
    );
    assert_eq!(
        defs(&f),
        vec![
            "Promo:struct",
            "new:method^Promo",
            "apply:method^Promo",
            "fmt:method^Promo",
            "helper:function",
            "tests:module",
            "works:function@tests",
            "routes:function"
        ]
    );
    assert_eq!(f.symbols[0].doc, "Layanan promo.");
    assert!(
        f.symbols[1].exported && !f.symbols[2].exported && f.symbols[3].exported,
        "pub / privat / impl trait"
    );
    assert!(f.symbols[6].test);
    assert_eq!(
        calls(&f),
        vec![
            "apply>Promo.new",
            "apply>money.cap",
            "apply>round",
            "new>Self",
            "new>helper",
            "routes>Router.new",
            "routes>Router::new().route",
            "routes>Router::new().route(\"/v1/promo\", get(list)).route",
            "routes>get",
            "routes>post",
            "works>super.helper"
        ]
    );
    assert_eq!(imports(&f), vec!["crate::money[round,cap]", "std[fmt]"]);
    assert_eq!(inherits(&f), vec!["Promo<Display"]);
    assert_eq!(
        f.routes
            .iter()
            .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
            .collect::<Vec<_>>(),
        vec!["GET /v1/promo list", "POST /v1/promo/:p save"]
    );
}

#[test]
fn java_spring() {
    let f = facts(
        "src/main/java/id/acme/promo/PromoController.java",
        r#"package id.acme.promo;
import id.acme.money.Money;
public class PromoController extends BaseController implements Api {
  @GetMapping("/v1/promo/{code}")
  public Promo get(String code) { return service.find(code).orElse(build()); }
  private Promo build() { return new Promo(Money.zero()); }
}
"#,
    );
    assert_eq!(
        defs(&f),
        vec![
            "PromoController:class",
            "get:method@PromoController",
            "build:method@PromoController"
        ]
    );
    assert!(f.symbols[1].exported && !f.symbols[2].exported);
    assert_eq!(
        calls(&f),
        vec![
            "build>Money.zero",
            "build>Promo",
            "get>build",
            "get>service.find",
            "get>service.find(code).orElse"
        ]
    );
    assert_eq!(imports(&f), vec!["id.acme.money.Money[]"]);
    assert_eq!(
        inherits(&f),
        vec!["PromoController<Api", "PromoController<BaseController"]
    );
    assert_eq!(
        f.routes
            .iter()
            .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
            .collect::<Vec<_>>(),
        vec!["GET /v1/promo/:p get"]
    );
}

#[test]
fn kotlin() {
    let f = facts(
        "app/src/Cart.kt",
        "class Cart(val items: List<Int>) : Base(), Shape {\n  fun total(n: Int): Int { return helper(n) + this.area() + Foo.make() }\n  private fun area(): Int = 1\n}\nfun helper(n: Int) = n\n",
    );
    assert_eq!(
        defs(&f),
        vec![
            "Cart:class",
            "total:method@Cart",
            "area:method@Cart",
            "helper:function"
        ]
    );
    assert!(!f.symbols[2].exported);
    assert_eq!(
        calls(&f),
        vec!["total>Foo.make", "total>helper", "total>this.area"]
    );
    assert_eq!(inherits(&f), vec!["Cart<Base", "Cart<Shape"]);
}

#[test]
fn php_laravel() {
    let f = facts(
        "app/Http/PromoController.php",
        r#"<?php
namespace App\Http;
use App\Models\User;
class PromoController extends Controller implements Api {
  public function show() { $this->load(); User::find(1); helper(); return new Promo(); }
  private function load() {}
}
function helper() {}
Route::get('/v1/promo', [PromoController::class, 'show']);
"#,
    );
    assert_eq!(
        defs(&f),
        vec![
            "PromoController:class",
            "show:method@PromoController",
            "load:method@PromoController",
            "helper:function"
        ]
    );
    assert!(!f.symbols[2].exported);
    assert_eq!(
        calls(&f),
        vec![
            "->Route.get",
            "show>$this.load",
            "show>Promo",
            "show>User.find",
            "show>helper"
        ]
    );
    assert_eq!(imports(&f), vec!["App\\Models\\User[]"]);
    assert_eq!(
        inherits(&f),
        vec!["PromoController<Api", "PromoController<Controller"]
    );
    assert_eq!(
        f.routes
            .iter()
            .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
            .collect::<Vec<_>>(),
        vec!["GET /v1/promo show"]
    );
}

#[test]
fn csharp() {
    let f = facts(
        "Api/PromoController.cs",
        "using App.Models;\nnamespace App { public class PromoController : Base, IApi {\n [HttpGet(\"/v1/promo\")]\n public int Run() { return Go() + this.Go() + Helper.Calc(1) + new D().X(); }\n private int Go() => 1; } }\n",
    );
    assert_eq!(
        defs(&f),
        vec![
            "PromoController:class",
            "Run:method@PromoController",
            "Go:method@PromoController"
        ]
    );
    assert!(f.symbols[1].exported && !f.symbols[2].exported);
    assert_eq!(
        calls(&f),
        vec![
            "Run>D",
            "Run>Go",
            "Run>Helper.Calc",
            "Run>new D().X",
            "Run>this.Go"
        ]
    );
    assert_eq!(
        inherits(&f),
        vec!["PromoController<Base", "PromoController<IApi"]
    );
    assert_eq!(
        f.routes
            .iter()
            .map(|r| format!("{} {} {}", r.method, r.path, r.handler_name))
            .collect::<Vec<_>>(),
        vec!["GET /v1/promo Run"]
    );
}

#[test]
fn dart() {
    let f = facts(
        "lib/cart.dart",
        "import 'package:kasir/money.dart';\nimport 'api.dart' as api;\nclass Cart extends Base implements Shape {\n  double area() { return total(); }\n  int total() => helper(1) + this.area().toInt() + api.make();\n}\nint helper(int n) => n;\nint _hidden() => 1;\n",
    );
    assert_eq!(
        defs(&f),
        vec![
            "Cart:class",
            "area:method@Cart",
            "total:method@Cart",
            "helper:function",
            "_hidden:function"
        ]
    );
    assert!(!f.symbols[4].exported);
    assert_eq!(
        calls(&f),
        vec![
            "area>total",
            "total>api.make",
            "total>helper",
            "total>this.area",
            "total>this.area().toInt"
        ]
    );
    assert_eq!(
        imports(&f),
        vec!["package:kasir/money.dart[]", "api.dart[] as api"]
    );
    assert_eq!(inherits(&f), vec!["Cart<Base", "Cart<Shape"]);
}

#[test]
fn todos_errors_and_manifest() {
    let f = facts("a.ts", "// TODO: hapus\nfunction a( {\n");
    assert_eq!(f.todos, 1);
    assert!(f.errors > 0);
    let m = facts("package.json", r#"{"name":"@acme/printer"}"#);
    assert_eq!(m.package.map(|p| p.name), Some("@acme/printer".into()));
}
