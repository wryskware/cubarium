#!/usr/bin/env bash
# Working ledgers: short, current notes per work area (Wrysk 2026-10-10: "memories are too
# permanent. these are like, almost session bound and need to be referenced or reimported to
# context whenever theres a compaction or we start a new thread in the same work area").
# Runs as a SessionStart hook (startup, resume, clear and compact), so every ledger touched in
# the last LEDGER_DAYS days comes back into context after a compaction or in a new thread.
# A ledger untouched for longer fades out by itself; delete it when its work is done.
dir="${CLAUDE_PROJECT_DIR:-$(pwd)}/.claude/ledgers"
days="${LEDGER_DAYS:-3}"
[ -d "$dir" ] || exit 0
mapfile -t files < <(find "$dir" -maxdepth 1 -name '*.md' -mtime "-$days" -printf '%T@ %p\n' | sort -rn | cut -d' ' -f2-)
[ "${#files[@]}" -gt 0 ] || exit 0
echo "WORKING LEDGERS (.claude/ledgers/): current notes for this work area. Read the one for your"
echo "topic before acting; keep it current as decisions, jobs and open questions change; keep it"
echo "short (state now, not history). Decided-but-unshipped items come first and must not be parked."
for f in "${files[@]}"; do
  echo
  echo "===== $(basename "$f") (updated $(date -r "$f" '+%Y-%m-%d %H:%M')) ====="
  head -n 120 "$f"
done
