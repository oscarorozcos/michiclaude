#!/usr/bin/env python3
"""Dibuja el arte del instalador (NSIS) — huella en estilo cómic.

NSIS solo acepta BMP de 24 bits en sus dos huecos de imagen, y con tamaños
FIJOS: la cabecera de 150x57 y la barra lateral de 164x314. Por eso el arte
se dibuja aquí en vez de exportarse de un editor: así el instalador se
regenera solo cuando cambie la marca, sin depender de un .psd que nadie
tiene, y sin sumar dependencias al proyecto (solo la stdlib — el resto del
repo tampoco tiene librerías de imagen, invariante #4).

    python3 scripts/make-installer-art.py

Escribe src-tauri/installer/{header,sidebar}.bmp, que tauri.conf.json
referencia. Antialias por supermuestreo x3: NSIS no sabe suavizar y los
bordes de la huella se verían de escalera.
"""
import struct
from pathlib import Path

SS = 3  # supermuestreo

# Paleta única del instalador (la del panel: azul-violeta de la maqueta
# cómic de Oscar). Un solo esquema a propósito — sus dos maquetas usaban
# colores distintos y mezclarlas partiría la marca en dos.
INK = (0x14, 0x10, 0x3A)  # contorno
BLUE = (0x25, 0x15, 0x90)  # fondo
DOT = (0x3A, 0x2A, 0xB0)  # lunares del fondo
PAW = (0xFC, 0xFB, 0xFF)  # la huella


def paw_hits(x, y, cx, cy, scale):
    """¿Cae el punto (x,y) dentro de la huella? Coordenadas del SVG del
    panel (viewBox 64x64) para que el dibujo sea EL MISMO en los dos
    sitios: si un día cambia la huella, cambia en ambos o se nota."""
    u, v = (x - cx) / scale + 32, (y - cy) / scale + 32
    toes = ((14, 30, 6.5, 8.5, -18), (27, 20, 7, 9, -7),
            (40, 20, 7, 9, 7), (53, 30, 6.5, 8.5, 18))
    for tx, ty, rx, ry, rot in toes:
        import math
        a = math.radians(-rot)
        dx, dy = u - tx, v - ty
        px = dx * math.cos(a) - dy * math.sin(a)
        py = dx * math.sin(a) + dy * math.cos(a)
        if (px / rx) ** 2 + (py / ry) ** 2 <= 1:
            return True
    # almohadilla: la elipse que aproxima el path del SVG
    return ((u - 33) / 17.0) ** 2 + ((v - 45.5) / 12.5) ** 2 <= 1


def render(w, h, cx, cy, scale, dots=True, right_border=False):
    """Devuelve una lista de filas de píxeles (arriba->abajo)."""
    rows = []
    for py in range(h):
        row = []
        for px in range(w):
            acc = [0, 0, 0]
            for sy in range(SS):
                for sx in range(SS):
                    x = px + (sx + 0.5) / SS
                    y = py + (sy + 0.5) / SS
                    if right_border and x >= w - 4:
                        c = INK
                    elif paw_hits(x, y, cx, cy, scale):
                        c = PAW
                    elif dots and ((int(x) // 8 + int(y) // 8) % 2 == 0
                                   and (x % 8 - 4) ** 2 + (y % 8 - 4) ** 2 <= 2.2):
                        c = DOT
                    else:
                        c = BLUE
                    acc[0] += c[0]; acc[1] += c[1]; acc[2] += c[2]
            n = SS * SS
            row.append((acc[0] // n, acc[1] // n, acc[2] // n))
        rows.append(row)
    return rows


def write_bmp(path, rows):
    """BMP de 24 bits, sin compresión: lo que NSIS entiende. Las filas van
    de abajo hacia arriba y cada una se rellena a múltiplo de 4 bytes."""
    h, w = len(rows), len(rows[0])
    pad = (-w * 3) % 4
    px = b"".join(
        b"".join(struct.pack("BBB", b, g, r) for r, g, b in row) + b"\0" * pad
        for row in reversed(rows))
    head = struct.pack("<2sIHHI", b"BM", 14 + 40 + len(px), 0, 0, 14 + 40)
    info = struct.pack("<IiiHHIIiiII", 40, w, h, 1, 24, 0, len(px), 2835, 2835, 0, 0)
    Path(path).write_bytes(head + info + px)
    print(f"{path}  {w}x{h}  {len(head + info + px) // 1024} KB")


if __name__ == "__main__":
    out = Path(__file__).resolve().parent.parent / "src-tauri" / "installer"
    out.mkdir(parents=True, exist_ok=True)
    # cabecera: la huella a la DERECHA — MUI2 escribe el título de la
    # página encima, a la izquierda, y taparlo dejaría el texto ilegible
    write_bmp(out / "header.bmp", render(150, 57, cx=118, cy=28, scale=0.62))
    # lateral (bienvenida y final): huella grande, centrada y algo alta
    write_bmp(out / "sidebar.bmp",
              render(164, 314, cx=82, cy=130, scale=1.75, right_border=True))
