#!/usr/bin/env python3
# Draws the 1200x630 Open Graph / Twitter cards in img/. Run after changing a
# page title or description:  python3 -I scripts/og.py  (needs Pillow; font:
# scripts/og-fonts/Inter.ttf, SIL OFL)
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
FONT = ROOT / 'scripts/og-fonts/Inter.ttf'
W, H, PAD = 1200, 630, 72
TOP, BOTTOM = (102, 126, 234), (118, 75, 162)  # the site's --primary / --secondary
WHITE, SOFT, PILL = (255, 255, 255), (232, 228, 255), (240, 147, 251)

CARDS = {
    'og.png': ('JavaScript for both RGB lines',
               "Each line's own Rust crates, compiled to WebAssembly: validate transfers, create "
               'contracts and invoices, and pay invoices, offline and keyless.'),
    'og-create.png': ('Create an RGB contract or invoice',
                      'Issue a token or make an invoice on a test network, for either RGB line, '
                      'offline in your browser.'),
    'og-check.png': ('Check an RGB invoice or file',
                     'Paste an invoice or drop a consignment: line, validity, contract and state, '
                     'checked offline in your browser.'),
    'og-conformance.png': ('Conformance, live in your browser',
                           'The upstream RGB test vectors, real CLI invoices and consignments, '
                           'run against both wasm builds as you watch.'),
}


def font(size, weight):
    f = ImageFont.truetype(str(FONT), size)
    f.set_variation_by_name(weight)
    return f


def wrap(draw, text, f, width):
    lines, cur = [], ''
    for word in text.split():
        trial = f'{cur} {word}'.strip()
        if draw.textlength(trial, font=f) <= width:
            cur = trial
        else:
            lines.append(cur)
            cur = word
    return lines + [cur]


def pill(d, x, y, text):
    f = font(26, b'SemiBold')
    w = d.textlength(text, font=f)
    d.rounded_rectangle((x, y, x + w + 40, y + 50), radius=25, outline=PILL, width=3)
    d.text((x + 20, y + 25), text, font=f, fill=WHITE, anchor='lm')
    return x + w + 40


for name, (title, desc) in CARDS.items():
    img = Image.new('RGB', (W, H))
    d = ImageDraw.Draw(img)
    for x in range(W):  # left-to-right gradient, like the site header
        t = x / W
        d.line((x, 0, x, H), fill=tuple(round(TOP[i] + (BOTTOM[i] - TOP[i]) * t) for i in range(3)))
    d.text((PAD, PAD + 20), 'rgbjs', font=font(56, b'ExtraBold'), fill=WHITE, anchor='lm')
    y = 200
    tf = font(68, b'Bold')
    for line in wrap(d, title, tf, W - 2 * PAD):
        d.text((PAD, y), line, font=tf, fill=WHITE)
        y += 80
    y += 16
    df = font(30, b'Regular')
    for line in wrap(d, desc, df, W - 2 * PAD)[:3]:
        d.text((PAD, y), line, font=df, fill=SOFT)
        y += 42
    by = H - PAD - 50
    x = pill(d, PAD, by, 'v0.11')
    x = pill(d, x + 14, by, 'v0.12')
    d.text((x + 20, by + 25), 'early development', font=font(26, b'Medium'), fill=SOFT, anchor='lm')
    d.text((W - PAD, by + 25), 'rgbjs.com', font=font(26, b'SemiBold'), fill=WHITE, anchor='rm')
    img.save(ROOT / 'img' / name, optimize=True)
    print('img/' + name)
