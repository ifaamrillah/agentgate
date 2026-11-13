//! Parsing satu file → [`FileFacts`]. Definisi, call, import, dan pewarisan memakai query per
//! bahasa (`queries/*.scm`); route, HTTP call, dan event memakai pola teks pada node call dan
//! anotasi, karena bentuknya bergantung framework.
use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use regex::Regex;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

use crate::{
    facts::{
        CallFact, EventFact, FileFacts, HttpFact, ImportFact, InheritFact, RouteFact, SymbolFact,
        SymbolKind,
    },
    lang::Lang,
    manifest,
};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("bahasa tidak didukung: {0}")]
    Unsupported(String),
    #[error("parser gagal")]
    Parser,
}

fn queries() -> &'static Vec<(Lang, Query)> {
    static Q: OnceLock<Vec<(Lang, Query)>> = OnceLock::new();
    Q.get_or_init(|| {
        Lang::ALL
            .into_iter()
            .filter_map(|l| {
                let g = l.grammar()?;
                Query::new(&g, l.query()).ok().map(|q| (l, q))
            })
            .collect()
    })
}

/// Parse file `path` (bahasa dari ekstensi).
///
/// # Errors
///
/// Bahasa tidak dikenal, atau parser tree-sitter gagal total.
pub fn extract(path: &str, source: &str) -> Result<FileFacts, ExtractError> {
    let lang = Lang::detect(path).ok_or_else(|| ExtractError::Unsupported(path.to_string()))?;
    let lines = u32::try_from(source.lines().count()).unwrap_or(u32::MAX);
    if lang == Lang::Manifest {
        let mut f = FileFacts::empty(lang, lines);
        f.package = manifest::parse(path, source);
        return Ok(f);
    }
    let grammar = lang.grammar().ok_or(ExtractError::Parser)?;
    let mut parser = Parser::new();
    parser
        .set_language(&grammar)
        .map_err(|_| ExtractError::Parser)?;
    let tree = parser.parse(source, None).ok_or(ExtractError::Parser)?;
    let query = queries()
        .iter()
        .find(|(l, _)| *l == lang)
        .map(|(_, q)| q)
        .ok_or(ExtractError::Parser)?;
    let mut x = Extractor {
        lang,
        src: source,
        path,
        defs: Vec::new(),
        facts: FileFacts::empty(lang, lines),
    };
    x.run(query, tree.root_node());
    Ok(x.facts)
}

struct Def<'t> {
    node: Node<'t>,
    name: String,
    kind: SymbolKind,
    owner: String,
}

struct Extractor<'s, 't> {
    lang: Lang,
    src: &'s str,
    path: &'s str,
    defs: Vec<Def<'t>>,
    facts: FileFacts,
}

fn line(n: Node<'_>) -> u32 {
    n.start_position().row as u32 + 1
}

fn def_kind(capture: &str) -> Option<SymbolKind> {
    Some(match capture {
        "def.function" => SymbolKind::Function,
        "def.method" => SymbolKind::Method,
        "def.class" => SymbolKind::Class,
        "def.interface" => SymbolKind::Interface,
        "def.struct" => SymbolKind::Struct,
        "def.enum" => SymbolKind::Enum,
        "def.trait" => SymbolKind::Trait,
        "def.type" => SymbolKind::Type,
        "def.module" => SymbolKind::Module,
        _ => return None,
    })
}

fn unquote(s: &str) -> String {
    s.trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .to_string()
}

fn short(s: &str, max: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > max {
        let mut t: String = one.chars().take(max).collect();
        t.push('…');
        t
    } else {
        one
    }
}

