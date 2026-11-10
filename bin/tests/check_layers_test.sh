#!/usr/bin/env bash
# Tes bin/check-layers pada fixture sementara.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/src/models" "$TMP/src/views" "$TMP/src/controllers"

echo 'use crate::common::x;' >"$TMP/src/models/a.rs"
"$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null || fail "kode bersih dianggap melanggar"

echo '    // lihat crate::controllers::x untuk contoh' >"$TMP/src/models/b.rs"
"$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null || fail "komentar dianggap melanggar"

echo 'use crate::controllers::x;' >"$TMP/src/models/c.rs"
if "$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null; then fail "models -> controllers lolos"; fi
rm "$TMP/src/models/c.rs"

echo 'pub use super::super::controllers::render;' >"$TMP/src/views/d.rs"
if "$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null; then fail "super::super::controllers di views lolos"; fi
rm "$TMP/src/views/d.rs"

echo 'fn f() { crate::services::auth::x(); }' >"$TMP/src/views/e.rs"
if "$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null; then fail "path di dalam ekspresi lolos"; fi
rm "$TMP/src/views/e.rs"

echo 'use crate::{common::x, controllers::home};' >"$TMP/src/models/f.rs"
if "$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null; then fail "import bergrup lolos"; fi
rm "$TMP/src/models/f.rs"

printf 'use crate::{\n    common::x,\n    views,\n};\n' >"$TMP/src/models/g.rs"
if "$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null; then fail "import bergrup multi-baris lolos"; fi
rm "$TMP/src/models/g.rs"

printf 'use crate::{\n    common::{views_helper, x},\n};\n' >"$TMP/src/models/h.rs"
"$ROOT/bin/check-layers" "$TMP/src" 2>/dev/null || fail "nama mirip (views_helper) dianggap melanggar"
echo "check_layers_test: ok"
