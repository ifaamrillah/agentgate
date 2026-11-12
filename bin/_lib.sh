# shellcheck shell=bash
# Library bersama untuk bin/*: env loader, helper port, dan helper Docker Compose.
# Satu tempat untuk semua perhitungan environment, supaya bin/dev, bin/test, bin/loco,
# dan bin/worktree tidak pernah berbeda pendapat tentang database atau port.
# Kompatibel bash 3.2 (macOS): tanpa associative array, tanpa ${var,,}.

AG_ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AG_MAIN_PORT=5150
AG_DB_HOST_PORT=54329

ag_die() {
  echo "agentgate: $*" >&2
  exit 1
}

# Port kosong pertama >= $1. Memakai /dev/tcp bash (tanpa lsof): koneksi berhasil berarti terpakai.
ag_port_in_use() {
  (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null
}

ag_free_port() {
  local port="$1"
  while ag_port_in_use "$port"; do
    port=$((port + 1))
  done
  echo "$port"
}

# Checkout ini linked git worktree (bukan checkout utama)? Dinilai dari git, bukan dari letak
# folder: worktree yang dibuat tool lain di luar worktrees/ juga harus dikenali.
ag_is_linked_worktree() {
  case "$AG_ROOT_DIR" in */worktrees/*) return 0 ;; esac
  local git_dir common_dir
  git_dir="$(git -C "$AG_ROOT_DIR" rev-parse --absolute-git-dir 2>/dev/null)" || return 1
  common_dir="$(cd "$AG_ROOT_DIR" && cd "$(git rev-parse --git-common-dir 2>/dev/null)" 2>/dev/null && pwd -P)" || return 1
  [ "$(cd "$git_dir" && pwd -P)" != "$common_dir" ]
}

# Set environment untuk checkout ini. Gagal tertutup: worktree tanpa .worktree berarti
# identitas tidak diketahui, dan kita berhenti alih-alih memakai database utama.
ag_load_env() {
  AG_ID=""
  AG_PREFERRED_PORT="$AG_MAIN_PORT"
  if ag_is_linked_worktree && [ ! -f "$AG_ROOT_DIR/.worktree" ]; then
    ag_die "file .worktree tidak ada di $AG_ROOT_DIR; siapkan dengan bin/worktree _setup dari checkout utama"
  fi
  if [ -f "$AG_ROOT_DIR/.worktree" ]; then
    # shellcheck disable=SC1091
    . "$AG_ROOT_DIR/.worktree"
    AG_ID="${WORKTREE_ID:-}"
    AG_PREFERRED_PORT="${WORKTREE_PORT:-$AG_MAIN_PORT}"
    [ -n "$AG_ID" ] || ag_die "WORKTREE_ID kosong di .worktree"
  fi

  if [ -n "$AG_ID" ]; then
    AG_DB_DEV="agentgate_dev_${AG_ID}"
    AG_DB_TEST="agentgate_test_${AG_ID}"
    AG_ID_OR_MAIN="$AG_ID"
    SESSION_COOKIE_SUFFIX="_${AG_ID}"
  else
    AG_DB_DEV="agentgate_dev"
    AG_DB_TEST="agentgate_test"
    AG_ID_OR_MAIN="main"
    SESSION_COOKIE_SUFFIX=""
  fi
  COMPOSE_PROJECT_NAME="agentgate-rs-$(printf '%s' "$AG_ID_OR_MAIN" | tr '_' '-')"
  # Host db di dalam network Docker; dari host sendiri pakai 127.0.0.1:$AG_DB_HOST_PORT.
  DATABASE_URL="postgres://agentgate:agentgate@db:5432/${AG_DB_DEV}"
  TEST_DATABASE_URL="postgres://agentgate:agentgate@db:5432/${AG_DB_TEST}"
  # Tag image dev = hash Dockerfile: branch yang mengubah Dockerfile otomatis memakai image baru.
  if [ ! -f "$AG_ROOT_DIR/docker/dev/Dockerfile" ]; then
    AG_IMAGE_TAG="latest"
  elif command -v sha256sum >/dev/null 2>&1; then
    AG_IMAGE_TAG="$(sha256sum "$AG_ROOT_DIR/docker/dev/Dockerfile" | cut -c1-12)"
  else
    AG_IMAGE_TAG="$(shasum -a 256 "$AG_ROOT_DIR/docker/dev/Dockerfile" | cut -c1-12)"
  fi
  AG_UID="$(id -u)"
  AG_GID="$(id -g)"
  export AG_ID AG_ID_OR_MAIN AG_DB_DEV AG_DB_TEST SESSION_COOKIE_SUFFIX COMPOSE_PROJECT_NAME \
    DATABASE_URL TEST_DATABASE_URL AG_UID AG_GID AG_PREFERRED_PORT AG_IMAGE_TAG
  export AG_ROOT="$AG_ROOT_DIR"
}

ag_compose_shared() {
  docker compose -p agentgate-rs-shared -f "$AG_ROOT_DIR/docker/compose.shared.yaml" "$@"
}

ag_compose_app() {
  docker compose -p "$COMPOSE_PROJECT_NAME" -f "$AG_ROOT_DIR/docker/compose.app.yaml" "$@"
}

# Volume cache yang dibagi semua checkout dibuat di sini (bukan oleh project compose app),
# supaya `down --volumes` saat menutup worktree tidak ikut menghapusnya.
AG_SHARED_VOLUMES="agentgate-rs-cargo-registry agentgate-rs-cargo-git agentgate-rs-dev-cache"

ag_ensure_shared() {
  local v
  for v in $AG_SHARED_VOLUMES; do
    docker volume inspect "$v" >/dev/null 2>&1 || docker volume create "$v" >/dev/null
  done
  if ! ag_compose_shared ps --status running --services 2>/dev/null | grep -qx db; then
    ag_compose_shared --progress quiet up -d --wait >&2 || ag_die "gagal menyalakan layanan bersama (db, s3, mailpit)"
    ag_ensure_bucket
  fi
}

# Bucket S3 dev (lampiran tiket, logo) di SeaweedFS bersama, dipastikan setiap layanan bersama
# dinyalakan. Idempoten: bucket yang sudah ada dibiarkan. Hanya menyentuh container layanan bersama agentgate-rs-shared.
AG_S3_BUCKET="agentgate-dev"
ag_ensure_bucket() {
  ag_compose_shared exec -T s3 sh -c "echo s3.bucket.list | weed shell 2>/dev/null | grep -qw $AG_S3_BUCKET \
    || echo 's3.bucket.create -name $AG_S3_BUCKET' | weed shell >/dev/null 2>&1" \
    || ag_die "gagal membuat bucket S3 dev $AG_S3_BUCKET"
}

# Container app checkout ini. Port dipilih saat container dibuat; selama container hidup,
# port-nya tetap (AG_PORT dibaca dari label container).
ag_ensure_app() {
  ag_ensure_shared
  local cid
  cid="$(ag_compose_app ps -q app 2>/dev/null)"
  if [ -n "$cid" ] && [ "$(docker inspect -f '{{.State.Running}}' "$cid" 2>/dev/null)" = "true" ] \
    && [ "$(docker inspect -f '{{.Config.Image}}' "$cid")" = "agentgate-rs-dev:$AG_IMAGE_TAG" ]; then
    AG_PORT="$(docker inspect -f '{{index .Config.Labels "agentgate.port"}}' "$cid")"
  else
    # Container lama (berhenti, atau image-nya usang) dihentikan dulu supaya port-nya bebas
    # sebelum port baru dipilih. Volume tidak dihapus.
    [ -n "$cid" ] && ag_compose_app stop app >/dev/null 2>&1
    AG_PORT="$(ag_free_port "$AG_PREFERRED_PORT")"
    export AG_PORT
    # Mountpoint volume target/ dibuat lebih dulu oleh user host; kalau tidak, Docker
    # membuatnya sebagai root dan folder itu tidak bisa dihapus user.
    mkdir -p "$AG_ROOT_DIR/target"
    ag_compose_app --progress quiet up -d --wait app >&2 || ag_die "gagal menyalakan container app"
  fi
  export AG_PORT
}

# Jalankan perintah di container app dengan env checkout ini.
# Pakai -T otomatis bila stdin bukan terminal (CI, pipe).
ag_exec() {
  ag_ensure_app
  local tty_flag=""
  [ -t 0 ] && [ -t 1 ] || tty_flag="-T"
  ag_compose_app exec $tty_flag \
    -e DATABASE_URL="$DATABASE_URL" \
    -e TEST_DATABASE_URL="$TEST_DATABASE_URL" \
    -e PORT="$AG_PORT" \
    -e SESSION_COOKIE_SUFFIX="$SESSION_COOKIE_SUFFIX" \
    -e AG_ID="$AG_ID" \
    -e AG_WORKDIR="${AG_WORKDIR:-/workspace}" \
    app bash -c 'cd "$AG_WORKDIR" && exec "$@"' bash "$@"
}
