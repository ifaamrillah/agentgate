#!/usr/bin/env bash
# Tes JS halaman (server/tests/js/*.test.mjs) dengan `node --test`, tanpa browser.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
command -v node >/dev/null 2>&1 || { echo "FAIL: node (>= 18) dibutuhkan untuk tes JS" >&2; exit 1; }
node --test "$ROOT"/server/tests/js/*.test.mjs
