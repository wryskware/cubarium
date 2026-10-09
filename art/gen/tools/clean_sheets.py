"""Contact sheets for artbook glow cleanup (2026-10-09 brief).

usage: clean_sheets.py pair <name.png>   -> clean/sheets/<name>-pair.png
       clean_sheets.py plates <name.png>... -> clean/sheets/all-plate-size.png
Pair: original | clean, each on #24115F and on green. Plates: 176 px wide box
filter, 40 colours, nearest x3 (the field guide's plate size), on #24115F.
"""
import sys, os
from PIL import Image, ImageDraw
A = os.path.join(os.path.dirname(__file__), '..', 'artbook')
C = os.path.join(A, 'clean'); S = os.path.join(C, 'sheets')
PLATE = (0x24, 0x11, 0x5F, 255); GREEN = (0, 255, 0, 255)

def on(im, c, box):
    t = im.copy(); t.thumbnail(box)
    bg = Image.new('RGBA', box, c); bg.alpha_composite(t, ((box[0]-t.width)//2, (box[1]-t.height)//2))
    return bg

def pair(n):
    o = Image.open(os.path.join(A, n)).convert('RGBA'); c = Image.open(os.path.join(C, n)).convert('RGBA')
    W = Image.new('RGBA', (1024, 1060), (0, 0, 0, 255))
    for y, im in enumerate([o, c]):
        for x, col in enumerate([PLATE, GREEN]):
            W.alpha_composite(on(im, col, (512, 512)), (x*512, y*530))
        ImageDraw.Draw(W).text((4, y*530+514), ('original ' if y == 0 else 'clean ') + n, fill='white')
    W.convert('RGB').save(os.path.join(S, n[:-4] + '-pair.png'))

def plates(ns):
    tiles = []
    for n in ns:
        im = Image.open(os.path.join(C, n)).convert('RGBA')
        im = im.resize((176, round(im.height*176/im.width)), Image.BOX)
        bg = Image.new('RGBA', im.size, PLATE); bg.alpha_composite(im)
        q = bg.convert('RGB').quantize(40).convert('RGB')
        tiles.append(q.resize((q.width*3, q.height*3), Image.NEAREST))
    h = max(t.height for t in tiles)
    W = Image.new('RGB', (sum(t.width for t in tiles) + 8*len(tiles), h), PLATE[:3])
    x = 0
    for t in tiles: W.paste(t, (x, 0)); x += t.width + 8
    W.save(os.path.join(S, 'all-plate-size.png'))

os.makedirs(S, exist_ok=True)
pair(sys.argv[2]) if sys.argv[1] == 'pair' else plates(sys.argv[2:])
