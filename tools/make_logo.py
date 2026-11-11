"""Renders Last Bastion's own logo: assets\\logo\\last_bastion.png (the home screen draws it, src\\home.rs).

In the spirit of TAB's logo - brass, a gear, red cloth, a title with big first letters - but our own: a cog ring with a
bastion tower inside, red banners hanging from a brass rod, "LAST BASTION". Nothing is taken from TAB's pictures; it's
drawn from shapes and a font of Windows, so the picture may go into git.

    python tools\\make_logo.py

Only needs Pillow. It draws everything at twice the size and shrinks it at the end - smooth edges.
"""

import math
import os
import random

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "logo", "last_bastion.png")
# Drawn at this size, saved at half of it.
W, H = 3200, 1280
FONTS = [r"C:\Windows\Fonts\georgiab.ttf", r"C:\Windows\Fonts\timesbd.ttf"]

# The middle of the emblem, its cog's outer and inner radius.
CX, CY = W // 2, 470
COG_OUT, COG_ROOT, COG_IN = 400, 340, 292

rng = random.Random(7)


def scale(image, k):
    """Every value of a grey picture times `k` (clipped) - like `numpy` `img * k`."""
    return image.point(lambda v: min(255, int(v * k)))


def gradient(height, stops):
    """A vertical colour gradient, `height` rows, one column: stops = [(share 0..1, (r, g, b)), ...]."""
    column = Image.new("RGB", (1, height))
    for y in range(height):
        t = y / max(1, height - 1)
        for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
            if t0 <= t <= t1:
                f = (t - t0) / max(1e-6, t1 - t0)
                column.putpixel((0, y), tuple(int(a + (b - a) * f) for a, b in zip(c0, c1)))
                break
    return column


