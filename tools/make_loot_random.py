"""Zeichnet das Symbol für "Loot (random)" im Editor-Katalog: ein Ausrufezeichen und rechts daneben ein Fragezeichen,
in Gold mit dunklem Rand – wie TABs Porträts: 200 x 200, durchsichtig. Ergebnis: assets\\ui\\portraits\\LootRandom.png

Aufruf (aus dem Projektordner):  python tools\\make_loot_random.py
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

SIZE = 200
GOLD = (232, 184, 48, 255)
LIGHT = (255, 226, 120, 255)
EDGE = (24, 18, 8, 255)
FONT = Path(r"C:\Windows\Fonts\ariblk.ttf")  # Arial Black


def main() -> None:
    font = ImageFont.truetype(str(FONT), 128)
    text = "!?"
    box = font.getbbox(text, stroke_width=7)
    width, height = box[2] - box[0], box[3] - box[1]
    at = ((SIZE - width) // 2 - box[0], (SIZE - height) // 2 - box[1])
    image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    # Rand, Gold – und oben eine hellere Kante, damit es nicht flach wirkt.
    draw.text(at, text, font=font, fill=GOLD, stroke_width=7, stroke_fill=EDGE)
    light = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    ImageDraw.Draw(light).text((at[0], at[1] - 3), text, font=font, fill=LIGHT)
    mask = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(mask).text(at, text, font=font, fill=255)
    lower = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(lower).text((at[0], at[1] + 4), text, font=font, fill=255)
    # Nur der Streifen oben, den die verschobene untere Schrift nicht deckt.
    strip = Image.composite(Image.new("L", (SIZE, SIZE), 0), mask, lower)
    image.paste(light, (0, 0), strip)
    out = Path(__file__).resolve().parent.parent / "assets" / "ui" / "portraits" / "LootRandom.png"
    image.save(out)
    print("geschrieben:", out)


if __name__ == "__main__":
    main()
