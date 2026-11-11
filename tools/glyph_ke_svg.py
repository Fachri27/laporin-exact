"""Ambil satu glyph font ikon (mis. Feather.ttf dari @expo/vector-icons laporin-apps) sebagai <path> SVG terisi 24×24.

Glyph font sudah terisi, jadi bentuknya persis ikon aslinya (tanpa garis_ke_isi.py); kurva kuadrat
ditulis sebagai kubik. Kode glyph dari glyphmaps/Feather.json. Geser y menengahkan glyph (Feather: 0.42).

Pakai: uv run --no-project --with fonttools python -I glyph_ke_svg.py <font.ttf> <kode> <svg keluar> [geser y]
"""
import sys
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
font = TTFont(sys.argv[1]); cp = int(sys.argv[2])
name = font.getBestCmap()[cp]; gs = font.getGlyphSet(); upm = font['head'].unitsPerEm
asc = font['hhea'].ascent; desc = font['hhea'].descent
class KubikPen(SVGPathPen):
    # Kurva kuadrat glyph TrueType ditulis sebagai kubik setara (C), bentuk yang sudah terbukti di ikon logo.
    def _qCurveToOne(self, q, p):
        p0 = self._getCurrentPoint()
        c1 = (p0[0] + 2 / 3 * (q[0] - p0[0]), p0[1] + 2 / 3 * (q[1] - p0[1]))
        c2 = (p[0] + 2 / 3 * (q[0] - p[0]), p[1] + 2 / 3 * (q[1] - p[1]))
        self._curveToOne(c1, c2, p)
pen = KubikPen(gs, ntos=lambda v: f"{v:.2f}".rstrip('0').rstrip('.'))
# Satuan font → 24 px; sumbu y dibalik dari baseline+ascent.
k = 24 / upm
gs[name].draw(TransformPen(pen, (k, 0, 0, -k, 0, asc * k - float(sys.argv[4]) if len(sys.argv) > 4 else asc * k)))
print(name, upm, asc, desc, gs[name].width, file=sys.stderr)
open(sys.argv[3], 'w').write(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">\n  <path fill="#000000" fill-rule="nonzero" d="{pen.getCommands()}"/>\n</svg>\n')
