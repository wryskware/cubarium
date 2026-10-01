"""One model-pass contact sheet: pick | builder render | voxel model, side by side.

    python3 scripts/blender/animal_model_sheet.py PICK MODEL VOXELS OUT TITLE NOTE

Run by `animal_models.py --sheets` (system Python, for PIL).
"""

import sys

from PIL import Image, ImageDraw

pick, model, voxels, out, title, note = sys.argv[1:7]
H = 560
ims = []
for path in (pick, model, voxels):
    im = Image.open(path).convert("RGBA")
    bg = Image.new("RGBA", im.size, (244, 242, 248, 255))
    im = Image.alpha_composite(bg, im).convert("RGB")
    ims.append(im.resize((round(im.width * H / im.height), H), Image.LANCZOS))
pad = 12
sheet = Image.new("RGB", (sum(i.width for i in ims) + pad * 4, H + 44), (24, 20, 34))
d = ImageDraw.Draw(sheet)
x = pad
for im, cap in zip(ims, ("pick (artbook)", "builder (animal_models.py)", f"voxel model, 0.125 m: {note}")):
    sheet.paste(im, (x, 36))
    d.text((x, 10), cap, fill=(230, 221, 248))
    x += im.width + pad
d.text((sheet.width - 8 * len(title) - pad, 10), title, fill=(255, 216, 96))
sheet.save(out)
