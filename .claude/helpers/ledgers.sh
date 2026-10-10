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
# Budgets keep context small (Wrysk: "make sure old entries are knocked off the ledgers because
# otherwise context will start to explode with stupid junk"): a ledger over SOFT lines is flagged
# for pruning, and nothing past HARD lines is printed.
soft="${LEDGER_SOFT_LINES:-60}"
hard="${LEDGER_HARD_LINES:-80}"
echo "WORKING LEDGERS (.claude/ledgers/): current notes for this work area. Read the one for your"
echo "topic before acting. On every update, delete what is done: merged branches, answered"
echo "questions, finished jobs, superseded facts. State now, not history; at most $soft lines each."
echo "Decided-but-unshipped items come first and must not be parked."
for f in "${files[@]}"; do
  n=$(wc -l < "$f")
  echo
  echo "===== $(basename "$f") (updated $(date -r "$f" '+%Y-%m-%d %H:%M'), $n lines) ====="
  head -n "$hard" "$f"
  if [ "$n" -gt "$hard" ]; then
    echo "[... $((n - hard)) lines cut. PRUNE THIS LEDGER NOW: it is over the $hard-line limit.]"
  elif [ "$n" -gt "$soft" ]; then
    echo "[PRUNE THIS LEDGER: $n lines is over the $soft-line budget.]"
  fi
done
