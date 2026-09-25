#!/bin/sh
# Deploy HEAD to the Tachyon: scripts/deploy.sh with config/hosts/tachyon.env.
#
#     ./scripts/tachyon-deploy.sh            # build, install, restart
#     ./scripts/tachyon-deploy.sh --no-restart
#     TACHYON=root@192.168.68.68 ./scripts/tachyon-deploy.sh
exec "$(dirname -- "$0")/deploy.sh" tachyon "$@"
