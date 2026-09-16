#!/bin/sh
# Install cubarium as a service on the Tachyon. Runs **on the board, as root**,
# against a checkout that has already been built there (scripts/tachyon-deploy.sh
# does the rsync, the build and then calls this).
#
#     /root/cubarium-deploy/scripts/tachyon-install.sh [REPO]
#
# Idempotent: every step is a create-if-missing or an overwrite with identical
# content, so running it twice changes nothing. It never touches
# /var/lib/cubarium/state — the world lives there and this script is not allowed
# to have an opinion about it.
#
# It does not start the service; the caller decides when the panel changes.
set -eu

REPO=${1:-$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)}
USER_NAME=cubarium
HOME_DIR=/var/lib/cubarium
BIN=/usr/local/bin/cubarium
UNIT=/etc/systemd/system/cubarium.service
DOC_DIR=/usr/local/share/doc/cubarium

[ "$(id -u)" = 0 ] || { echo "tachyon-install: must run as root" >&2; exit 1; }
[ -f "$REPO/target/release/cubarium" ] || {
    echo "tachyon-install: no $REPO/target/release/cubarium; build it first" >&2
    exit 1
}

# --- the user -------------------------------------------------------------
# A system user with no login and no password. `video` is the frame socket's
# group, `render` is /dev/dri/renderD128 (the Vulkan loader). It gets no
# capability beyond those two groups: the daemon keeps root and KMS.
if id "$USER_NAME" >/dev/null 2>&1; then
    echo "user $USER_NAME exists: $(id "$USER_NAME")"
else
    useradd --system --home-dir "$HOME_DIR" --shell /usr/sbin/nologin \
            --groups video,render "$USER_NAME"
    echo "created $(id "$USER_NAME")"
fi
# Asserted every run rather than only at creation: a board that gains the groups
# later (GS-2b added `particle` to an empty `render` by hand) must not need a
# second procedure.
for g in video render; do
    id -nG "$USER_NAME" | tr ' ' '\n' | grep -qx "$g" || usermod -aG "$g" "$USER_NAME"
done

# --- the directories ------------------------------------------------------
# `state` is the world; `art` is the baked atelier pack. Both owned by the
# service user, because the service user is the only thing that writes them.
install -d -o "$USER_NAME" -g "$USER_NAME" -m 755 "$HOME_DIR"
install -d -o "$USER_NAME" -g "$USER_NAME" -m 755 "$HOME_DIR/state"
install -d -o "$USER_NAME" -g "$USER_NAME" -m 755 "$HOME_DIR/art"

# --- the art --------------------------------------------------------------
# --delete so a removed sprite really goes; the pack is 76 KiB and read-only to
# the running world, which never writes here.
rsync -a --delete "$REPO/assets/atelier/" "$HOME_DIR/art/"
chown -R "$USER_NAME:$USER_NAME" "$HOME_DIR/art"

# --- the fresh-world config ----------------------------------------------
# Root-owned and world-readable: the service reads it, and only an operator
# edits it. Rewriting it cannot disturb a world that already exists — on a
# resume the runner takes `capacity` and `weather.moving` from it and nothing
# else, and the founders below are spent at creation only.
install -o root -g root -m 644 "$REPO/config/tachyon/world.toml" "$HOME_DIR/world.toml"

# --- the binary -----------------------------------------------------------
# Root-owned 0755: the service user runs it and must not be able to replace it.
install -o root -g root -m 755 "$REPO/target/release/cubarium" "$BIN"

# --- the unit -------------------------------------------------------------
install -d -m 755 "$DOC_DIR"
install -o root -g root -m 644 "$REPO/docs/tachyon.md" "$DOC_DIR/tachyon.md"
install -o root -g root -m 644 "$REPO/config/tachyon/cubarium.service" "$UNIT"
systemctl daemon-reload
systemctl enable cubarium.service >/dev/null

echo "installed:"
ls -l "$BIN" "$UNIT" "$HOME_DIR/world.toml"
ls -ld "$HOME_DIR" "$HOME_DIR/state" "$HOME_DIR/art"
echo "art: $(find "$HOME_DIR/art" -type f | wc -l) file(s)"
echo "$("$BIN" --version 2>/dev/null || echo 'cubarium (no --version)')"
