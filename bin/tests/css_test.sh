#!/usr/bin/env bash
# Tes CSS hasil build produksi (--minify): aturan yang pernah dibuang minifier tetap ada.
# Regresi: `translate: none` di `.dialog > *` dibuang lightningcss, isi dialog tergeser -50% dan terpotong.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
# shellcheck disable=SC1091
. "$ROOT/bin/_lib.sh"
ag_load_env

OUT="/tmp/agentgate-css-test.css"
"$ROOT/bin/tailwind" --minify -o "$OUT" >/dev/null 2>&1 || fail "build tailwind gagal"
RULE="$(ag_exec sh -c "tr -d '\n' <$OUT | grep -o '\.dialog>\*,\.dialog:is(\[open\],:popover-open)>\*{[^}]*}' | head -1")"
[ -n "$RULE" ] || fail "aturan .dialog > * tidak ditemukan di CSS minify"
for want in '--tw-translate-x:0' '--tw-translate-y:0' '--tw-scale-x:100%' '--tw-scale-y:100%' 'position:static'; do
  case "$RULE" in *"$want"*) ;; *) fail "aturan .dialog > * kehilangan '$want': $RULE" ;; esac
done
# Basecoat menggeser lewat `translate: var(--tw-translate-x) …`; variabel di atas yang menetralkannya.
ag_exec grep -q 'translate:var(--tw-translate-x) var(--tw-translate-y)' "$OUT" \
  || fail "Basecoat tidak lagi memakai --tw-translate-*: periksa ulang override dialog.css"
echo "css_test: ok"
