#!/usr/bin/env bash
# Deploy this checkout's HEAD to a host: ship the committed tree, build natively on the
# host, install, restart its service. Runs **on the development host**. The host's
# profile is config/hosts/<host>.env.
#
#     scripts/deploy.sh voidrunner
#     scripts/deploy.sh tachyon --no-restart
#
# Ships HEAD (git archive), not the working tree: commit first. Files deleted in git
# stay on the host until removed by hand; nothing there is deleted.
#
# Profile variables:
#   SSH          ssh destination
#   REMOTE       checkout directory on the host (relative to the remote home, or absolute)
#   SCOPE        user | system: which systemd manages UNIT
#   UNIT         the service to restart
#   UNIT_FILE    optional: a unit in this repo to install (user scope) before restarting
#   INSTALL      optional: a script in the repo, run on the host as `$INSTALL $REMOTE`
#   BUILD_CPUS   optional: taskset list for the build
#   OFFLINE      yes: cargo --offline;  SYNC_REGISTRY yes: rsync this host's cargo registry first
#   STOP_BEFORE_BUILD  yes: stop UNIT before building (it stays stopped if the build fails)
set -euo pipefail
host=${1:?usage: deploy.sh <host> [--no-restart]}
restart=yes
[ "${2:-}" = --no-restart ] && restart=no

repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
profile="$repo/config/hosts/$host.env"
[ -f "$profile" ] || { echo "deploy: no profile $profile" >&2; exit 1; }
# shellcheck source=/dev/null
. "$profile"
: "${SSH:?} ${REMOTE:?} ${SCOPE:?} ${UNIT:?}"
ctl="systemctl"
[ "$SCOPE" = user ] && ctl="systemctl --user"

rev=$(git -C "$repo" rev-parse --short HEAD)
if ! git -C "$repo" diff --quiet HEAD --; then
  echo "!! uncommitted changes to tracked files are NOT shipped (HEAD is $rev)" >&2
fi

echo "==> ship $rev -> $SSH:$REMOTE"
ssh "$SSH" "mkdir -p $REMOTE"
git -C "$repo" archive HEAD | ssh "$SSH" "tar -x -C $REMOTE && echo $rev > $REMOTE/.deployed-rev"

if [ "${SYNC_REGISTRY:-no}" = yes ]; then
  echo "==> sync cargo registry"
  rsync -a --update "$HOME/.cargo/registry/cache/" "$SSH:.cargo/registry/cache/" 2>/dev/null || true
  rsync -a --update "$HOME/.cargo/registry/index/" "$SSH:.cargo/registry/index/" 2>/dev/null || true
fi

if [ "${STOP_BEFORE_BUILD:-no}" = yes ]; then
  echo "==> stop $UNIT before the build"
  ssh "$SSH" "$ctl stop $UNIT"
fi

echo "==> build on $host"
offline=""
[ "${OFFLINE:-no}" = yes ] && offline="--offline"
pin=""
[ -n "${BUILD_CPUS:-}" ] && pin="taskset -c $BUILD_CPUS"
# bash -s: the remote login shell may be fish.
if ! ssh "$SSH" bash -s <<EOF; then
set -eu
export PATH=\$HOME/.cargo/bin:\$PATH
export CUBARIUM_BUILD_REV=$rev
cd $REMOTE
start=\$(date +%s)
$pin cargo build --release $offline -p cubarium --bin cubarium
echo "build: \$((\$(date +%s) - start)) s"
EOF
  echo "!! build failed on $host; $UNIT not restarted" >&2
  exit 1
fi

if [ -n "${INSTALL:-}" ]; then
  echo "==> install ($INSTALL)"
  ssh "$SSH" "$REMOTE/$INSTALL $REMOTE"
elif [ -n "${UNIT_FILE:-}" ]; then
  echo "==> install $UNIT_FILE"
  ssh "$SSH" bash -s <<EOF
set -eu
mkdir -p \$HOME/.config/systemd/user
install -m 644 $REMOTE/$UNIT_FILE \$HOME/.config/systemd/user/$UNIT
$ctl daemon-reload
$ctl enable $UNIT >/dev/null
EOF
fi

if [ "$restart" = yes ]; then
  echo "==> restart $UNIT"
  ssh "$SSH" bash -s <<EOF
$ctl restart $UNIT && sleep 5
$ctl --no-pager --full status $UNIT | head -12
pid=\$($ctl show -p MainPID --value $UNIT)
[ "\$pid" != 0 ] && taskset -cp "\$pid"
EOF
else
  echo "==> leaving $UNIT as it is (--no-restart)"
fi
echo "==> $host runs $rev"
