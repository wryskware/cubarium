#!/usr/bin/env python3
"""Build art/gen/index.html: one static page for browsing generation runs.

Scans art/gen/runs/*/ and runs/*/ (only dirs that contain PNGs) and writes a
single HTML file that references images by relative path. Rerun after every
run; there is no config and nothing to edit by hand.

    python3 art/gen/tools/build_index.py
"""

import html
import os
import re
import sys
from datetime import datetime
from pathlib import Path
from urllib.parse import quote

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / "art" / "gen" / "index.html"
ROOTS = [REPO / "art" / "gen" / "runs", REPO / "runs"]
BRIEFS = REPO / "art" / "gen" / "briefs"
FINAL_PATTERNS = ["contact-sheet*.png", "smoke-sheet*.png", "sheet*.png",
                  "matrix*.png", "compare*.png"]
DATE_RE = re.compile(r"(\d{4}-\d{2}-\d{2})")


def rel(p: Path) -> str:
    return quote(os.path.relpath(p, OUT.parent).replace(os.sep, "/"))


def summary(run: Path) -> str:
    for name in ("LOG.md", "README.md"):
        f = run / name
        if not f.is_file():
            continue
        for line in f.read_text(errors="replace").splitlines():
            s = line.strip()
            if s and not s.startswith("#"):
                return s
    return ""


def briefs_for(run: Path) -> list[Path]:
    found = sorted(run.glob("*brief*.md"))
    if BRIEFS.is_dir():
        slug = DATE_RE.sub("", run.name).strip("-_")
        log = run / "LOG.md"
        log_text = log.read_text(errors="replace") if log.is_file() else ""
        for b in sorted(BRIEFS.glob("*.md")):
            if (slug and b.stem.startswith(slug)) or f"briefs/{b.name}" in log_text:
                found.append(b)
    return found


def sort_key(run: Path):
    mtime = run.stat().st_mtime
    m = DATE_RE.search(run.name)
    date = m.group(1) if m else datetime.fromtimestamp(mtime).strftime("%Y-%m-%d")
    return (date, mtime)


def collect():
    runs = []
    for root in ROOTS:
        if not root.is_dir():
            continue
        for d in root.iterdir():
            if d.is_dir() and any(d.rglob("*.png")):
                runs.append(d)
    runs.sort(key=sort_key, reverse=True)
    return runs


def figure(p: Path, caption: str) -> str:
    src = rel(p)
    return (f'<figure><a href="{src}"><img src="{src}" loading="lazy" alt=""></a>'
            f"<figcaption>{html.escape(caption)}</figcaption></figure>")


def section(run: Path, anchor: str) -> str:
    label = os.path.relpath(run, REPO)
    top = sorted(run.glob("*.png"))
    final = []
    for pat in FINAL_PATTERNS:
        for p in sorted(run.glob(pat)):
            if p not in final:
                final.append(p)
    shown = final or top
    all_pngs = sorted(run.rglob("*.png"))

    links = []
    for name in ("LOG.md", "README.md"):
        if (run / name).is_file():
            links.append(f'<a href="{rel(run / name)}">{name}</a>')
    for b in briefs_for(run):
        links.append(f'<a href="{rel(b)}">brief: {html.escape(b.name)}</a>')

    out = [f'<section id="{anchor}" data-name="{html.escape(label.lower())}">',
           f'<h2><a href="#{anchor}">{html.escape(run.name)}</a> '
           f'<small>{html.escape(os.path.dirname(label))}/</small></h2>']
    s = summary(run)
    if s:
        out.append(f'<p class="sum">{html.escape(s)}</p>')
    if links:
        out.append(f'<p class="links">{" · ".join(links)}</p>')
    if shown:
        cls = "grid final" if final else "grid"
        out.append(f'<div class="{cls}">'
                   + "".join(figure(p, p.name) for p in shown) + "</div>")
    else:
        out.append('<p class="sum">No top-level PNGs; see the full list.</p>')
    out.append(f"<details><summary>All PNGs ({len(all_pngs)})</summary>"
               '<div class="grid small">'
               + "".join(figure(p, str(p.relative_to(run))) for p in all_pngs)
               + "</div></details></section>")
    return "\n".join(out)


CSS = """
body{background:#111316;color:#d6d8dc;font:14px/1.45 system-ui,sans-serif;margin:0;padding:16px 24px}
a{color:#7fc8ff}h1{font-size:20px;margin:0 0 8px}
h2{font-size:16px;margin:0 0 4px}h2 small{color:#777;font-weight:normal}
#filter{background:#1d2025;color:#eee;border:1px solid #333;padding:6px 8px;width:320px;max-width:100%}
nav{columns:3 260px;margin:12px 0 24px;font-size:13px}nav a{display:block}
section{border-top:1px solid #2a2d33;padding:16px 0}
.sum{color:#aaa;margin:2px 0}.links{margin:2px 0 8px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:10px}
.grid.final{grid-template-columns:repeat(auto-fill,minmax(480px,1fr))}
.grid.small{grid-template-columns:repeat(auto-fill,minmax(140px,1fr));margin-top:8px}
figure{margin:0;background:#0a0b0d;padding:4px}
img{width:100%;height:auto;display:block;image-rendering:pixelated}
figcaption{font-size:11px;color:#888;word-break:break-all;margin-top:2px}
summary{cursor:pointer;color:#aaa;margin-top:8px}
"""

JS = """
document.getElementById('filter').addEventListener('input',function(){
  var q=this.value.toLowerCase();
  document.querySelectorAll('section').forEach(function(s){s.hidden=!s.dataset.name.includes(q)});
  document.querySelectorAll('nav a').forEach(function(a){a.hidden=!a.dataset.name.includes(q)});
});
"""


def main() -> int:
    runs = collect()
    toc, body = [], []
    for i, run in enumerate(runs):
        anchor = f"r{i}-" + re.sub(r"[^A-Za-z0-9_-]", "-", run.name)
        name = os.path.relpath(run, REPO).lower()
        toc.append(f'<a href="#{anchor}" data-name="{html.escape(name)}">'
                   f"{html.escape(run.name)}</a>")
        body.append(section(run, anchor))
    stamp = datetime.now().strftime("%Y-%m-%d %H:%M")
    page = (
        "<!doctype html><html><head><meta charset=\"utf-8\">"
        "<title>Cubarium art runs</title>"
        f"<style>{CSS}</style></head><body>"
        f"<h1>Cubarium art runs</h1><p class=\"sum\">{len(runs)} runs · built {stamp} "
        "by <code>python3 art/gen/tools/build_index.py</code></p>"
        '<input id="filter" type="search" placeholder="Filter runs by name">'
        f"<nav>{''.join(toc)}</nav>" + "\n".join(body) +
        f"<script>{JS}</script></body></html>\n")
    OUT.write_text(page)
    print(f"wrote {os.path.relpath(OUT, REPO)}: {len(runs)} runs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
