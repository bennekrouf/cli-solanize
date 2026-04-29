#!/bin/bash
# =============================================================================
# solanize — cli-solanize deploy
# Triggered by GitHub Actions on push to master.
# Run as root: sudo bash /opt/solanize/src/cli-solanize/deploy.sh
# =============================================================================
set -euo pipefail

APP_DIR="/opt/solanize"
SRC_DIR="$APP_DIR/src"
BIN_DIR="$APP_DIR/bin"
DEPLOY_USER="ubuntu"
DEPLOY_KEY="/var/www/.ssh/id_ed25519"
SERVICE_SRC="$SRC_DIR/cli-solanize"
BINARY_NAME="cli-solanize"
PM2_NAME="cli-solanize"
ROCKET_PORT="9876"

RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
err()  { echo -e "${RED}[X]${NC} $1"; exit 1; }
step() { echo -e "\n${CYAN}=== $1 ===${NC}\n"; }

[ "$EUID" -ne 0 ] && err "Run as root: sudo bash deploy.sh"

# =============================================================================
step "1/3 — Pull latest cli-solanize from git"
# =============================================================================

[ -d "$SERVICE_SRC/.git" ] || err "$SERVICE_SRC is not a git repo"

GIT_SSH_COMMAND="ssh -i $DEPLOY_KEY -o StrictHostKeyChecking=no" \
  git -c safe.directory='*' -C "$SERVICE_SRC" fetch origin

BRANCH=$(git -C "$SERVICE_SRC" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "master")
git -c safe.directory='*' -C "$SERVICE_SRC" reset --hard "origin/$BRANCH"
log "cli-solanize pulled (branch: $BRANCH)"

chown -R "$DEPLOY_USER:$DEPLOY_USER" "$SERVICE_SRC"
log "Ownership restored to $DEPLOY_USER"

# =============================================================================
step "2/3 — Rebuild cli-solanize"
# =============================================================================

CARGO_HOME="/home/$DEPLOY_USER/.cargo"
export PATH="$CARGO_HOME/bin:$PATH"

echo "  → Running cargo build --release ..."
sudo -u "$DEPLOY_USER" HOME="/home/$DEPLOY_USER" \
  bash -c "source ~/.cargo/env && cd '$SERVICE_SRC' && cargo build --release 2>&1"

# Atomic binary replace
cp "$SERVICE_SRC/target/release/$BINARY_NAME" "$BIN_DIR/$BINARY_NAME.new"
chmod 755 "$BIN_DIR/$BINARY_NAME.new"
mv "$BIN_DIR/$BINARY_NAME.new" "$BIN_DIR/$BINARY_NAME"
chown "$DEPLOY_USER:$DEPLOY_USER" "$BIN_DIR/$BINARY_NAME"
log "Binary replaced: $BIN_DIR/$BINARY_NAME"

# Sync config (never overwrite production config if it already exists)
if [ ! -f "$APP_DIR/config/cli-solanize.yaml" ]; then
  mkdir -p "$APP_DIR/config"
  cp "$SERVICE_SRC/config.yaml" "$APP_DIR/config/cli-solanize.yaml"
  chown "$DEPLOY_USER:$DEPLOY_USER" "$APP_DIR/config/cli-solanize.yaml"
  log "Default config installed to $APP_DIR/config/cli-solanize.yaml"
else
  warn "Production config already exists — not overwriting"
fi

# =============================================================================
step "3/3 — Restart cli-solanize"
# =============================================================================

PM2=$(which pm2)
sudo -u "$DEPLOY_USER" HOME="/home/$DEPLOY_USER" \
  $PM2 restart "$PM2_NAME" || \
  sudo -u "$DEPLOY_USER" HOME="/home/$DEPLOY_USER" \
    $PM2 start "$SRC_DIR/ecosystem.config.js" --only "$PM2_NAME"

sudo -u "$DEPLOY_USER" HOME="/home/$DEPLOY_USER" $PM2 save
log "$PM2_NAME restarted"

echo ""
echo -e "${GREEN}cli-solanize deploy complete.${NC}"
sudo -u "$DEPLOY_USER" HOME="/home/$DEPLOY_USER" $PM2 list
