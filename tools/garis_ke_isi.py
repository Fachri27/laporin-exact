"""Ubah ikon bergaris (stroke 2, ujung bulat) menjadi satu <path> terisi untuk Exact di Android.

Setiap ruas garis menjadi persegi panjang selebar stroke, setiap titik sambung dan ujung menjadi
lingkaran (poligon 14 sisi). Semua subpath dibuat searah jarum jam, jadi fill-rule nonzero
menyatukannya tanpa lubang. Kurva (A, C, S) dan <circle> diambil sampelnya menjadi polyline.

Pakai: python3 -I garis_ke_isi.py <svg masuk> <svg keluar>
"""
import math
import re
import sys

LEBAR = 2.0
SISI_BULAT = 14


def token(d):
    return re.findall(r"[MmLlHhVvAaCcSsZz]|-?(?:\d+\.?\d*|\.\d+)(?:e-?\d+)?", d)


def arc_titik(x1, y1, rx, ry, phi, fa, fs, x2, y2, n=24):
    """Konversi busur SVG (parameter titik akhir) menjadi titik-titik (SVG 1.1 lampiran F.6)."""
    if rx == 0 or ry == 0:
        return [(x2, y2)]
    rx, ry = abs(rx), abs(ry)
    cp, sp = math.cos(math.radians(phi)), math.sin(math.radians(phi))
    dx, dy = (x1 - x2) / 2, (y1 - y2) / 2
    x1p, y1p = cp * dx + sp * dy, -sp * dx + cp * dy
    lam = (x1p ** 2) / (rx ** 2) + (y1p ** 2) / (ry ** 2)
    if lam > 1:
        rx, ry = rx * math.sqrt(lam), ry * math.sqrt(lam)
    num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
    den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    co = math.sqrt(max(0.0, num / den)) if den else 0.0
    if fa == fs:
        co = -co
    cxp, cyp = co * rx * y1p / ry, -co * ry * x1p / rx
    cx = cp * cxp - sp * cyp + (x1 + x2) / 2
    cy = sp * cxp + cp * cyp + (y1 + y2) / 2

    def sudut(ux, uy, vx, vy):
        a = math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)
        return a

    t1 = sudut(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    dt = sudut((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if not fs and dt > 0:
        dt -= 2 * math.pi
    elif fs and dt < 0:
        dt += 2 * math.pi
    langkah = max(4, int(abs(dt) / (2 * math.pi) * 48))
    out = []
    for i in range(1, langkah + 1):
        t = t1 + dt * i / langkah
        x = cp * rx * math.cos(t) - sp * ry * math.sin(t) + cx
        y = sp * rx * math.cos(t) + cp * ry * math.sin(t) + cy
        out.append((x, y))
    return out


def kubik(p0, p1, p2, p3, n=16):
    out = []
    for i in range(1, n + 1):
        t = i / n
        u = 1 - t
        x = u ** 3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t ** 3 * p3[0]
        y = u ** 3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t ** 3 * p3[1]
        out.append((x, y))
    return out


def path_ke_garis(d):
    """Path SVG → daftar (titik-titik, tertutup)."""
    t = token(d)
    i, cmd = 0, None
    x = y = sx = sy = 0.0
    kontrol = None
    hasil, kini = [], []

    def angka():
        nonlocal i
        v = float(t[i])
        i += 1
        return v

    def tutup_subpath(tertutup):
        nonlocal kini
        if kini:
            hasil.append((kini, tertutup))
        kini = []

    while i < len(t):
        if re.match(r"[A-Za-z]", t[i]):
            cmd = t[i]
            i += 1
            if cmd in "Zz":
                tutup_subpath(True)
                x, y = sx, sy
                kontrol = None
                continue
        rel = cmd.islower()
        c = cmd.upper()
        if c == "M":
            tutup_subpath(False)
            nx, ny = angka(), angka()
            x, y = (x + nx, y + ny) if rel else (nx, ny)
            sx, sy = x, y
            kini = [(x, y)]
            cmd = "l" if rel else "L"
            kontrol = None
        elif c == "L":
            nx, ny = angka(), angka()
            x, y = (x + nx, y + ny) if rel else (nx, ny)
            kini.append((x, y))
            kontrol = None
        elif c == "H":
            nx = angka()
            x = x + nx if rel else nx
            kini.append((x, y))
            kontrol = None
        elif c == "V":
            ny = angka()
            y = y + ny if rel else ny
            kini.append((x, y))
            kontrol = None
        elif c == "A":
            rx, ry, phi, fa, fs, nx, ny = (angka() for _ in range(7))
            ex, ey = (x + nx, y + ny) if rel else (nx, ny)
            kini.extend(arc_titik(x, y, rx, ry, phi, int(fa), int(fs), ex, ey))
            x, y = ex, ey
            kontrol = None
        elif c == "C":
            v = [angka() for _ in range(6)]
            if rel:
                v = [v[0] + x, v[1] + y, v[2] + x, v[3] + y, v[4] + x, v[5] + y]
            kini.extend(kubik((x, y), (v[0], v[1]), (v[2], v[3]), (v[4], v[5])))
            kontrol = (v[2], v[3])
            x, y = v[4], v[5]
        elif c == "S":
            v = [angka() for _ in range(4)]
            if rel:
                v = [v[0] + x, v[1] + y, v[2] + x, v[3] + y]
            c1 = (2 * x - kontrol[0], 2 * y - kontrol[1]) if kontrol else (x, y)
            kini.extend(kubik((x, y), c1, (v[0], v[1]), (v[2], v[3])))
            kontrol = (v[0], v[1])
            x, y = v[2], v[3]
        else:
            raise ValueError(f"perintah path tidak didukung: {cmd}")
    tutup_subpath(False)
    return hasil


def lingkaran(cx, cy, r, n):
    return [(cx + r * math.cos(2 * math.pi * k / n), cy + r * math.sin(2 * math.pi * k / n)) for k in range(n)]


def searah(poli):
    luas = sum(poli[k][0] * poli[(k + 1) % len(poli)][1] - poli[(k + 1) % len(poli)][0] * poli[k][1] for k in range(len(poli)))
    return poli if luas >= 0 else poli[::-1]


def garis_ke_poligon(titik, tertutup):
    h = LEBAR / 2
    poli = []
    pts = titik + ([titik[0]] if tertutup else [])
    for a, b in zip(pts, pts[1:]):
        dx, dy = b[0] - a[0], b[1] - a[1]
        p = math.hypot(dx, dy)
        if p < 1e-9:
            continue
        nx, ny = -dy / p * h, dx / p * h
        poli.append(searah([(a[0] + nx, a[1] + ny), (b[0] + nx, b[1] + ny), (b[0] - nx, b[1] - ny), (a[0] - nx, a[1] - ny)]))
    for q in titik:
        poli.append(searah(lingkaran(q[0], q[1], h, SISI_BULAT)))
    return poli


def ke_d(poli):
    f = lambda v: f"{v:.2f}".rstrip("0").rstrip(".")
    return "".join("M" + "L".join(f"{f(x)} {f(y)}" for x, y in p) + "Z" for p in poli)


def main(masuk, keluar):
    svg = open(masuk, encoding="utf-8").read()
    poli = []
    for d in re.findall(r'<path[^>]*\sd="([^"]+)"', svg):
        for titik, tertutup in path_ke_garis(d):
            poli.extend(garis_ke_poligon(titik, tertutup))
    for cx, cy, r in re.findall(r'<circle[^>]*cx="([\d.]+)"[^>]*cy="([\d.]+)"[^>]*r="([\d.]+)"', svg):
        poli.extend(garis_ke_poligon(lingkaran(float(cx), float(cy), float(r), 32), True))
    with open(keluar, "w", encoding="utf-8") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">\n  <path fill="#000000" fill-rule="nonzero" d="{ke_d(poli)}"/>\n</svg>\n')
    print(f"{keluar}: {len(poli)} subpath")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