impl<'s, 't> Extractor<'s, 't> {
    fn text(&self, n: Node<'_>) -> &'s str {
        n.utf8_text(self.src.as_bytes()).unwrap_or("")
    }

    fn run(&mut self, query: &Query, root: Node<'t>) {
        let names = query.capture_names();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, root, self.src.as_bytes());
        let mut calls: Vec<(Node<'t>, String, String)> = Vec::new();
        let mut imports: Vec<(usize, ImportFact)> = Vec::new();
        let mut inherits: Vec<(Node<'t>, String, String)> = Vec::new();
        let mut seen_calls: HashMap<(usize, String), ()> = HashMap::new();
        while let Some(m) = matches.next() {
            let mut def: Option<(Node<'t>, SymbolKind)> = None;
            let mut name = None;
            let mut owner = String::new();
            let mut call = None;
            let mut callee = None;
            let mut receiver = String::new();
            let mut import = None;
            let mut source = None;
            let mut imported = None;
            let mut alias = String::new();
            let mut inherit = None;
            let mut base = None;
            for c in m.captures() {
                let cap = names[c.index as usize];
                let n = c.node;
                match cap {
                    "name" => name = Some(self.text(n).to_string()),
                    "owner" => owner = self.text(n).to_string(),
                    "call" => call = Some(n),
                    "callee" => callee = Some(self.text(n).to_string()),
                    "receiver" => receiver = self.text(n).to_string(),
                    "import" => import = Some(n),
                    "source" => source = Some(self.text(n).to_string()),
                    "imported" => imported = Some(self.text(n).to_string()),
                    "alias" => alias = self.text(n).to_string(),
                    "inherit" => inherit = Some(n),
                    "base" => base = Some(self.text(n).to_string()),
                    other => {
                        if let Some(k) = def_kind(other) {
                            def = Some((n, k));
                        }
                    }
                }
            }
            if let (Some((node, kind)), Some(name)) = (def, name) {
                self.add_def(node, name, kind, owner.clone());
            }
            if let (Some(node), Some(callee)) = (call, callee)
                && seen_calls.insert((node.id(), callee.clone()), ()).is_none()
            {
                calls.push((node, callee, receiver.clone()));
            }
            if let (Some(node), Some(source)) = (import, source) {
                let names: Vec<String> = imported
                    .map(|i| i.rsplit(['.', ':']).next().unwrap_or(&i).to_string())
                    .into_iter()
                    .collect();
                if let Some((_, f)) = imports.iter_mut().find(|(id, _)| *id == node.id()) {
                    f.names.extend(names);
                    if f.alias.is_empty() {
                        f.alias.clone_from(&alias);
                    }
                } else {
                    imports.push((
                        node.id(),
                        ImportFact {
                            source: unquote(&source),
                            names,
                            alias: alias.clone(),
                            line: line(node),
                        },
                    ));
                }
            }
            if let (Some(node), Some(base)) = (inherit, base) {
                inherits.push((node, owner, base));
            }
        }
        self.finish_defs();
        for (node, name, receiver) in calls {
            let scope = self.scope_of(node);
            self.facts.calls.push(CallFact {
                name,
                receiver: short(&receiver, 60),
                line: line(node),
                scope,
            });
        }
        self.facts.imports = imports.into_iter().map(|(_, f)| f).collect();
        for (node, owner, base) in inherits {
            let symbol = self
                .defs
                .iter()
                .position(|d| d.node.id() == node.id())
                .and_then(|i| u32::try_from(i).ok());
            let owner = if owner.is_empty() {
                symbol
                    .map(|i| self.facts.symbols[i as usize].name.clone())
                    .unwrap_or_default()
            } else {
                owner
            };
            if owner.is_empty() || owner == base {
                continue;
            }
            self.facts.inherits.push(InheritFact {
                owner,
                symbol,
                base,
                line: line(node),
            });
        }
        self.scan(root);
    }

    fn add_def(&mut self, node: Node<'t>, name: String, kind: SymbolKind, owner: String) {
        if name.is_empty() {
            return;
        }
        if let Some(d) = self.defs.iter_mut().find(|d| d.node.id() == node.id()) {
            // Pola khusus (method dalam impl/kelas) menang atas pola umum (fungsi).
            if kind == SymbolKind::Method && d.kind == SymbolKind::Function {
                d.kind = kind;
                if !owner.is_empty() {
                    d.owner = owner;
                }
            }
            return;
        }
        self.defs.push(Def {
            node,
            name,
            kind,
            owner,
        });
    }

    /// Urutkan definisi, tentukan induk (definisi terdalam yang membungkus), lalu isi simbol.
    fn finish_defs(&mut self) {
        self.defs
            .sort_by_key(|d| (d.node.start_byte(), std::cmp::Reverse(d.node.end_byte())));
        let n = self.defs.len();
        let parents: Vec<Option<usize>> = (0..n)
            .map(|i| {
                let (s, e) = (self.defs[i].node.start_byte(), self.defs[i].node.end_byte());
                (0..i).rev().find(|&j| {
                    let d = &self.defs[j];
                    d.node.start_byte() <= s
                        && e <= d.node.end_byte()
                        && d.node.id() != self.defs[i].node.id()
                })
            })
            .collect();
        for (i, parent) in parents.into_iter().enumerate() {
            let mut kind = self.defs[i].kind;
            if kind == SymbolKind::Function && parent.is_some_and(|p| self.defs[p].kind.is_type()) {
                kind = SymbolKind::Method;
            }
            let node = self.defs[i].node;
            let name = self.defs[i].name.clone();
            let mut owner = self.defs[i].owner.clone();
            if owner.is_empty()
                && let Some(p) = parent.filter(|&p| self.defs[p].kind.is_type())
            {
                owner.clone_from(&self.defs[p].name);
            }
            let exported = self.exported(node, &name, parent);
            let test = self.is_test(node, &name);
            let complexity = if kind.is_callable() {
                complexity(node)
            } else {
                0
            };
            let sym = SymbolFact {
                name,
                kind,
                start: line(node),
                end: node.end_position().row as u32 + 1,
                parent: parent.and_then(|p| u32::try_from(p).ok()),
                owner,
                exported,
                test,
                doc: self.doc(node),
                signature: self.signature(node),
                complexity,
            };
            self.facts.symbols.push(sym);
        }
    }

    /// Definisi terdalam yang membungkus `node`.
    fn scope_of(&self, node: Node<'_>) -> Option<u32> {
        let (s, e) = (node.start_byte(), node.end_byte());
        self.defs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.node.start_byte() <= s && e <= d.node.end_byte())
            .min_by_key(|(_, d)| d.node.end_byte() - d.node.start_byte())
            .and_then(|(i, _)| u32::try_from(i).ok())
    }

    /// Teks kepala definisi (sebelum body), untuk modifier dan signature.
    fn header(&self, node: Node<'_>) -> &'s str {
        let end = node
            .child_by_field_name("body")
            .map_or_else(|| node.end_byte(), |b| b.start_byte());
        let end = end.min(node.start_byte() + 400);
        let mut end = end;
        while !self.src.is_char_boundary(end) {
            end -= 1;
        }
        self.src.get(node.start_byte()..end).unwrap_or("")
    }

    fn exported(&self, node: Node<'_>, name: &str, parent: Option<usize>) -> bool {
        let head = self.header(node);
        let has = |kw: &str| {
            head.split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|w| w == kw)
        };
        let parent_exported =
            parent.is_none_or(|p| self.facts.symbols.get(p).is_none_or(|s| s.exported));
        match self.lang {
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript => {
                if parent.is_some() {
                    return parent_exported && !has("private") && !name.starts_with('#');
                }
                let mut cur = node.parent();
                for _ in 0..3 {
                    let Some(p) = cur else { break };
                    if p.kind() == "export_statement" {
                        return true;
                    }
                    cur = p.parent();
                }
                parent.is_some() && parent_exported && !has("private") && !name.starts_with('#')
            }
            Lang::Python => {
                !name.starts_with('_') || (name.starts_with("__") && name.ends_with("__"))
            }
            Lang::Go => name.chars().next().is_some_and(char::is_uppercase),
            Lang::Rust => {
                let mut c = node.walk();
                let is_pub = node
                    .children(&mut c)
                    .any(|ch| ch.kind() == "visibility_modifier");
                // Method impl trait dipanggil lewat trait: dianggap publik.
                let trait_impl = node.parent().and_then(|p| p.parent()).is_some_and(|i| {
                    i.kind() == "impl_item" && i.child_by_field_name("trait").is_some()
                });
                let trait_item = node
                    .parent()
                    .and_then(|p| p.parent())
                    .is_some_and(|i| i.kind() == "trait_item");
                is_pub || trait_impl || trait_item
            }
            Lang::Java | Lang::CSharp => {
                (has("public") || has("protected") || has("internal") || has("override"))
                    && parent_exported
            }
            Lang::Kotlin | Lang::Php => !has("private") && parent_exported,
            Lang::Dart => !name.starts_with('_'),
            Lang::Manifest => false,
        }
    }

    fn is_test(&self, node: Node<'_>, name: &str) -> bool {
        if crate::walk::is_test_path(self.path) {
            return true;
        }
        match self.lang {
            Lang::Python | Lang::Go => name.starts_with("test") || name.starts_with("Test"),
            Lang::Rust => {
                let mut prev = node.prev_named_sibling();
                while let Some(p) = prev {
                    if p.kind() != "attribute_item" {
                        break;
                    }
                    let t = self.text(p);
                    if t.contains("test") {
                        return true;
                    }
                    prev = p.prev_named_sibling();
                }
                false
            }
            Lang::Java | Lang::Kotlin | Lang::CSharp => {
                let h = self.header(node);
                h.contains("@Test") || h.contains("[Test") || h.contains("[Fact")
            }
            _ => false,
        }
    }

    /// Komentar tepat di atas definisi (atau docstring Python).
    fn doc(&self, node: Node<'_>) -> String {
        if self.lang == Lang::Python
            && let Some(body) = node.child_by_field_name("body")
            && let Some(first) = body.named_child(0)
            && first.kind() == "expression_statement"
            && let Some(s) = first.named_child(0)
            && s.kind() == "string"
        {
            return short(&clean_comment(self.text(s)), 300);
        }
        // Naik ke pernyataan pembungkus (export, deklarasi variabel, dekorator) sebelum mencari.
        let mut anchor = node;
        while let Some(p) = anchor.parent() {
            if matches!(
                p.kind(),
                "export_statement"
                    | "lexical_declaration"
                    | "variable_declaration"
                    | "decorated_definition"
                    | "variable_declarator"
            ) {
                anchor = p;
            } else {
                break;
            }
        }
        let mut parts = Vec::new();
        let mut row = anchor.start_position().row;
        let mut prev = anchor.prev_sibling();
        while let Some(p) = prev {
            let k = p.kind();
            if k.contains("comment") && p.end_position().row + 1 >= row {
                parts.push(clean_comment(self.text(p)));
                row = p.start_position().row;
                prev = p.prev_sibling();
            } else if (k == "attribute_item" || k == "annotation" || k == "decorator")
                && p.end_position().row + 1 >= row
            {
                row = p.start_position().row;
                prev = p.prev_sibling();
            } else {
                break;
            }
        }
        parts.reverse();
        short(&parts.join(" "), 300)
    }

    fn signature(&self, node: Node<'_>) -> String {
        short(self.header(node).trim().trim_end_matches('{'), 160)
    }

    /// Pindai seluruh pohon: route, HTTP call, event, TODO, ERROR.
    fn scan(&mut self, root: Node<'t>) {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            let k = n.kind();
            if n.is_error() {
                self.facts.errors += 1;
            }
            if k.contains("comment") {
                let t = self.text(n);
                if t.contains("TODO") || t.contains("FIXME") {
                    self.facts.todos += 1;
                }
                continue;
            }
            if is_annotation(k) {
                self.annotation(n);
            } else if is_call(k) {
                self.call_text(n);
            }
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                stack.push(ch);
            }
        }
        self.facts
            .routes
            .sort_by(|a, b| (a.line, &a.path).cmp(&(b.line, &b.path)));
        self.facts.http.sort_by_key(|h| h.line);
        self.facts.events.sort_by_key(|e| e.line);
    }

