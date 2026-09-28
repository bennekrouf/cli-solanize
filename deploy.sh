#!/bin/bash
# =============================================================================
# solanize — cli-solanize deploy
# Triggered by GitHub Actions on push to master (.github/workflows/deploy.yml).
# Run as root: sudo bash /opt/solanize/src/cli-solanize/deploy.sh
#
# Kept as the CI entry point; the work is in deploy/update.sh, the same script
# `deploy-solanize` runs, so CI and a manual deploy cannot drift apart.
# =============================================================================
set -euo pipefail
exec bash "$(dirname "$(readlink -f "$0")")/deploy/update.sh" cli-solanize
