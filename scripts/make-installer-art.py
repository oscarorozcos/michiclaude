#!/usr/bin/env python3
"""Dibuja la marca: el icono de la app y el arte del instalador (NSIS).

Una sola huella, un solo par de colores, tres destinos:

    app-icon.png                    1024x1024 PNG con esquinas redondeadas
    src-tauri/installer/header.bmp  150x57  BMP 24 bits
    src-tauri/installer/sidebar.bmp 164x314 BMP 24 bits

NSIS solo acepta BMP de 24 bits y con esos tamaños EXACTOS; el icono de la
app lo pide Tauri en PNG cuadrado. Por eso el arte se dibuja aquí en vez de
exportarse de un editor: se regenera solo cuando cambie la marca, sin
depender de un archivo que vive en la máquina de alguien, y siempre con las
medidas correctas. Solo stdlib (zlib y struct vienen con Python): el
proyecto no gana dependencias (invariante #4).

    python3 scripts/make-installer-art.py
    npm run icons     # convierte app-icon.png al juego de iconos (.ico…)

Antialias por supermuestreo x3: ni NSIS ni el .ico suavizan bordes, y sin
esto la huella se ve de escalera.
"""
import math
import struct
import zlib
from pathlib import Path

SS = 3  # supermuestreo

# Los colores SON los del panel (index.html): --card para el fondo y
# --brand para la huella. Si un día cambia la paleta de la app, esto se
# actualiza a mano y el icono vuelve a casar con la UI.
BG = (0x15, 0x1F, 0x3A)     # --card, azul oscuro
PAWC = (0xE0, 0x8B, 0x63)   # --brand, naranja


def paw_hits(x, y, cx, cy, scale):
    """¿Cae (x,y) dentro de la huella? Coordenadas del SVG del panel
    (viewBox 64x64) para que el dibujo sea EL MISMO en todos los sitios:
    si cambia la huella, cambia en todos o se nota."""
    u, v = (x - cx) / scale + 32, (y - cy) / scale + 32
    for tx, ty, rx, ry, rot in ((14, 30, 6.5, 8.5, -18), (27, 20, 7, 9, -7),
                                (40, 20, 7, 9, 7), (53, 30, 6.5, 8.5, 18)):
        a = math.radians(-rot)
        dx, dy = u - tx, v - ty
        px = dx * math.cos(a) - dy * math.sin(a)
        py = dx * math.sin(a) + dy * math.cos(a)
        if (px / rx) ** 2 + (py / ry) ** 2 <= 1:
            return True
    # almohadilla: elipse que aproxima el path del SVG
    return ((u - 33) / 17.0) ** 2 + ((v - 45.5) / 12.5) ** 2 <= 1


def in_round_rect(x, y, w, h, r):
    """Cuadrado de esquinas redondeadas, como el avatar del panel."""
    cx = min(max(x, r), w - r)
    cy = min(max(y, r), h - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def render(w, h, cx, cy, scale, radius=None):
    """Filas de píxeles (arriba->abajo). Con `radius` devuelve RGBA y las
    esquinas quedan TRANSPARENTES; sin él, RGB opaco."""
    rows = []
    for py in range(h):
        row = []
        for px in range(w):
            acc = [0, 0, 0, 0]
            for sy in range(SS):
                for sx in range(SS):
                    x, y = px + (sx + .5) / SS, py + (sy + .5) / SS
                    if radius is not None and not in_round_rect(x, y, w, h, radius):
                        c = (0, 0, 0, 0)          # fuera de la pastilla
                    elif paw_hits(x, y, cx, cy, scale):
                        c = PAWC + (255,)
                    else:
                        c = BG + (255,)
                    for i in range(4):
                        acc[i] += c[i]
            n = SS * SS
            # el color se premultiplica al promediar: sin esto el borde de
            # la pastilla tira a negro sobre fondos claros
            a = acc[3] // n
            row.append(tuple(acc[i] // n for i in range(3)) + (a,))
        rows.append(row)
    return rows


def write_bmp(path, rows):
    """BMP de 24 bits sin comprimir: lo que NSIS entiende. Las filas van de
    abajo arriba y cada una se rellena a múltiplo de 4 bytes."""
    h, w = len(rows), len(rows[0])
    pad = (-w * 3) % 4
    px = b"".join(b"".join(struct.pack("BBB", p[2], p[1], p[0]) for p in row)
                  + b"\0" * pad for row in reversed(rows))
    head = struct.pack("<2sIHHI", b"BM", 14 + 40 + len(px), 0, 0, 14 + 40)
    info = struct.pack("<IiiHHIIiiII", 40, w, h, 1, 24, 0, len(px),
                       2835, 2835, 0, 0)
    Path(path).write_bytes(head + info + px)
    print(f"{path}  {w}x{h}")


def write_png(path, rows):
    """PNG RGBA de 8 bits, sin filtros (tipo 0 por fila)."""
    h, w = len(rows), len(rows[0])
    raw = b"".join(b"\0" + bytes(v for p in row for v in p) for row in rows)

    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c))

    Path(path).write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b""))
    print(f"{path}  {w}x{h}")


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    out = root / "src-tauri" / "installer"
    out.mkdir(parents=True, exist_ok=True)

    # Icono de la app: pastilla redondeada como el avatar del panel. El
    # radio (22%) es el de Windows 11 y el que usan las tiendas.
    write_png(root / "app-icon.png",
              render(1024, 1024, cx=512, cy=520, scale=11.0, radius=225))
    # Cabecera: la huella a la DERECHA — MUI2 escribe el título de la
    # página encima, a la izquierda, y taparlo dejaría el texto ilegible.
    write_bmp(out / "header.bmp", render(150, 57, cx=118, cy=28, scale=.62))
    # Lateral (bienvenida y final): huella grande, centrada y algo alta.
    write_bmp(out / "sidebar.bmp", render(164, 314, cx=82, cy=130, scale=1.75))