    /// Simbol yang dihias anotasi/dekorator `n`.
    fn decorated(&self, n: Node<'_>) -> Option<u32> {
        // Anotasi Java/C#/Kotlin/PHP berada di dalam node definisi.
        if let Some(s) = self.scope_of(n) {
            let sym = &self.facts.symbols[s as usize];
            if sym.kind.is_callable() && sym.start <= line(n) {
                return Some(s);
            }
        }
        // Dekorator Python dan atribut Rust berada sebelum definisi.
        let mut next = n.next_named_sibling();
        while let Some(x) = next {
            if let Some(i) = self.defs.iter().position(|d| d.node.id() == x.id()) {
                return u32::try_from(i).ok();
            }
            if let Some(i) = self.defs.iter().position(|d| {
                d.node.start_byte() >= x.start_byte() && d.node.end_byte() <= x.end_byte()
            }) {
                return u32::try_from(i).ok();
            }
            if is_annotation(x.kind()) {
                next = x.next_named_sibling();
            } else {
                break;
            }
        }
        n.parent().and_then(|p| {
            if p.kind() == "decorated_definition" {
                p.child_by_field_name("definition").and_then(|d| {
                    self.defs
                        .iter()
                        .position(|x| x.node.id() == d.id())
                        .and_then(|i| u32::try_from(i).ok())
                })
            } else {
                None
            }
        })
    }

