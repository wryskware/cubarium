#!/bin/sh
# Deploy this checkout to the Tachyon: rsync, native release build on the board,
# install, restart. Runs **on the development host**.
#
#     ./scripts/tachyon-deploy.sh            # build, install, restart
#     ./scripts/tachyon-deploy.sh --no-restart
#     TACHYON=root@192.168.68.68 ./scripts/tachyon-deploy.sh
#
# Use the IP. `tachyon-8968c731.local` resolves to a link-local IPv6 address
# that ssh refuses.
set -eu

TACHYON=${TACHYON:-root@192.168.68.68}
REMOTE=${REMOTE:-/root/cubarium-deploy}
RESTART=yes
[ "${1:-}" = "--no-restart" ] && RESTART=no

REPO=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

echo "==> rsync $REPO -> $TACHYON:$REMOTE"
rsync -a --delete \
      --exclude '/target/' --exclude '.git' --exclude '/.claude/' \
      --exclude '/captures/' --exclude '/graft/' --exclude 'window/target/' \
      --exclude '/state/' \
      "$REPO/" "$TACHYON:$REMOTE/"

# taskset -c 4-7, never a single core: core_ctl isolates idle big cores, and a
# one-core mask is both a slow build and a misleading one.
#
# The first build in an empty $REMOTE/target is a cold workspace build and takes
# tens of minutes on the A78s. Seed it once from any earlier tree built with the
# same toolchain to make it an incremental one:
#     ssh $TACHYON 'cp -a /root/cubarium-gs1/target /root/cubarium-deploy/target'
echo "==> build on the board (taskset -c 4-7, release)"
ssh "$TACHYON" "set -eu
    export PATH=/root/.cargo/bin:\$PATH
    cd $REMOTE
    start=\$(date +%s)
    taskset -c 4-7 cargo build --release -p cubarium --bin cubarium
    end=\$(date +%s)
    echo \"build: \$((end - start)) s\"
    ls -l target/release/cubarium"

echo "==> install"
ssh "$TACHYON" "$REMOTE/scripts/tachyon-install.sh $REMOTE"

if [ "$RESTART" = yes ]; then
    echo "==> restart cubarium.service"
    # The daemon takes one client at a time, so the old process has to have let
    # the socket go before the new one attaches. `restart` is synchronous about
    # the stop, and `connect_when_free` retries for 4 s on top of that.
    ssh "$TACHYON" 'systemctl restart cubarium.service && sleep 5 &&
        systemctl --no-pager --full status cubarium.service | head -12'
fi

echo "==> done; scripts/tachyon-status.sh for the full picture"
