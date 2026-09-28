#!/bin/bash
# pm2 entry point for cli-solanize (same shape as /opt/api0/run-*.sh).
# Installed to /opt/solanize/run-cli-solanize.sh by deploy/update.sh.
#   pm2 start /opt/solanize/run-cli-solanize.sh --name cli-solanize
set -euo pipefail

set -a
source /opt/solanize/cli-solanize.env
set +a

cd /opt/solanize/cli-solanize
exec /opt/solanize/bin/cli-solanize -c config.yaml server
