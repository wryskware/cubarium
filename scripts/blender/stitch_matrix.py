"""Stack the per-species row renders of animal_bodies.py into one matrix.png."""
import sys
from pathlib import Path

from PIL import Image

out = Path(sys.argv[1])
order = ["settled", "loftstrider", "lanternjaw", "chorister", "capgnawer", "ripple-snail", "seedporter"]
rows = [Image.open(out / f"row-{k}.png").convert("RGB") for k in order]
w = 2400
rows = [r.resize((w, round(r.height * w / r.width)), Image.LANCZOS) for r in rows]
sheet = Image.new("RGB", (w, sum(r.height for r in rows)), (12, 8, 24))
y = 0
for r in rows:
    sheet.paste(r, (0, y))
    y += r.height
sheet.save(out / "matrix.png")
print(sheet.size)