    fn annotation(&mut self, n: Node<'_>) {
        let text = short(self.text(n), 300);
        let Some(c) = ANNOTATION_ROUTE.captures(&text) else {
            return;
        };
        let verb = c.get(1).map_or("", |m| m.as_str());
        let path = c.get(2).map_or("", |m| m.as_str());
        let mut method = verb_method(verb);
        if method == "ANY"
            && let Some(m) = METHODS_LIST.captures(&text)
        {
            method = m[1].to_ascii_uppercase();
        }
        let handler = self.decorated(n);
        let handler_name = handler
            .map(|h| self.facts.symbols[h as usize].name.clone())
            .unwrap_or_default();
        self.facts.routes.push(RouteFact {
            method,
            path: normalize_path(path),
            line: line(n),
            handler,
            handler_name,
        });
    }

    /// Teks callee dan argumen sebuah node call.
    fn call_parts(&self, n: Node<'_>) -> Option<(String, String)> {
        let args = n.child_by_field_name("arguments").or_else(|| {
            let mut c = n.walk();
            n.children(&mut c).find(|ch| {
                matches!(
                    ch.kind(),
                    "value_arguments" | "token_tree" | "argument_list"
                )
            })
        })?;
        let callee = self.src.get(n.start_byte()..args.start_byte())?.trim();
        let callee = callee
            .strip_prefix("await ")
            .unwrap_or(callee)
            .trim_end_matches('!');
        let callee = match callee.rfind('<') {
            Some(i) if callee.ends_with('>') => &callee[..i],
            _ => callee,
        };
        Some((callee.to_string(), short(self.text(args), 400)))
    }