def lit(mask, base, bevel, strength=1.0):
    """`base` (RGB) lit like a raised metal shape cut out by `mask`: light from the top left, a bevel `bevel` wide, a
    little grime. Returns RGBA."""
    height = mask.filter(ImageFilter.GaussianBlur(bevel))
    d = max(2, bevel // 2)
    # Where the shape rises towards the top left: the edge faces the light. `offset` shifts the picture by (d, d).
    light = ImageChops.subtract(height, ImageChops.offset(height, d, d))
    shade = ImageChops.subtract(height, ImageChops.offset(height, -d, -d))
    light = scale(light, 3.0 * strength)
    shade = scale(shade, 3.0 * strength)
    out = ImageChops.screen(base, Image.merge("RGB", (light, scale(light, 0.9), scale(light, 0.6))))
    out = ImageChops.multiply(out, Image.merge("RGB", [ImageChops.invert(shade)] * 3))
    # Grime and wear: noise, a little blurred, laid softly over it.
    noise = Image.effect_noise(mask.size, 48).filter(ImageFilter.GaussianBlur(1.6))
    out = ImageChops.soft_light(out, Image.merge("RGB", [noise] * 3))
    scratches = Image.new("L", mask.size, 0)
    draw = ImageDraw.Draw(scratches)
    box = mask.getbbox() or (0, 0, *mask.size)
    for _ in range(int((box[2] - box[0]) * (box[3] - box[1]) / 9000)):
        x, y = rng.uniform(box[0], box[2]), rng.uniform(box[1], box[3])
        a = rng.uniform(0, math.pi)
        length = rng.uniform(10, 60)
        draw.line((x, y, x + math.cos(a) * length, y + math.sin(a) * length), fill=rng.randint(40, 110), width=2)
    out = ImageChops.screen(out, Image.merge("RGB", [scale(scratches, 0.6)] * 3))
    out = out.convert("RGBA")
    out.putalpha(mask)
    return out


def brass(mask, bevel, dark=False):
    """Brass: bright gold at the top, a light band, deep brown at the bottom of the shape."""
    box = mask.getbbox() or (0, 0, *mask.size)
    stops = [(0.0, (255, 236, 170)), (0.3, (228, 182, 96)), (0.55, (176, 120, 50)), (0.8, (214, 160, 80)), (1.0, (96, 60, 24))]
    if dark:
        stops = [(t, tuple(int(v * 0.75) for v in c)) for t, c in stops]
    column = gradient(box[3] - box[1], stops)
    base = Image.new("RGB", mask.size, stops[-1][1])
    base.paste(column.resize((mask.size[0], box[3] - box[1])), (0, box[1]))
    return lit(mask, base, bevel)


def outline(mask, width, color=(28, 16, 8)):
    """A dark rim round the shape, under it."""
    rim = mask.filter(ImageFilter.MaxFilter(width | 1))
    layer = Image.new("RGBA", mask.size, color + (0,))
    layer.putalpha(rim)
    return layer


def shadow(alpha, blur, offset, strength):
    layer = Image.new("RGBA", alpha.size, (0, 0, 0, 0))
    layer.putalpha(scale(ImageChops.offset(alpha, *offset).filter(ImageFilter.GaussianBlur(blur)), strength))
    return layer


# --- The banners: red cloth hanging from a brass rod, torn at the bottom ---


def banner(x0, width, top, length, phase):
    """One banner: its mask and its cloth (RGBA)."""
    mask = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(mask)
    sway = lambda y: 18 * math.sin((y - top) / length * math.pi * 1.3 + phase)  # noqa: E731
    left = [(x0 + sway(y), y) for y in range(top, top + length, 20)]
    right = [(x0 + width + sway(y), y) for y in range(top + length, top, -20)]
    # The bottom: a swallowtail, torn - many small jagged steps.
    tail = []
    steps = 24
    for k in range(steps + 1):
        u = k / steps
        depth = length - 150 * (1 - abs(u - 0.5) * 2) + rng.uniform(-30, 30)
        tail.append((x0 + width * (1 - u) + sway(top + depth), top + depth))
    draw.polygon(left + [(x0 + sway(top + length), top + length)] + tail[::-1][1:-1] + right, fill=255)
    # A few holes and tears.
    for _ in range(4):
        hx, hy = x0 + rng.uniform(0.2, 0.8) * width, top + rng.uniform(0.45, 0.85) * length
        r = rng.uniform(8, 22)
        draw.ellipse((hx - r, hy - r * 1.6, hx + r, hy + r * 1.6), fill=0)
    # The folds: bright and dark stripes down the cloth, darker towards its tail.
    row = Image.new("RGB", (W, 1))
    for x in range(W):
        u = (x - x0) / width
        v = 0.55 + 0.3 * math.sin(u * math.pi * 5 + phase) + 0.15 * math.sin(u * math.pi * 11 + 1.7 * phase)
        v = max(0.0, min(1.0, v))
        dark, bright = (90, 4, 6), (232, 34, 22)
        row.putpixel((x, 0), tuple(int(a + (b - a) * v) for a, b in zip(dark, bright)))
    cloth = row.resize((W, H))
    fade = gradient(H, [(0.0, (255, 255, 255)), (top / H, (255, 255, 255)), (min(1.0, (top + length) / H), (120, 120, 120)), (1.0, (120, 120, 120))])
    cloth = ImageChops.multiply(cloth, fade.resize((W, H)))
    # Its edges a bit darker, so it reads as cloth with thickness.
    inner = ImageChops.invert(mask.filter(ImageFilter.GaussianBlur(14)))
    cloth = ImageChops.multiply(cloth, Image.merge("RGB", [ImageChops.invert(scale(inner, 0.8))] * 3))
    noise = Image.effect_noise((W, H), 30).filter(ImageFilter.GaussianBlur(2))
    cloth = ImageChops.soft_light(cloth, Image.merge("RGB", [noise] * 3)).convert("RGBA")
    cloth.putalpha(mask)
    return cloth


def banners():
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    rod_y = 150
    # (left edge, width, length) on the left; the right side mirrored.
    for x0, width, length, phase in [(330, 330, 760, 0.3), (720, 290, 640, 2.1), (1130, 230, 520, 4.0)]:
        for left, ph in [(x0, phase), (W - x0 - width, phase + 1.0)]:
            cloth = banner(left, width, rod_y, length, ph)
            layer = Image.alpha_composite(layer, shadow(cloth.getchannel("A"), 10, (8, 14), 0.6))
            layer = Image.alpha_composite(layer, cloth)
    # The rod they hang from, its knobs at the ends.
    rod = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(rod)
    draw.rounded_rectangle((260, rod_y - 26, W - 260, rod_y + 18), radius=20, fill=255)
    for x in (230, W - 230):
        draw.ellipse((x - 46, rod_y - 50, x + 46, rod_y + 42), fill=255)
        draw.polygon([(x - 22, rod_y - 40), (x + 22, rod_y - 40), (x, rod_y - 130)], fill=255)
    layer = Image.alpha_composite(layer, outline(rod, 9))
    layer = Image.alpha_composite(layer, brass(rod, 8))
    return layer


# --- The emblem: a cog ring with a bastion tower inside ---


def emblem():
    ring = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(ring)
    teeth = 14
    points = []
    for k in range(teeth):
        a = 2 * math.pi * k / teeth - math.pi / 2
        half = math.pi / teeth
        for da, r in [(-half * 0.95, COG_ROOT), (-half * 0.5, COG_OUT), (half * 0.5, COG_OUT), (half * 0.95, COG_ROOT)]:
            points.append((CX + math.cos(a + da) * r, CY + math.sin(a + da) * r))
    draw.polygon(points, fill=255)
    draw.ellipse((CX - COG_IN, CY - COG_IN, CX + COG_IN, CY + COG_IN), fill=0)

    tower = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(tower)
    draw.rectangle((CX - 130, CY - 150, CX + 130, CY + 250), fill=255)  # the body
    draw.rectangle((CX - 168, CY - 192, CX + 168, CY - 140), fill=255)  # the parapet
    for left in (-168, -32, 104):  # three merlons
        draw.rectangle((CX + left, CY - 252, CX + left + 64, CY - 186), fill=255)
    draw.polygon([(CX - 190, CY + 290), (CX + 190, CY + 290), (CX + 150, CY + 230), (CX - 150, CY + 230)], fill=255)  # the foot
    # The gate (an arch), a window slit, a crack down the wall.
    draw.rectangle((CX - 58, CY + 110, CX + 58, CY + 232), fill=0)
    draw.ellipse((CX - 58, CY + 52, CX + 58, CY + 168), fill=0)
    draw.rectangle((CX - 15, CY - 100, CX + 15, CY - 20), fill=0)
    draw.ellipse((CX - 15, CY - 115, CX + 15, CY - 85), fill=0)
    crack = [(CX + 92, CY - 140), (CX + 70, CY - 80), (CX + 98, CY - 30), (CX + 74, CY + 30), (CX + 90, CY + 60)]
    draw.line(crack, fill=0, width=9)
    # The gate's portcullis: bars left standing in the arch.
    for x in (-34, 0, 34):
        draw.rectangle((CX + x - 6, CY + 70, CX + x + 6, CY + 232), fill=255)
    draw.rectangle((CX - 58, CY + 150, CX + 58, CY + 162), fill=255)
    # The foot reaches into the ring: one piece of brass.
    ring_and_tower = ImageChops.lighter(ring, tower)

    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    # A dark disc behind the tower, so it stands out against the banners.
    disc = Image.new("L", (W, H), 0)
    ImageDraw.Draw(disc).ellipse((CX - COG_IN - 6, CY - COG_IN - 6, CX + COG_IN + 6, CY + COG_IN + 6), fill=235)
    back = Image.new("RGBA", (W, H), (22, 12, 8, 0))
    back.putalpha(disc)
    layer = Image.alpha_composite(layer, back)
    layer = Image.alpha_composite(layer, shadow(ring_and_tower, 16, (10, 18), 0.8))
    layer = Image.alpha_composite(layer, outline(ring_and_tower, 11))
    layer = Image.alpha_composite(layer, brass(ring, 18))
    layer = Image.alpha_composite(layer, brass(tower, 14, dark=True))
    # Rivets on the ring.
    rivets = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(rivets)
    for k in range(14):
        a = 2 * math.pi * (k + 0.5) / 14 - math.pi / 2
        r = (COG_IN + COG_ROOT) / 2
        x, y = CX + math.cos(a) * r, CY + math.sin(a) * r
        draw.ellipse((x - 13, y - 13, x + 13, y + 13), fill=255)
    layer = Image.alpha_composite(layer, outline(rivets, 5))
    layer = Image.alpha_composite(layer, brass(rivets, 5))
    return layer


# --- The title ---


def title():
    font_path = next((f for f in FONTS if os.path.exists(f)), None)
    big = ImageFont.truetype(font_path, 330)
    small = ImageFont.truetype(font_path, 248)
    # (text, font) - the first letter of each word big, like an old poster.
    parts = [("L", big), ("AST", small), ("  ", small), ("B", big), ("ASTION", small)]
    spacing = 6
    width = sum(font.getlength(text) + spacing * len(text) for text, font in parts)
    x = (W - width) / 2
    base = 1160
    mask = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(mask)
    for text, font in parts:
        for ch in text:
            draw.text((x, base), ch, font=font, fill=255, anchor="ls")
            x += font.getlength(ch) + spacing
    left, right = (W - width) / 2, (W + width) / 2
    # A rule under it, with a small diamond at each end.
    bar = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(bar)
    draw.rectangle((left + 40, base + 30, right - 40, base + 46), fill=255)
    for x in (left + 30, right - 30):
        draw.polygon([(x - 30, base + 38), (x, base + 14), (x + 30, base + 38), (x, base + 62)], fill=255)
    letters = ImageChops.lighter(mask, bar)

    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    layer = Image.alpha_composite(layer, shadow(letters, 18, (10, 16), 0.9))
    layer = Image.alpha_composite(layer, outline(letters, 15))
    metal = brass(letters, 9)
    # Blood: a few splashes, only where there's metal.
    blood = Image.new("L", (W, H), 0)
    draw = ImageDraw.Draw(blood)
    for cx, cy in [(left + width * 0.08, base - 60), (left + width * 0.52, base - 150), (left + width * 0.9, base - 40)]:
        for _ in range(26):
            r = rng.uniform(4, 34)
            dx, dy = rng.gauss(0, 55), rng.gauss(0, 45)
            draw.ellipse((cx + dx - r, cy + dy - r, cx + dx + r, cy + dy + r), fill=rng.randint(120, 220))
    blood = ImageChops.multiply(blood.filter(ImageFilter.GaussianBlur(3)), letters)
    red = Image.new("RGBA", (W, H), (120, 10, 8, 0))
    red.putalpha(blood)
    metal = Image.alpha_composite(metal, red)
    return Image.alpha_composite(layer, metal)


def main():
    logo = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    for part in (banners(), emblem(), title()):
        logo = Image.alpha_composite(logo, part)
    logo = logo.crop(logo.getbbox())
    logo = logo.resize((logo.width // 2, logo.height // 2), Image.LANCZOS)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    logo.save(OUT, optimize=True)
    print(f"{OUT}: {logo.width} x {logo.height}")


if __name__ == "__main__":
    main()
