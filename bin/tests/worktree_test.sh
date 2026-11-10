#!/usr/bin/env bash
# Tes bin/worktree dengan repo git sementara dan `docker` palsu (tanpa Docker/Postgres sungguhan).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

# Repo sementara dengan bin/ dan docker/ dari repo ini.
REPO="$TMP/repo"; mkdir -p "$REPO"
cp -R "$ROOT/bin" "$ROOT/docker" "$REPO/"
cd "$REPO"
git init -q -b staging
git config user.email t@t.test; git config user.name t
printf 'worktrees/\n.worktree\n' >.gitignore
git add -A; git commit -qm init

# docker palsu: mencatat panggilan, menyimpan daftar database di file.
mkdir -p "$TMP/fakebin"
STATE="$TMP/dbs"; LOG="$TMP/docker.log"; : >"$STATE"; : >"$LOG"
cat >"$TMP/fakebin/docker" <<SHIM
#!/usr/bin/env bash
echo "\$*" >>"$LOG"
args="\$*"
case "\$args" in
  *"compose version"*) exit 0 ;;
  *"ps --status running --services"*) echo db; exit 0 ;;
  *"pg_available_extensions"*) echo 1; exit 0 ;;
  *"FROM pg_database WHERE datname = '"*)
    name="\${args##*datname = \'}"; name="\${name%%\'*}"
    grep -qx "\$name" "$STATE" && echo 1; exit 0 ;;
  *"CREATE DATABASE"*)
    name="\${args#*CREATE DATABASE \"}"; name="\${name%%\"*}"
    echo "\$name" >>"$STATE"; exit 0 ;;
  *"DROP DATABASE"*)
    name="\${args#*DROP DATABASE IF EXISTS \"}"; name="\${name%%\"*}"
    grep -vx "\$name" "$STATE" >"$STATE.tmp" || true; mv "$STATE.tmp" "$STATE"; exit 0 ;;
  *"-tAc SELECT 1"*) echo 1; exit 0 ;;
esac
exit 0
SHIM
chmod +x "$TMP/fakebin/docker"
echo agentgate_dev >"$STATE"
export PATH="$TMP/fakebin:$PATH" AG_WORKTREE_SKIP_MIGRATE=1

# new membuat folder, .worktree, dan database.
bin/worktree new feature/x --no-server >/dev/null || fail "new gagal"
[ -d worktrees/feature-x ] || fail "folder tidak dibuat"
grep -q '^WORKTREE_ID=feature_x$' worktrees/feature-x/.worktree || fail ".worktree id"
grep -q '^WORKTREE_PORT=51[5-9][0-9]$' worktrees/feature-x/.worktree || fail ".worktree port"
grep -q 'CREATE DATABASE "agentgate_dev_feature_x" TEMPLATE agentgate_dev' "$LOG" || fail "clone db dev"
grep -q 'CREATE DATABASE "agentgate_test_feature_x"' "$LOG" || fail "db test"

# new kedua kali idempoten.
: >"$LOG"
bin/worktree new feature/x --no-server >/dev/null || fail "new kedua gagal"
if grep -q 'CREATE DATABASE' "$LOG"; then fail "new kedua membuat database lagi"; fi

# Branch berbeda dengan id/folder sama ditolak.
if bin/worktree new feature-x --no-server >/dev/null 2>&1; then fail "bentrok feature/x vs feature-x lolos"; fi

# close ditolak bila ada file untracked.
touch worktrees/feature-x/baru.txt
if bin/worktree close feature/x --yes >/dev/null 2>&1; then fail "close dengan file untracked lolos"; fi
[ -d worktrees/feature-x ] || fail "folder terhapus padahal close ditolak"
rm worktrees/feature-x/baru.txt

# close ditolak bila branch punya commit yang belum di-merge.
(cd worktrees/feature-x && echo a >a.txt && git add a.txt && git commit -qm a)
if bin/worktree close feature/x --yes >/dev/null 2>&1; then fail "close branch belum di-merge lolos"; fi

# close --force membuang semuanya.
bin/worktree close feature/x --force --yes >/dev/null || fail "close --force gagal"
[ ! -d worktrees/feature-x ] || fail "folder masih ada"
if grep -qx agentgate_dev_feature_x "$STATE"; then fail "db dev tidak di-drop"; fi
if git show-ref --verify --quiet refs/heads/feature/x; then fail "branch masih ada"; fi

# close branch yang sudah di-merge tanpa --force.
bin/worktree new feature/y --no-server >/dev/null
(cd worktrees/feature-y && echo b >b.txt && git add b.txt && git commit -qm b)
git merge -q --no-ff feature/y -m "merge y"
bin/worktree close feature/y --yes >/dev/null || fail "close branch yang sudah di-merge gagal"
[ ! -d worktrees/feature-y ] || fail "folder y masih ada"

echo "worktree_test: ok"