    fn call_text(&mut self, n: Node<'_>) {
        let Some((callee, args)) = self.call_parts(n) else {
            return;
        };
        let text = format!("{callee}{args}");
        let Some(c) = ARGS_LITERAL.captures(&args) else {
            return;
        };
        let quote = c.get(1).map_or("", |m| m.as_str());
        let literal = c.get(2).map_or("", |m| m.as_str());
        let rest = c.get(3).map_or("", |m| m.as_str());
        let callee = callee.as_str();
        let (receiver, verb) = match callee.rfind(['.', ':', '>']) {
            Some(i) => (&callee[..i], &callee[i + 1..]),
            None => ("", callee),
        };
        let receiver = receiver.trim_end_matches(['-', ':', '?']);
        let verb_l = verb.to_ascii_lowercase();
        let recv_l = receiver.to_ascii_lowercase();
        let scope = self.scope_of(n);
        let templated = quote == "`" && literal.contains("${") || literal.contains("{{");
        let has_more_args = rest.trim_start().starts_with(',');
        let is_client = verb_l == "fetch"
            || CLIENT_RECEIVERS
                .iter()
                .any(|r| recv_l == *r || recv_l.ends_with(&format!(".{r}")));
        // HTTP client.
        if is_client
            && (HTTP_VERBS.contains(&verb_l.as_str()) || verb_l == "fetch" || verb_l == "request")
        {
            if let Some(path) = url_path(literal) {
                let method = if verb_l == "fetch" || verb_l == "request" {
                    METHOD_PROP
                        .captures(&text)
                        .map_or_else(|| "GET".to_string(), |m| m[1].to_ascii_uppercase())
                } else {
                    verb_method(&verb_l)
                };
                let exact = !templated && !literal.starts_with("${");
                self.facts.http.push(HttpFact {
                    method,
                    path,
                    exact,
                    line: line(n),
                    scope,
                });
            }
            return;
        }
        // Route server: verba HTTP/route + path literal + handler sesudahnya.
        let route_verb = HTTP_VERBS.contains(&verb_l.as_str())
            || matches!(
                verb_l.as_str(),
                "route"
                    | "handle"
                    | "handlefunc"
                    | "any"
                    | "all"
                    | "match"
                    | "mapget"
                    | "mappost"
                    | "mapput"
                    | "mapdelete"
                    | "mappatch"
                    | "path"
                    | "re_path"
            );
        if route_verb && has_more_args && !receiver.is_empty() || route_verb && verb_l == "path" {
            let path_ok = literal.starts_with('/') || verb_l == "path" || verb_l == "re_path";
            if !path_ok {
                return;
            }
            let mut method = verb_method(&verb_l);
            if method == "ANY"
                && let Some(m) = INNER_VERB.captures(rest)
            {
                method = m[1].to_ascii_uppercase();
            }
            if method == "ANY"
                && let Some(m) = METHODS_LIST.captures(rest)
            {
                method = m[1].to_ascii_uppercase();
            }
            let handler_name = HANDLER_ARG
                .captures(rest)
                .and_then(|m| m.get(1).or_else(|| m.get(2)))
                .map(|m| {
                    let h = m.as_str();
                    h.rsplit(['.', ':']).next().unwrap_or(h).to_string()
                })
                .unwrap_or_default();
            let handler = self
                .facts
                .symbols
                .iter()
                .position(|s| s.kind.is_callable() && s.name == handler_name)
                .and_then(|i| u32::try_from(i).ok());
            self.facts.routes.push(RouteFact {
                method,
                path: normalize_path(literal),
                line: line(n),
                handler,
                handler_name,
            });
            return;
        }
        // Event / queue.
        let publish = PUBLISH_VERBS.contains(&verb_l.as_str());
        let subscribe = SUBSCRIBE_VERBS.contains(&verb_l.as_str())
            || verb_l == "on" && EVENT_RECEIVER.is_match(&recv_l);
        if (publish || subscribe) && TOPIC.is_match(literal) && !templated {
            self.facts.events.push(EventFact {
                topic: literal.to_string(),
                publish,
                line: line(n),
                scope,
            });
        }
    }
}

