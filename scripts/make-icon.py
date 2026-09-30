"""Draw sprid's icon: a ghost with a terminal cursor for a mouth, in Tokyo
Night colours, on a macOS-shaped rounded square. Writes assets/icon.png and
assets/Sprid.icns (via iconutil)."""

import os
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.join(os.path.dirname(__file__), "..")
S = 1024
SS = 4  # supersampling for smooth edges


def icon() -> Image.Image:
    n = S * SS
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))

    # The rounded square macOS icons sit in: 824 of 1024, radius ~185.
    inset, radius = 100 * SS, 185 * SS
    bg = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    grad = Image.new("RGBA", (n, n))
    top, bottom = (0x2A, 0x2F, 0x4A), (0x16, 0x17, 0x22)
    gd = ImageDraw.Draw(grad)
    for y in range(n):
        t = y / n
        gd.line([(0, y), (n, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,))
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle([inset, inset, n - inset, n - inset], radius, fill=255)
    bg.paste(grad, (0, 0), mask)

    # Soft shadow under the square.
    shadow = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle([inset, inset + 18 * SS, n - inset, n - inset + 18 * SS], radius, fill=(0, 0, 0, 110))
    shadow = shadow.filter(ImageFilter.GaussianBlur(22 * SS))
    img = Image.alpha_composite(img, shadow)
    img = Image.alpha_composite(img, bg)

    # The ghost: a dome, straight sides, a scalloped hem.
    d = ImageDraw.Draw(img)
    ghost = (0xC0, 0xCA, 0xF5, 255)
    cx, top_y, w = 512 * SS, 250 * SS, 420 * SS
    left, right = cx - w // 2, cx + w // 2
    d.ellipse([left, top_y, right, top_y + w], fill=ghost)
    body_bottom = 700 * SS
    d.rectangle([left, top_y + w // 2, right, body_bottom], fill=ghost)
    # Odd, so the hem is symmetric: bumps at both ends, bites between.
    scallops = 5
    sw = w / scallops
    for i in range(scallops):
        x0 = left + i * sw
        # Alternate: bumps below the hem, bites into it.
        if i % 2 == 0:
            d.ellipse([x0, body_bottom - sw / 2, x0 + sw, body_bottom + sw / 2], fill=ghost)
        else:
            d.ellipse([x0, body_bottom - sw / 2, x0 + sw, body_bottom + sw / 2], fill=(0, 0, 0, 0))
    ink = (0x1A, 0x1B, 0x26, 255)
    eye_w, eye_h, eye_y = 62 * SS, 92 * SS, 410 * SS
    for ex in (cx - 95 * SS, cx + 95 * SS):
        d.ellipse([ex - eye_w // 2, eye_y, ex + eye_w // 2, eye_y + eye_h], fill=ink)
    # A prompt and a block cursor for a mouth.
    prompt_y = 560 * SS
    blue = (0x7A, 0xA2, 0xF7, 255)
    stroke = 26 * SS
    px = cx - 90 * SS
    d.line([(px, prompt_y), (px + 55 * SS, prompt_y + 45 * SS), (px, prompt_y + 90 * SS)], fill=blue, width=stroke, joint="curve")
    d.rectangle([cx + 10 * SS, prompt_y + 5 * SS, cx + 70 * SS, prompt_y + 90 * SS], fill=blue)

    # Bites in the hem show the square's gradient through them.
    hem = Image.new("L", (n, n), 0)
    hd = ImageDraw.Draw(hem)
    for i in range(1, scallops, 2):
        x0 = left + i * sw
        hd.ellipse([x0, body_bottom - sw / 2, x0 + sw, body_bottom + sw / 2], fill=255)
    img.paste(bg, (0, 0), Image.composite(mask, Image.new("L", (n, n), 0), hem))

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
