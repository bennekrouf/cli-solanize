#!/bin/bash
# =============================================================================
# solanize — pull latest + rebuild + restart
# Run as root: sudo bash /opt/solanize/src/update.sh [service ...]
#
#   deploy-solanize                  everything below
#   deploy-solanize cli-solanize     just the Solana API
#   deploy-solanize landing          just ribh.io
#
# THIS FILE IS THE SOURCE OF TRUTH (same arrangement as api0's update.sh). The
# copy on the host is a symlink to it:
#
#   sudo ln -sf /opt/solanize/src/cli-solanize/deploy/update.sh /opt/solanize/src/update.sh
#
# Invoked on the host through: alias deploy-solanize='sudo bash /opt/solanize/src/update.sh'
#
# gateway-solanize and chat-solanize are not deployed yet; add them here when
# they are.
# =============================================================================
set -euo pipefail

APP_DIR="/opt/solanize"
SRC_DIR="$APP_DIR/src"
BIN_DIR="$APP_DIR/bin"
DEPLOY_USER="ubuntu"
DEPLOY_KEY="/var/www/.ssh/id_ed25519"

LANDING_DIR="/var/www/solanize"
CLI_ENV="$APP_DIR/cli-solanize.env"
CLI_RUNTIME="$APP_DIR/cli-solanize"
# cli-solanize logs here unconditionally (init_logging! in src/main.rs).
LOG_FILE="/var/log/solanize.log"

declare -A REPO_DIRS=(
  ["cli-solanize"]="$SRC_DIR/cli-solanize"
  ["landing"]="$LANDING_DIR"
)
ALL_SERVICES=(cli-solanize landing)

RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
err()  { echo -e "${RED}[X]${NC} $1"; exit 1; }
step() { echo -e "\n${CYAN}=== $1 ===${NC}\n"; }

[ "$EUID" -ne 0 ] && err "Run as root: sudo bash update.sh"

SERVICES=("$@")
[ ${#SERVICES[@]} -eq 0 ] && SERVICES=("${ALL_SERVICES[@]}")
for SERVICE in "${SERVICES[@]}"; do
  [ -n "${REPO_DIRS[$SERVICE]:-}" ] || err "Unknown service '$SERVICE' (known: ${ALL_SERVICES[*]})"
done
wants() { [[ " ${SERVICES[*]} " == *" $1 "* ]]; }

PM2=$(which pm2)
as_user() { sudo -u "$1" HOME="$(getent passwd "$1" | cut -d: -f6)" "${@:2}"; }

# =============================================================================
step "1/3 — Pull latest from git"
# =============================================================================

GIT_SSH="ssh -i $DEPLOY_KEY -o StrictHostKeyChecking=no"
GIT="git -c safe.directory=*"

for SERVICE in "${SERVICES[@]}"; do
  TARGET="${REPO_DIRS[$SERVICE]}"
  [ -d "$TARGET/.git" ] || err "$TARGET is not a git checkout — clone it first"
  OWNER=$(stat -c %U "$TARGET")
  # Commit any local changes (e.g. posts published on the host), then pull
  $GIT -C "$TARGET" add -A
  $GIT -C "$TARGET" diff --cached --quiet || $GIT -C "$TARGET" commit -m "auto-commit local changes before deploy"
  GIT_SSH_COMMAND="$GIT_SSH" $GIT -C "$TARGET" pull
  GIT_SSH_COMMAND="$GIT_SSH" $GIT -C "$TARGET" push || warn "  $SERVICE: push failed — local commits stay on the host"
  # git ran as root → give the tree back to whoever builds it
  chown -R "$OWNER:$OWNER" "$TARGET"
  log "  $SERVICE updated ($($GIT -C "$TARGET" log --oneline -1))"
done

# =============================================================================
step "2/3 — Rebuild"
# =============================================================================

if wants cli-solanize; then
  SRC="${REPO_DIRS[cli-solanize]}"
  echo "  → Rebuilding cli-solanize..."
  as_user "$DEPLOY_USER" bash -c "source ~/.cargo/env && cd '$SRC' && cargo build --release 2>&1"

  # cp→chmod→mv: atomic replace — never overwrites a running binary in-place
  mkdir -p "$BIN_DIR"
  cp "$SRC/target/release/cli-solanize" "$BIN_DIR/cli-solanize.new"
  chmod 755 "$BIN_DIR/cli-solanize.new"
  mv "$BIN_DIR/cli-solanize.new" "$BIN_DIR/cli-solanize"
  log "  cli-solanize rebuilt"

  # The service runs from its runtime dir; config.yaml holds no secrets (those
  # are in $CLI_ENV), so it is synced from git on every deploy.
  mkdir -p "$CLI_RUNTIME"
  cp "$SRC/config.yaml" "$CLI_RUNTIME/config.yaml"
  install -m 755 "$SRC/deploy/run-cli-solanize.sh" "$APP_DIR/run-cli-solanize.sh"
  log "  Synced config.yaml and run-cli-solanize.sh"

  if [ ! -f "$CLI_ENV" ]; then
    sed "s/^CLI_INTERNAL_SECRET=.*/CLI_INTERNAL_SECRET=$(openssl rand -hex 32)/" \
      "$SRC/deploy/cli-solanize.env.example" > "$CLI_ENV"
    chmod 600 "$CLI_ENV"
    warn "  Created $CLI_ENV with a fresh CLI_INTERNAL_SECRET."
    warn "    api0 OIDC stays off until SOLANIZE_OIDC_* are filled in there."
  fi

  touch "$LOG_FILE"
  chown "$DEPLOY_USER:$DEPLOY_USER" "$LOG_FILE" "$CLI_ENV"
  chown -R "$DEPLOY_USER:$DEPLOY_USER" "$BIN_DIR" "$CLI_RUNTIME" "$APP_DIR/run-cli-solanize.sh"
fi

if wants landing; then
  OWNER=$(stat -c %U "$LANDING_DIR")
  echo "  → Rebuilding landing (as $OWNER)..."
  as_user "$OWNER" bash -c "cd '$LANDING_DIR' && yarn install && yarn build"
  log "  landing rebuilt"
fi

# =============================================================================
step "3/3 — Restart"
# =============================================================================

if wants cli-solanize; then
  as_user "$DEPLOY_USER" $PM2 restart cli-solanize 2>/dev/null || \
    as_user "$DEPLOY_USER" $PM2 start "$APP_DIR/run-cli-solanize.sh" --name cli-solanize

  PORT=$(grep -E '^ROCKET_PORT=' "$CLI_ENV" | cut -d= -f2)
  for _ in 1 2 3 4 5; do
    sleep 2
    curl -sf "http://127.0.0.1:${PORT:-9876}/solana/health" >/dev/null && break
  done
  if curl -sf "http://127.0.0.1:${PORT:-9876}/solana/health" >/dev/null; then
    log "  cli-solanize healthy on 127.0.0.1:${PORT:-9876}"
  else
    warn "  cli-solanize is not answering /solana/health — check: pm2 logs cli-solanize"
  fi
fi

if wants landing; then
  as_user "$DEPLOY_USER" $PM2 restart solanize-landing
  log "  solanize-landing restarted"
fi

as_user "$DEPLOY_USER" $PM2 save >/dev/null

echo ""
echo -e "${GREEN}Update complete.${NC}"
as_user "$DEPLOY_USER" $PM2 list