fn is_call(kind: &str) -> bool {
    matches!(
        kind,
        "call_expression"
            | "call"
            | "method_invocation"
            | "invocation_expression"
            | "function_call_expression"
            | "member_call_expression"
            | "scoped_call_expression"
            | "macro_invocation"
    )
}

fn is_annotation(kind: &str) -> bool {
    matches!(
        kind,
        "decorator" | "annotation" | "marker_annotation" | "attribute_item" | "attribute"
    )
}

fn verb_method(verb: &str) -> String {
    let v = verb.to_ascii_lowercase();
    let v = v
        .trim_start_matches("map")
        .trim_start_matches("http")
        .trim_end_matches("mapping");
    match v {
        "get" | "post" | "put" | "patch" | "delete" | "head" | "options" => v.to_ascii_uppercase(),
        _ => "ANY".to_string(),
    }
}

/// Path dari URL literal: skema+host dibuang, query dibuang, parameter → `:p`.
#[must_use]
pub fn url_path(literal: &str) -> Option<String> {
    let mut s = literal.trim();
    // `${BASE}/v1/x` atau `${this.base}/x`: buang prefix template.
    while let Some(rest) = s.strip_prefix("${") {
        s = rest.split_once('}').map_or("", |(_, r)| r);
    }
    if let Some(i) = s.find("://") {
        let after = &s[i + 3..];
        s = after.find('/').map_or("/", |j| &after[j..]);
    }
    if !s.starts_with('/') {
        return None;
    }
    Some(normalize_path(s))
}

