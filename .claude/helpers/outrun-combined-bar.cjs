#!/usr/bin/env node
// Two-row status line.
//   Row 1: the Outrun bar (model, dir, git, context meter, limits, cost).
//   Row 2: Graft's graph state, flattened to a single row.
//
// Row 2 is produced by running Graft's own shim rather than importing Graft's
// internals, so package upgrades and path changes can't break this file. The
// context_window field is withheld from that call because row 1 already draws
// the context meter and Graft would otherwise repeat it as a number.
//
// Named so it does NOT contain "graft-statusline.cjs": `graft init` identifies
// its own statusLine by that substring and rewrites it. Under this name Graft
// treats the setting as foreign and leaves it alone (it only logs a warning).

const path = require('path');
const { execFileSync } = require('child_process');

const OUTRUN = '/home/wrysk/.claude/statusline.js';
const GRAFT_SHIM = path.join(__dirname, 'graft-statusline.cjs');

const MUTED = '\x1b[38;5;244m';
const RESET = '\x1b[0m';
const BULLET = `${MUTED}▸ ${RESET}`;
const SEP = `${MUTED} · ${RESET}`;

// Graft emits a lead line plus an optional "▸ "-prefixed detail line. Drop the
// bullet marker and rejoin so the whole thing occupies one row.
function flatten(text) {
  return text
    .split('\n')
    .map((l) => (l.startsWith(BULLET) ? l.slice(BULLET.length) : l))
    .filter((l) => l.trim() !== '')
    .join(SEP);
}

let input = {};
try {
  input = JSON.parse(require('fs').readFileSync(0, 'utf8'));
} catch { /* no or invalid stdin; both rows degrade on their own */ }

const projectDir =
  process.env.CLAUDE_PROJECT_DIR || input.workspace?.project_dir || input.cwd || process.cwd();

const rows = [];

try {
  const { renderOutrun } = require(OUTRUN);
  const row = renderOutrun(input);
  if (row && row.trim()) rows.push(row);
} catch { /* Outrun bar unavailable — fall through to Graft's row alone */ }

try {
  const forGraft = { ...input };
  delete forGraft.context_window;

  const out = execFileSync(process.execPath, [GRAFT_SHIM], {
    input: JSON.stringify(forGraft),
    encoding: 'utf8',
    timeout: 3000,
    stdio: ['pipe', 'pipe', 'ignore'],
    env: { ...process.env, CLAUDE_PROJECT_DIR: projectDir },
  });
  const row = flatten(out);
  if (row) rows.push(row);
} catch { /* Graft unavailable or not built — keep the Outrun row */ }

process.stdout.write(rows.join('\n'));
