"""Draw sprid's social preview (OGP) image: the icon's ghost, the name, what
it is and its numbers, in Almide's colours on black, 1280 x 640 as GitHub asks.
Writes assets/ogp.png."""

import importlib.util
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.join(os.path.dirname(__file__), "..")
W, H = 1280, 640
SS = 2  # supersampling for smooth edges

# The ghost, its colours and its drawing are the icon's.
_spec = importlib.util.spec_from_file_location("make_icon", os.path.join(os.path.dirname(__file__), "make-icon.py"))
icon = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(icon)

AVENIR = "/System/Library/Fonts/Avenir Next.ttc"
AVENIR_BOLD, AVENIR_DEMI, AVENIR_MEDIUM = 0, 2, 5
# The terminal's own font, where it is; else Menlo, as sprid falls back to.
MONO = next((f for f in [os.path.expanduser("~/Library/Fonts/UDEVGothic35NFLG-Regular.ttf")] if os.path.exists(f)), "/System/Library/Fonts/Menlo.ttc")
LIGHT_TEAL = icon.BANDS[0]


def font(path: str, size: float, index: int = 0) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(path, round(size * SS), index=index)


def ogp() -> Image.Image:
    w, h = W * SS, H * SS
    # Black with a faintly cool cast at the top left, as the icon's square.
    yy, xx = np.mgrid[0:h, 0:w]
    t = ((xx / w) * 0.4 + (yy / h) * 0.6)[..., None]
    top, bottom = np.array([0x1A, 0x1E, 0x26]), np.array([0x08, 0x0A, 0x0D])
    img = Image.fromarray((top + (bottom - top) * t).astype(np.uint8)).convert("RGBA")

    # The ghost on the left, 400 px tall, centred.
    k = 400 / icon.GHOST_H
    g = icon.ghost(k * SS)
    img.alpha_composite(g, (150 * SS, (H - 400) // 2 * SS + 4 * SS))

    d = ImageDraw.Draw(img)
    x = 560 * SS
    white = icon.WHITE + (255,)
    d.text((x - 8 * SS, 92 * SS), "sprid", font=font(AVENIR, 176, AVENIR_BOLD), fill=white)
    tagline = font(AVENIR, 42, AVENIR_DEMI)
    d.text((x, 344 * SS), "A fast, light terminal", font=tagline, fill=LIGHT_TEAL + (255,))
    d.text((x, 396 * SS), "written in Almide", font=tagline, fill=LIGHT_TEAL + (255,))
    # The numbers, as a terminal would show them.
    mono = font(MONO, 25)
    d.text((x, 476 * SS), "0.23 s start · 40 MB idle", font=mono, fill=icon.WHITE + (170,))
    d.text((x, 512 * SS), "scrollback that can't leak", font=mono, fill=icon.WHITE + (170,))

    # The shell's bands along the bottom edge.
    band_h = 7 * SS
    for i, c in enumerate(icon.BANDS):
        y = h - (len(icon.BANDS) - i) * band_h
        d.rectangle([0, y, w, y + band_h], fill=c + (255,))

    return img.resize((W, H), Image.LANCZOS).convert("RGB")


def main() -> None:
    out = os.path.join(ROOT, "assets", "ogp.png")
    ogp().save(out, optimize=True)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