/// Normalisasi path route/URL: `:id`, `{id}`, `<id>`, `${id}`, `{id:int}` → `:p`; tanpa `/` akhir.
#[must_use]
pub fn normalize_path(path: &str) -> String {
    let p = path.split(['?', '#']).next().unwrap_or("");
    let mut out = String::new();
    for seg in p.split('/') {
        if seg.is_empty() {
            continue;
        }
        out.push('/');
        let param = seg.starts_with(':')
            || (seg.starts_with('{') && seg.ends_with('}'))
            || (seg.starts_with('<') && seg.ends_with('>'))
            || seg.starts_with("${")
            || seg == "*";
        if param {
            out.push_str(":p");
        } else {
            out.push_str(seg);
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

fn clean_comment(t: &str) -> String {
    t.lines()
        .map(|l| {
            l.trim()
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim_start_matches("//")
                .trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim_start_matches('#')
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Kompleksitas siklomatik kasar: 1 + jumlah cabang di dalam fungsi.
fn complexity(node: Node<'_>) -> u16 {
    let mut n: u16 = 1;
    let mut stack = vec![node];
    while let Some(x) = stack.pop() {
        let k = x.kind();
        let branch = matches!(
            k,
            "if_statement"
                | "if_expression"
                | "elif_clause"
                | "else_if_clause"
                | "for_statement"
                | "for_in_statement"
                | "for_expression"
                | "enhanced_for_statement"
                | "foreach_statement"
                | "while_statement"
                | "while_expression"
                | "do_statement"
                | "case_clause"
                | "switch_case"
                | "switch_section"
                | "expression_case"
                | "match_arm"
                | "when_entry"
                | "catch_clause"
                | "except_clause"
                | "conditional_expression"
                | "ternary_expression"
        ) || (x.child_count() == 0 && matches!(k, "&&" | "||" | "and" | "or"));
        if branch {
            n = n.saturating_add(1);
        }
        let mut c = x.walk();
        for ch in x.children(&mut c) {
            stack.push(ch);
        }
    }
    n
}

const HTTP_VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];
const CLIENT_RECEIVERS: &[&str] = &[
    "axios",
    "http",
    "https",
    "requests",
    "httpx",
    "client",
    "api",
    "$http",
    "httpclient",
    "this.http",
    "resttemplate",
    "webclient",
    "ky",
    "superagent",
    "got",
    "session",
    "dio",
    "apiclient",
    "request",
    "fetcher",
];
const PUBLISH_VERBS: &[&str] = &[
    "publish",
    "emit",
    "produce",
    "enqueue",
    "broadcast",
    "send_event",
    "dispatch_event",
];
const SUBSCRIBE_VERBS: &[&str] = &[
    "subscribe",
    "consume",
    "listen",
    "add_listener",
    "addlistener",
    "handle_event",
];

static ARGS_LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"^\(\s*(["'`])([^"'`]*)["'`](.*)$"#).unwrap()
});
static ANNOTATION_ROUTE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?i)^[@\[#!\s]*\[?(?:[\w.]+\.)?(get|post|put|patch|delete|route|api_view|request|(?:get|post|put|patch|delete|request)mapping|http(?:get|post|put|patch|delete))\s*\(\s*(?:value\s*=\s*|path\s*=\s*)?["']([^"']*)["']"#).unwrap()
});
static METHODS_LIST: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?i)(?:methods\s*=\s*\[\s*|RequestMethod\.)["']?(get|post|put|patch|delete)"#)
        .unwrap()
});
static INNER_VERB: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"(?i)^\s*,\s*(?:[\w:]+::)?(get|post|put|patch|delete)\s*\(").unwrap()
});
static HANDLER_ARG: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^\s*,\s*(?:(?:[\w:]+::)?(?:get|post|put|patch|delete)\s*\(\s*)?(?:\[\s*[\w\\]+::class\s*,\s*'(\w+)'|([A-Za-z_$][\w$.:]*))").unwrap()
});
static METHOD_PROP: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r#"(?i)method\s*:\s*["'](get|post|put|patch|delete)["']"#).unwrap()
});
static TOPIC: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"^[A-Za-z][\w.:\-]{2,80}$").unwrap()
});
static EVENT_RECEIVER: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::unwrap_used)]
    Regex::new(r"(bus|emitter|events?|queue|socket|channel|consumer|subscriber|nats|kafka|redis|pubsub|broker)").unwrap()
});
