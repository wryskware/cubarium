#!/usr/bin/env python3
"""Build the local concept gallery from artbook/README.md; leave images intact."""
from html import escape
from pathlib import Path
import re

from PIL import Image


def main():
    book = Path(__file__).resolve().parents[1] / "artbook"
    group = ""
    cards = []
    groups = []
    for line in (book / "README.md").read_text().splitlines():
        if line.startswith("## "):
            group = line[3:]
        if not line.startswith("| ") or line.startswith(("| Subject", "| ---")):
            continue
        subject, link, note = [cell.strip() for cell in line.split("|")[1:-1]]
        match = re.fullmatch(r"\[([^]]+)\]\(([^)]+)\)", link)
        if not match:
            raise ValueError(f"Invalid image link: {link}")
        title, src = match.groups()
        with Image.open(book / src) as im:
            alpha = "A" in im.getbands() and im.getchannel("A").getextrema()[0] < 255
            size = f"{im.width} × {im.height}"
        transparency = "Alpha channel present · inspect edge cleanup" if alpha else "Opaque background · cutout needed"
        if group not in groups:
            groups.append(group)
        cards.append(f'''<article data-group="{escape(group, quote=True)}">
<a class="preview" href="{escape(src, quote=True)}" target="_blank" rel="noopener"><img loading="lazy" src="{escape(src, quote=True)}" alt="{escape(subject + ': ' + title, quote=True)}"></a>
<div class="caption"><small>{escape(group)}</small><h2>{escape(subject)}</h2>
<p class="title">{escape(title)}</p><p>{escape(note)}</p>
<p class="meta">{size} · {transparency}</p><a href="{escape(src, quote=True)}">Open full image</a></div></article>''')
    buttons = ''.join(f'<button data-filter="{escape(g, quote=True)}" aria-pressed="false">{escape(g)}</button>' for g in groups)
    page = '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Cubarium · Artbook</title><style>
:root{color-scheme:dark;font:16px/1.55 system-ui,sans-serif;background:#11101a;color:#e7e3ef;--preview:#252331}
*{box-sizing:border-box}body{margin:0}header,main,footer{max-width:1500px;margin:auto;padding:28px}
h1{font-size:clamp(2rem,4vw,3.3rem);line-height:1.1;margin:10px 0}h2{font-size:1.3rem;margin:.3rem 0}
a{color:#b7cfff}header p{max-width:900px}.muted,small,.meta{color:#b9b3c7}small{text-transform:uppercase;letter-spacing:.07em}
nav,.backgrounds{display:flex;flex-wrap:wrap;gap:8px;margin:16px 0;align-items:center}
button{font:inherit;border:1px solid #575068;border-radius:20px;padding:7px 15px;background:#252130;color:inherit;cursor:pointer}
button[aria-pressed=true]{background:#d4c4ff;color:#211831;border-color:#d4c4ff}button:focus-visible,a:focus-visible{outline:3px solid #6ee7ed;outline-offset:3px}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,340px),1fr));gap:24px}
article{border:1px solid #3f384e;border-radius:14px;overflow:hidden;background:#1c1926}article[hidden]{display:none}
.preview{display:block;height:390px;background:var(--preview)}.preview img{width:100%;height:100%;object-fit:contain;padding:12px}
body[data-bg=checker] .preview{background-color:#c9c6d1;background-image:conic-gradient(#e9e7ed 25%,transparent 0 50%,#e9e7ed 0 75%,transparent 0);background-size:24px 24px}
body[data-bg=white]{--preview:#fff}body[data-bg=black]{--preview:#000}.caption{padding:20px}.caption p{margin:10px 0}.title{color:#cdb9ef}.meta{font-size:.8rem}
details{padding:14px 18px;background:#242030;border-radius:10px;max-width:1000px}summary{cursor:pointer}footer{color:#b9b3c7}
@media print{nav,.backgrounds{display:none}.grid{grid-template-columns:1fr 1fr}article{break-inside:avoid}.preview{height:260px}header,main{padding:12px}}
</style></head><body data-bg="checker"><header>
<small>Concept review · 24–25 September 2026</small><h1>Cubarium artbook</h1>
<p>Selected creatures and plants, retained alternatives, and terrain studies. These are visual references for a later rendering pass; body models and texture masters have not been updated.</p>
<p><a href="README.md">Selection notes</a> · <a href="../../../design/art-direction/species-dossiers-2026-09-21.md">Species dossiers</a> · <a href="../../../design/animal-body-reimagining-2026-09-24.md">Animal body dossiers</a> · <a href="../../../design/art-direction/species-colours-2026-09-24.md">Colour direction</a></p>
<details><summary>Outstanding refinements</summary><p>Remove painted outer halos and opaque backgrounds. Keep capgnawer sensor patches light pink. Shift bellwing’s fine corkscrew tendril slightly yellower than the muted peach pick. Velvetpad remains provisional; Bloomcrown retains both colours, leaning pink. Seedporter retains both selected green references. All terrain stays candidate material, and further colourway generation is deferred.</p></details>
<nav aria-label="Filter art"><button data-filter="all" aria-pressed="true">All</button>''' + buttons + '''</nav>
<div class="backgrounds" role="group" aria-label="Preview background"><span>Preview background:</span><button data-background="checker" aria-pressed="true">Checker</button><button data-background="white" aria-pressed="false">White</button><button data-background="black" aria-pressed="false">Black</button></div>
<p class="muted" id="count" aria-live="polite"></p></header><main><div class="grid">''' + '\n'.join(cards) + '''</div></main>
<footer>Click an image to inspect its full-resolution source. Background controls expose alpha; they do not remove baked-in halos. Rebuild from README.md with <code>python art/gen/tools/build_artbook.py</code>.</footer>
<script>
const cards=[...document.querySelectorAll('article')];
function count(){document.querySelector('#count').textContent=cards.filter(c=>!c.hidden).length+' of '+cards.length+' references';}count();
document.querySelectorAll('[data-filter]').forEach(b=>b.addEventListener('click',()=>{document.querySelectorAll('[data-filter]').forEach(x=>x.setAttribute('aria-pressed',String(x===b)));cards.forEach(c=>c.hidden=b.dataset.filter!=='all'&&c.dataset.group!==b.dataset.filter);count();}));
document.querySelectorAll('[data-background]').forEach(b=>b.addEventListener('click',()=>{document.body.dataset.bg=b.dataset.background;document.querySelectorAll('[data-background]').forEach(x=>x.setAttribute('aria-pressed',String(x===b)));}));
</script></body></html>'''
    (book / "index.html").write_text(page)
    print(f"Built {book / 'index.html'} ({len(cards)} references)")


if __name__ == "__main__":
    main()
