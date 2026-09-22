#!/usr/bin/env bash
# GPT-image lane through the Codex CLI (no API key; uses Codex's built-in image_generation).
# Usage: gpt_image.sh <run-dir> <count> <prompt-file> [--edit <reference.png>...]
# Writes <run-dir>/NNNN.png and <run-dir>/codex-last.md. Model gpt-5.6-terra, reasoning high
# (Wrysk, 2026-09-21: mid-range model unless the prompt needs real intelligence).
set -euo pipefail
run="$1"; n="$2"; pf="$3"; shift 3
mkdir -p "$run"
imgs=()
if [ "${1:-}" = "--edit" ]; then shift; for r in "$@"; do imgs+=(-i "$r"); done; fi
model="${GPT_IMAGE_MODEL:-gpt-5.6-terra}"
task="You are an image-generation operator. Use your image_generation tool.
Generate $n image(s) from the prompt below. Save every result as a PNG into the directory
$(realpath "$run") named 0001.png, 0002.png, ... (continue numbering after any existing files).
Do not edit any other file. Do not add text, labels or captions to the images. If reference
images were attached, treat them as the design to keep consistent, not as a style to copy.
After saving, print one line per file: <filename> | <size> | <one-sentence description>.

PROMPT:
$(cat "$pf")"
codex exec -m "$model" -c model_reasoning_effort=high -c 'features.image_generation=true' \
  --sandbox workspace-write -C "$(realpath "$run")" --skip-git-repo-check \
  "${imgs[@]}" -o "$run/codex-last.md" "$task"
