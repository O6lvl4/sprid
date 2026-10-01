"""Draw sprid's icon: a ghost in Almide's colours — its head the armadillo's
banded shell, teal from light to deep, its body Almide's navy — with a
terminal prompt for a mouth, on a white macOS-shaped rounded square. Writes
assets/icon.png and assets/Sprid.icns (via iconutil)."""

import os
import shutil
import subprocess

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(__file__), "..")
S = 1024
SS = 4  # supersampling for smooth edges

# Almide's logo: the shell's bands, light to deep, and the body's navy.
BANDS = [(0x14, 0xA2, 0xA2), (0x0F, 0x86, 0x8F), (0x0E, 0x7E, 0x86), (0x09, 0x65, 0x78)]
NAVY = (0x1F, 0x3A, 0x5F)
WHITE = (0xFF, 0xFF, 0xFF)


def ghost_mask(n: int, cx: float, top_y: float, w: float, hem_y: float, scallops: int) -> np.ndarray:
    """The ghost's silhouette: a dome, straight sides, a scalloped hem."""
    m = Image.new("L", (n, n), 0)
    d = ImageDraw.Draw(m)
    left, right = cx - w / 2, cx + w / 2
    d.ellipse([left, top_y, right, top_y + w], fill=255)
    d.rectangle([left, top_y + w / 2, right, hem_y], fill=255)
    sw = w / scallops
    for i in range(scallops):
        x0 = left + i * sw
        # Bumps below the hem at both ends and between, bites in the rest.
        d.ellipse([x0, hem_y - sw / 2, x0 + sw, hem_y + sw / 2], fill=255 if i % 2 == 0 else 0)
    return np.asarray(m) > 127


def icon() -> Image.Image:
    n = S * SS
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))

    # The rounded square macOS icons sit in: 824 of 1024, radius ~185; white
    # going faintly cool toward the bottom, as Almide's page is.
    inset, radius = 100 * SS, 185 * SS
    ys = np.linspace(0, 1, n)[:, None]
    top, bottom = np.array([0xFF, 0xFF, 0xFF]), np.array([0xE9, 0xF1, 0xF3])
    grad = (top + (bottom - top) * ys[..., None]).repeat(n, axis=1).astype(np.uint8)
    square = Image.new("L", (n, n), 0)
    ImageDraw.Draw(square).rounded_rectangle([inset, inset, n - inset, n - inset], radius, fill=255)
    bg = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    bg.paste(Image.fromarray(grad).convert("RGBA"), (0, 0), square)

    shadow = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle([inset, inset + 18 * SS, n - inset, n - inset + 18 * SS], radius, fill=(0x1F, 0x3A, 0x5F, 90))
    shadow = shadow.filter(ImageFilter.GaussianBlur(22 * SS))
    img = Image.alpha_composite(img, shadow)
    img = Image.alpha_composite(img, bg)

    # The ghost.
    cx, top_y, w, hem_y = 512 * SS, 236 * SS, 440 * SS, 708 * SS
    body = ghost_mask(n, cx, top_y, w, hem_y, 5)
    yy, xx = np.mgrid[0:n, 0:n]
    px = np.array(img)

    # The head is the shell: bands curving round a point to the right, as
    # the armadillo's do, light at the outside to deep at the inside, a white
    # gap between each; a white line parts it from the body.
    shell_bottom = 440 * SS
    gap = 12 * SS
    shell = body & (yy < shell_bottom)
    r = np.hypot(xx - (cx + 330 * SS), yy - 470 * SS)
    r_lo, r_hi = r[shell].min(), r[shell].max()
    t = np.clip((r_hi - r) / (r_hi - r_lo), 0, 0.9999)
    band = (t * len(BANDS)).astype(int)
    width = (r_hi - r_lo) / len(BANDS)
    to_edge = np.abs((t * len(BANDS) - np.round(t * len(BANDS)))) * width
    inside = (np.round(t * len(BANDS)) > 0) & (np.round(t * len(BANDS)) < len(BANDS))
    colors = np.array(BANDS, dtype=np.uint8)[band]
    px[shell, :3] = colors[shell]
    px[shell & inside & (to_edge < gap / 2), :3] = WHITE
    seam = body & (yy >= shell_bottom) & (yy < shell_bottom + gap)
    px[seam, :3] = WHITE
    lower = body & (yy >= shell_bottom + gap)
    px[lower, :3] = NAVY
    px[body, 3] = 255
    img = Image.fromarray(px)

    d = ImageDraw.Draw(img)
    # Eyes: white, looking a little toward the prompt.
    eye_w, eye_h, eye_y = 58 * SS, 76 * SS, 478 * SS
    for ex in (cx - 92 * SS, cx + 92 * SS):
        d.ellipse([ex - eye_w // 2, eye_y, ex + eye_w // 2, eye_y + eye_h], fill=WHITE + (255,))
        d.ellipse([ex - 12 * SS, eye_y + 30 * SS, ex + 16 * SS, eye_y + 62 * SS], fill=NAVY + (255,))
    # A prompt and a block cursor for a mouth, in the shell's light teal.
    prompt_y = 572 * SS
    teal = BANDS[0] + (255,)
    stroke = 24 * SS
    p = cx - 82 * SS
    d.line([(p, prompt_y), (p + 50 * SS, prompt_y + 40 * SS), (p, prompt_y + 80 * SS)], fill=teal, width=stroke, joint="curve")
    d.rectangle([cx + 12 * SS, prompt_y + 4 * SS, cx + 66 * SS, prompt_y + 80 * SS], fill=teal)

    return img.resize((S, S), Image.LANCZOS)


def main() -> None:
    assets = os.path.join(ROOT, "assets")
    os.makedirs(assets, exist_ok=True)
    img = icon()
    img.save(os.path.join(assets, "icon.png"))
    iconset = os.path.join(assets, "Sprid.iconset")
    shutil.rmtree(iconset, ignore_errors=True)
    os.makedirs(iconset)
    for size in (16, 32, 128, 256, 512):
        img.resize((size, size), Image.LANCZOS).save(os.path.join(iconset, f"icon_{size}x{size}.png"))
        img.resize((size * 2, size * 2), Image.LANCZOS).save(os.path.join(iconset, f"icon_{size}x{size}@2x.png"))
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(assets, "Sprid.icns")], check=True)
    shutil.rmtree(iconset)


if __name__ == "__main__":
    main()
