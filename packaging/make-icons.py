#!/usr/bin/env python3
"""Erzeugt die Programmsymbole aus app/assets/icon.png (1024 px, Vorlage):

- app/assets/icon-256.png          Fenster- und Taskleistensymbol (main.rs)
- app/assets/logo-64.png           Logo oben links in der App (32 px, doppelt für scharfe Bildschirme)
- app/assets/windows/klassenwerk.ico  Symbol der .exe und des Installers

Ab 48 px wird die Vorlage verkleinert (K, Zeilen, Haken). Für 16–32 px wird
das Symbol eigens gezeichnet: nur das K, größer und pixelgenau, denn Zeilen
und Haken wären dort nur noch ein grauer Fleck. Diese kleine Form zeigt auch
die App neben „Klassenwerk“, so dass App, Titelleiste und Taskleiste gleich
aussehen.

Braucht Pillow und die Schrift Liberation Sans Bold (Linux: fonts-liberation)
oder Arial Bold. Danach packaging/windows/make-resources.sh ausführen.
"""
import os
import sys
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
A = os.path.join(ROOT, "app", "assets")
PURPLE = (79, 70, 229, 255)
FONTS = [
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "C:/Windows/Fonts/arialbd.ttf",
    "/Library/Fonts/Arial Bold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
]


def font(px):
    for f in FONTS:
        if os.path.exists(f):
            return ImageFont.truetype(f, px)
    sys.exit("Schrift Liberation Sans Bold / Arial Bold nicht gefunden")


def small(size):
    """Nur das K auf dem lila Feld, für 16–64 px."""
    pad = max(1, round(size * 0.0625)) if size > 16 else 1
    inner = size - 2 * pad
    # Rundung und Kanten 4-fach berechnen, dann verkleinern
    ss = 4
    bg = Image.new("RGBA", (size * ss, size * ss), (0, 0, 0, 0))
    ImageDraw.Draw(bg).rounded_rectangle(
        [pad * ss, pad * ss, (size - pad) * ss - 1, (size - pad) * ss - 1], radius=round(inner * 0.22 * ss), fill=PURPLE
    )
    im = bg.resize((size, size), Image.LANCZOS)
    # das K direkt in Zielgröße setzen: die Schrift wird dabei auf Pixel ausgerichtet
    f = font(round(inner * 0.86))
    d = ImageDraw.Draw(im)
    l, t, r, b = d.textbbox((0, 0), "K", font=f)
    x = round((size - (r - l)) / 2 - l)
    y = round((size - (b - t)) / 2 - t)
    d.text((x, y), "K", font=f, fill=(255, 255, 255, 255))
    return im


def main():
    master = Image.open(os.path.join(A, "icon.png")).convert("RGBA")
    big = lambda s: master.resize((s, s), Image.LANCZOS)
    big(256).save(os.path.join(A, "icon-256.png"), optimize=True)
    small(64).save(os.path.join(A, "logo-64.png"), optimize=True)
    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    imgs = [small(s) if s <= 32 else big(s) for s in sizes]
    # ICO: jede Größe als eigenes Bild (Pillow übernimmt die mitgegebenen Bilder)
    imgs[-1].save(
        os.path.join(A, "windows", "klassenwerk.ico"),
        format="ICO",
        sizes=[(s, s) for s in sizes],
        append_images=imgs[:-1],
    )
    print("Symbole erzeugt:", ", ".join(f"{s}px" for s in sizes))


if __name__ == "__main__":
    main()
