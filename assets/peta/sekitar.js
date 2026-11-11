'use strict';
/*
 * Logika sekitar.html; protokol Contract ↔ halaman ada di kepala halaman itu. Padanan laporin-apps:
 *   lib/kisi-peta.ts, lib/hotspot.ts                 → "Kisi & hotspot murni"
 *   lib/asap-gl.ts                                   → buatLapisanAsap
 *   lib/hotspot-gl.ts                                → "Lapisan hotspot"
 *   lib/sebaran-asap.ts                              → "Data asap" (berkas → IndexedDB, bila boleh)
 *   lib/hotspot-muat.ts                              → "Data hotspot"
 *   components/peta-gl.tsx                           → "Peta", "Penanda laporan", "Animasi asap"
 *   components/bilah-waktu.tsx                       → "Bilah waktu"
 *   components/peta-prakiraan.tsx, pesan-prakiraan   → "Mode Angin"
 *   components/peta-laporan.tsx                      → selaraskan(): mode, waktu, hari hotspot
 * AppState aplikasi = document.hidden di sini; saatJaringanPulih = peristiwa 'online'.
 */

const GEO_URL = 'https://geoplatform.nusantara.earth';
/** Citra satelit lewat tembolok geoplatform (app/api/citra), bukan langsung ke EOX. */
const URL_CITRA = `${GEO_URL}/api/citra/{z}/{y}/{x}`;
const URL_PRAKIRAAN = `${GEO_URL}/api/forecasting`;
const URL_COPERNICUS = 'https://atmosphere.copernicus.eu/';
const PUSAT = { lat: -2.5, lng: 118 };
/** Lebar kamera (derajat bujur): seluruh Nusantara, dan lokasiku. */
const LEBAR_NUSANTARA = 46;
const LEBAR_LOKASI = 1.2;
const ZOOM_MAKS = 16;
/** Zoom terdekat yang masih memecah penanda (kisi-peta.ts ZOOM_MAKS). */
const ZOOM_KISI = 15;
const UK_PIN = 40;
/** Opasitas penanda di balik bola dunia: 0, supaya pin sisi lain bumi tak tampak bergerak berlawanan. */
const BALIK_BOLA = 0;
/** Tinggi kartu yang muncul di tepi bawah (px, kira-kira): titik yang diketuk tak tertutup kartunya. */
const KARTU = { laporan: 110, hotspot: 180 };
/** Lama satu langkah animasi asap (detik), sama dengan situs. */
const LAMA_LANGKAH = { 1: 0.75, 2: 0.35 };
/** Mulai zoom ini titik hotspot bisa diketuk. */
const ZOOM_KETUK_HOTSPOT = 7;
/** Sasaran sentuh titik hotspot (px): jari, bukan tetikus situs. */
const JARI_HOTSPOT = 18;
/** Bilah waktu di atas kartu yang terbuka (padBawah). */
const SELA_BILAH = 12;
const TINTA = '#0C0E0D';

const API = `<svg width="17" height="17" viewBox="0 0 24 24" aria-hidden="true"><path fill="${TINTA}" d="M12 2c.6 3.6-1.4 5.6-2.9 7.6C7.6 11.5 6 13.4 6 16c0 3.4 2.7 6 6 6s6-2.6 6-6c0-2.4-1.1-4.3-2.4-5.7.1 1.8-.6 3.3-2 4 .5-3.7-.7-7.3-1.6-12.3z"/></svg>`;
/** Ikon Feather (garis 2, ujung bulat), seperti @expo/vector-icons. */
const feather = (uk, isi) => `<svg width="${uk}" height="${uk}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${isi}</svg>`;
const IKON_MAIN = feather(15, '<polygon points="5 3 19 12 5 21 5 3"/>');
const IKON_JEDA = feather(15, '<rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>');
const IKON_AWAN_MATI = feather(16, '<path d="M22.61 16.95A5 5 0 0 0 18 10h-1.26a8 8 0 0 0-7.05-6M5 5a8 8 0 0 0 4 15h9a5 5 0 0 0 1.7-.3"/><line x1="1" y1="1" x2="23" y2="23"/>');
const IKON_PUTARAN = '<svg class="putaran" viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-dasharray="34 60"/></svg>';

/* ——— Bahasa ———
 * isi.bahasa "id" | "en" (tak ada = "id"); berganti = semua teks diganti di tempat (terapkanBahasa), peta, asap,
 * hotspot, dan Windy tidak dimuat ulang. Teks "id" sama persis dengan sebelum ada terjemahan. Teks halaman di
 * sekitar.html bertanda data-teks (isi) / data-teks-label (aria-label) dengan kunci di sini.
 */
const TEKS = {
  id: {
    bulan: ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des'],
    // Linimasa & siklus model asap situs (lib/zarr-reader.ts) menyingkat Agustus "Agt".
    bulanAsap: ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agt', 'Sep', 'Okt', 'Nov', 'Des'],
    ribuan: '.',
    desimal: ',',
    laporan: 'Laporan',
    nLaporan: '{n} laporan',
    malam: '(malam)',
    jam24: '24 jam terakhir',
    memuatLinimasa: 'Memuat linimasa',
    tidakTersedia: 'Data tidak tersedia saat ini',
    gagalAsap: 'Gagal memuat sebaran asap',
    cobaLagi: 'Coba lagi',
    cobaLagiKapital: 'COBA LAGI',
    putar: 'Putar animasi',
    jeda: 'Jeda animasi',
    linimasa: 'Linimasa',
    prediksi: 'Prediksi',
    analisis: 'Analisis',
    memuatAngin: 'Memuat peta angin dan kualitas udara Windy…',
    galatAngin: 'Peta angin dan kualitas udara tidak termuat. Periksa koneksi lalu coba lagi.',
    judulAngin: 'Peta angin dan kualitas udara',
  },
  en: {
    bulan: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
    bulanAsap: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
    ribuan: ',',
    desimal: '.',
    laporan: 'Report',
    nLaporan: '{n} reports',
    malam: '(night)',
    jam24: 'last 24 hours',
    memuatLinimasa: 'Loading timeline',
    tidakTersedia: "Data isn't available right now",
    gagalAsap: "Couldn't load smoke data",
    cobaLagi: 'Try again',
    cobaLagiKapital: 'TRY AGAIN',
    putar: 'Play animation',
    jeda: 'Pause animation',
    linimasa: 'Timeline',
    prediksi: 'Forecast',
    analisis: 'Analysis',
    memuatAngin: 'Loading the Windy wind and air quality map…',
    galatAngin: "The wind and air quality map didn't load. Check your connection and try again.",
    judulAngin: 'Wind and air quality map',
  },
};
let bahasa = 'id';

/** Teks `kunci` dalam bahasa kini (kunci yang tak ada: Indonesia); {nama} diisi dari `isian`. */
function teks(kunci, isian) {
  const t = kunci in TEKS[bahasa] ? TEKS[bahasa][kunci] : TEKS.id[kunci];
  return isian ? t.replace(/\{(\w+)\}/g, (_, k) => String(isian[k])) : t;
}

/** Pemisah ribuan bahasa kini: "1234567" → "1.234.567" / "1,234,567". */
const ribuan = (bulat) => bulat.replace(/\B(?=(\d{3})+(?!\d))/g, teks('ribuan'));

function kirim(teks) {
  if (window.laporin) window.laporin.kirim(teks);
}

/** Zoom MapLibre (ubin 512 px) dari lebar kamera dalam derajat bujur. */
function zoomLebar(derajat) {
  return Math.log2((360 * window.innerWidth) / (512 * derajat));
}

function jarakKm(a, b) {
  const rad = (d) => (d * Math.PI) / 180;
  const dLat = rad(b.lat - a.lat);
  const dLng = rad(b.lng - a.lng);
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(rad(a.lat)) * Math.cos(rad(b.lat)) * Math.sin(dLng / 2) ** 2;
  return 2 * 6371 * Math.asin(Math.sqrt(h));
}

/** "< 1 km", "3,4 km" / "3.4 km", "1.234 km" / "1,234 km". */
function jarakTeks(km) {
  if (km < 1) return '< 1 km';
  if (km < 10) return `${km.toFixed(1).replace('.', teks('desimal'))} km`;
  return `${ribuan(String(Math.round(km)))} km`;
}

/**
 * fetch dengan batas waktu yang juga mencakup membaca isinya (seperti AbortSignal.timeout): di sinyal lemah koneksi
 * bisa tersambung tanpa data yang datang.
 */
async function ambil(url, ms, baca) {
  const c = new AbortController();
  const t = setTimeout(() => c.abort(), ms);
  try {
    const r = await fetch(url, { signal: c.signal, headers: { Accept: '*/*' } });
    return await baca(r);
  } finally {
    clearTimeout(t);
  }
}

/* ——— Kisi & hotspot murni (lib/kisi-peta.ts, lib/hotspot.ts) ——— */

/** Tingkat zoom kasar dari lebar region (derajat bujur): naik satu tiap lebar peta menjadi separuhnya. */
function tingkatZoom(lebar) {
  if (!(lebar > 0)) return 0;
  return Math.min(Math.max(Math.floor(Math.log2(360 / lebar)), 0), ZOOM_KISI);
}

/** Satu penanda per sel kisi; pulau ikut kunci supaya penanda redup tidak bercampur. */
function kunciTitik(b, zoom) {
  const n = 2 ** zoom / 45;
  return `${b.pulau}|${Math.floor(b.lat * n)},${Math.floor(b.lng * n)}`;
}

function kelompokkan(semua, zoom) {
  const m = new Map();
  for (const b of semua) {
    const k = kunciTitik(b, zoom);
    const isi = m.get(k);
    if (isi) isi.push(b);
    else m.set(k, [b]);
  }
  return [...m.values()];
}

/** Tetap satu penanda di zoom terdekat: memperbesar peta tidak memisahkannya. */
function satuTitik(isi) {
  const k = kunciTitik(isi[0], ZOOM_KISI);
  return isi.every((b) => kunciTitik(b, ZOOM_KISI) === k);
}

/** WIB selalu UTC+7 (tanpa musim panas). */
const JAM_WIB = 7 * 3600 * 1000;
function hariWib(ms) {
  return new Date(ms + JAM_WIB).toISOString().slice(0, 10);
}
const duaAngka = (n) => String(n).padStart(2, '0');
/** "20 Okt, 13:50 WIB" / "20 Oct, 13:50 WIB" — waktu deteksi di kartu titik. */
function formatWib(ms) {
  const d = new Date(ms + JAM_WIB);
  return `${d.getUTCDate()} ${teks('bulan')[d.getUTCMonth()]}, ${duaAngka(d.getUTCHours())}:${duaAngka(d.getUTCMinutes())} WIB`;
}
/** Hari terbaru (`sejak`) = "24 jam terakhir" / "last 24 hours", lainnya "27 Okt 2026" / "27 Oct 2026". */
function labelHari(data) {
  if (data.sejak) return teks('jam24');
  const d = new Date(`${data.hari}T00:00:00Z`);
  if (Number.isNaN(d.getTime())) return data.hari;
  return `${d.getUTCDate()} ${teks('bulan')[d.getUTCMonth()]} ${d.getUTCFullYear()}`;
}
/** Bilangan tanpa Intl: id 1234567 → "1.234.567", 32.1 → "32,1"; en "1,234,567", "32.1". */
function angka(n) {
  const [bulat, pecahan] = String(n).split('.');
  return ribuan(bulat) + (pecahan ? `${teks('desimal')}${pecahan}` : '');
}
/** Warna per tingkat kepercayaan (0 rendah, 1 sedang, 2 tinggi). */
const WARNA_SIPONGI = ['#4caf50', '#fbc02d', '#f01e1e'];
/** Satelit per indeks `s`; `sisi` = bentuk penandanya: 5 segi lima, 6 segi enam, 0 bintang. */
const SATELIT_HOTSPOT = [
  { label: 'NASA-TERRA/AQUA', sensor: 'MODIS', sisi: 5 },
  { label: 'NASA-SNPP', sensor: 'VIIRS', sisi: 6 },
  { label: 'NASA-NOAA20', sensor: 'VIIRS', sisi: 0 },
];
/** Baris satelit yang menyala; kolom ke-10 = indeks titik di data (tetap walau sakelar lain diganti). */
function susunTitikTampil(data, satelit) {
  const hasil = [];
  for (let n = 0; n < data.titik.length; n++) {
    const t = data.titik[n];
    if (satelit[t[5]]) hasil.push([...t.slice(0, 9), n]);
  }
  return hasil;
}
const hitungan = new WeakMap();
/** Jumlah titik per satelit (semua kepercayaan), seperti lembar Lapisan. */
function jumlahPerSatelit(data) {
  let n = hitungan.get(data);
  if (!n) {
    n = [0, 0, 0];
    for (const t of data.titik) if (n[t[5]] !== undefined) n[t[5]]++;
    hitungan.set(data, n);
  }
  return n;
}

/* ——— Lapisan WebGL Aerosol Karhutla (lib/asap-gl.ts) ——— */

/* Grid global CAMS dari /api/sebaran-asap: 451 baris × 900 kolom, sel 0,4°, baris 0 = 90°LS, kolom 0 = 180°BB,
   byte = round(clamp(AOD/2, 0, 1) × 255). */
const GRID_KOLOM = 900;
const GRID_BARIS = 451;
const UKURAN_GRID = GRID_BARIS * GRID_KOLOM;

/**
 * Satu shader per proyeksi (datar/bola): MapLibre memberi prelude `projectTile` yang sesuai; masukannya koordinat
 * Mercator [0, 1]². v_depan > 0 = sisi bola yang menghadap kamera: sebagian GPU HP tak memotong sisi belakang,
 * jadi FS membuangnya sendiri.
 */
const VS = (s) => `#version 300 es
${s.vertexShaderPrelude}
${s.define}
uniform vec4 u_rect;
in vec2 a_pos;
out vec2 v_pos;
out float v_depan;
void main() {
  v_pos = u_rect.xy + a_pos * u_rect.zw;
  gl_Position = projectTile(v_pos);
#ifdef GLOBE
  vec3 bola = projectToSphere(v_pos);
  v_depan = dot(bola, u_projection_clipping_plane.xyz) + u_projection_clipping_plane.w;
#else
  v_depan = 1.0;
#endif
}
`;

// Rentang plot 0–1,8, opasitas 80 %, fade-in 25 % — kalibrasi Copernicus Fire Emissions Watch situs.
const FS = `#version 300 es
  precision highp float;
  uniform sampler2D u_texA;
  uniform sampler2D u_texB;
  uniform float u_mix;
  uniform int u_style; // 0: copernicus/fire_smoke, 1: sh_Oranges_aod, 2: sh_all_aod
  in vec2 v_pos;
  in float v_depan;
  out vec4 fragColor;

  const vec2 u_plotRange = vec2(0.00, 1.80);
  const float u_opacity = 0.80;
  const float u_fadeIn = 0.25;
  const float PI = 3.141592653589793;

  vec3 getFireSmokeColor(float norm) {
    vec3 c0 = vec3(1.000, 0.969, 0.953);
    vec3 c1 = vec3(0.992, 0.881, 0.869);
    vec3 c2 = vec3(0.989, 0.782, 0.763);
    vec3 c3 = vec3(0.981, 0.640, 0.715);
    vec3 c4 = vec3(0.971, 0.445, 0.645);
    vec3 c5 = vec3(0.887, 0.245, 0.600);
    vec3 c6 = vec3(0.724, 0.049, 0.516);
    vec3 c7 = vec3(0.569, 0.004, 0.479);
    vec3 c8 = vec3(0.407, 0.003, 0.448);
    vec3 c9 = vec3(0.286, 0.000, 0.416);
    if (norm < 0.12) return mix(c0, c1, norm / 0.12);
    else if (norm < 0.24) return mix(c1, c2, (norm - 0.12) / 0.12);
    else if (norm < 0.36) return mix(c2, c3, (norm - 0.24) / 0.12);
    else if (norm < 0.48) return mix(c3, c4, (norm - 0.36) / 0.12);
    else if (norm < 0.60) return mix(c4, c5, (norm - 0.48) / 0.12);
    else if (norm < 0.72) return mix(c5, c6, (norm - 0.60) / 0.12);
    else if (norm < 0.82) return mix(c6, c7, (norm - 0.72) / 0.10);
    else if (norm < 0.92) return mix(c7, c8, (norm - 0.82) / 0.10);
    else return mix(c8, c9, clamp((norm - 0.92) / 0.08, 0.0, 1.0));
  }

  vec3 getOrangesColor(float norm) {
    vec3 c0 = vec3(1.0, 0.95, 0.88);
    vec3 c1 = vec3(1.0, 0.65, 0.15);
    vec3 c2 = vec3(0.96, 0.49, 0.00);
    vec3 c3 = vec3(0.90, 0.32, 0.00);
    vec3 c4 = vec3(0.75, 0.21, 0.05);
    if (norm < 0.22) return mix(c0, c1, norm / 0.22);
    else if (norm < 0.45) return mix(c1, c2, (norm - 0.22) / 0.23);
    else if (norm < 0.70) return mix(c2, c3, (norm - 0.45) / 0.25);
    else return mix(c3, c4, clamp((norm - 0.70) / 0.30, 0.0, 1.0));
  }

  vec3 getSpectralColor(float norm) {
    vec3 c0 = vec3(0.0, 0.3, 1.0);
    vec3 c1 = vec3(0.0, 0.9, 0.9);
    vec3 c2 = vec3(0.1, 0.85, 0.1);
    vec3 c3 = vec3(1.0, 0.9, 0.0);
    vec3 c4 = vec3(1.0, 0.2, 0.0);
    vec3 c5 = vec3(0.5, 0.0, 0.1);
    if (norm < 0.18) return mix(c0, c1, norm / 0.18);
    else if (norm < 0.36) return mix(c1, c2, (norm - 0.18) / 0.18);
    else if (norm < 0.55) return mix(c2, c3, (norm - 0.36) / 0.19);
    else if (norm < 0.75) return mix(c3, c4, (norm - 0.55) / 0.20);
    else return mix(c4, c5, clamp((norm - 0.75) / 0.25, 0.0, 1.0));
  }

  void main() {
    if (v_depan < 0.0) discard;
    float mercX = fract(v_pos.x);
    float mercY = v_pos.y;
    if (mercY < 0.0 || mercY > 1.0) discard;
    float lat = degrees(2.0 * atan(exp(PI * (1.0 - 2.0 * mercY))) - PI * 0.5);
    // Tekstur global 900×451, seperti situs.
    vec2 uv = vec2(mercX, clamp((lat + 90.0) / 180.0, 0.0, 1.0));
    float aodA = texture(u_texA, uv).r * 2.0;
    float aodB = texture(u_texB, uv).r * 2.0;
    float aod = mix(aodA, aodB, u_mix);
    float norm = clamp((aod - u_plotRange.x) / (u_plotRange.y - u_plotRange.x), 0.0, 1.0);
    float alpha = 1.0;
    if (u_fadeIn > 0.0) alpha = smoothstep(0.0, u_fadeIn, norm);
    alpha *= u_opacity;
    if (alpha < 0.005) discard;
    vec3 rgb;
    if (u_style == 0) rgb = getFireSmokeColor(norm);
    else if (u_style == 1) rgb = getOrangesColor(norm);
    else rgb = getSpectralColor(norm);
    fragColor = vec4(rgb * alpha, alpha);
  }
`;

const KOSONG = new Uint8Array(UKURAN_GRID);
/**
 * Jala satuan PECAH×PECAH sel, direntang hanya ke bagian dunia yang tampak (persegiTampak): di bola tiap sel
 * tergambar sebagai bidang datar, jadi sel besar tenggelam di bawah permukaan dan asap tertinggal dari citra.
 */
const PECAH = 64;
const DUNIA = [0, 0, 1, 1];
const LINTANG_MAKS = 85.051129;

function mercatorY(lintang) {
  const phi = (Math.max(-LINTANG_MAKS, Math.min(LINTANG_MAKS, lintang)) * Math.PI) / 180;
  return 0.5 - Math.log(Math.tan(Math.PI / 4 + phi / 2)) / (2 * Math.PI);
}

/** Bagian dunia Mercator yang tampak plus 25 % tiap sisi; peta datar: seluruh dunia. */
function persegiTampak(map) {
  const pr = map.getProjection();
  if (!pr || pr.type !== 'globe') return DUNIA;
  const b = map.getBounds();
  let x0 = (b.getWest() + 180) / 360;
  let x1 = (b.getEast() + 180) / 360;
  if (x1 < x0) x1 += 1;
  let y0 = mercatorY(b.getNorth());
  let y1 = mercatorY(b.getSouth());
  const px = (x1 - x0) * 0.25;
  const py = (y1 - y0) * 0.25;
  x0 -= px;
  x1 += px;
  y0 = Math.max(0, y0 - py);
  y1 = Math.min(1, y1 + py);
  if (!(x1 - x0 < 1) || !(y1 > y0)) return DUNIA;
  return [x0, y0, x1 - x0, y1 - y0];
}

function jalaSatuan() {
  const v = new Float32Array(PECAH * PECAH * 12);
  for (let y = 0; y < PECAH; y++) {
    for (let x = 0; x < PECAH; x++) {
      const x0 = x / PECAH;
      const x1 = (x + 1) / PECAH;
      const y0 = y / PECAH;
      const y1 = (y + 1) / PECAH;
      v.set([x0, y0, x1, y0, x0, y1, x0, y1, x1, y0, x1, y1], (y * PECAH + x) * 12);
    }
  }
  return v;
}

const SERAGAM = ['u_texA', 'u_texB', 'u_mix', 'u_style', 'u_rect', 'u_projection_matrix', 'u_projection_fallback_matrix',
  'u_projection_tile_mercator_coords', 'u_projection_clipping_plane', 'u_projection_transition'];

/**
 * Lapisan kustom "smoke-layer" yang membaca `g` (frame A/B, mix, gaya, tampil) setiap kali digambar; pemiliknya
 * mengubah `g` lalu memanggil triggerRepaint(). Tekstur hanya diunggah ulang saat kunci frame berganti.
 */
function buatLapisanAsap(g) {
  /** Per variantName proyeksi; null = gagal dikompilasi (tidak dicoba ulang tiap frame). */
  const program = new Map();
  let peta = null;
  let jala = null;
  let texA = null;
  let texB = null;
  let isiA = '';
  let isiB = '';

  function unggah(gl, tex, data, unit) {
    gl.activeTexture(unit);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, GRID_KOLOM, GRID_BARIS, gl.LUMINANCE, gl.UNSIGNED_BYTE, data || KOSONG);
  }

  function siapkan(gl, s) {
    if (program.has(s.variantName)) return program.get(s.variantName);
    const shader = (jenis, sumber) => {
      const sh = gl.createShader(jenis);
      if (!sh) return null;
      gl.shaderSource(sh, sumber);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
        console.error('[asap-gl] shader:', gl.getShaderInfoLog(sh));
        gl.deleteShader(sh);
        return null;
      }
      return sh;
    };
    const vs = shader(gl.VERTEX_SHADER, VS(s));
    const fs = shader(gl.FRAGMENT_SHADER, FS);
    let hasil = null;
    const p = vs && fs ? gl.createProgram() : null;
    if (p) {
      gl.attachShader(p, vs);
      gl.attachShader(p, fs);
      gl.linkProgram(p);
      if (gl.getProgramParameter(p, gl.LINK_STATUS)) {
        const u = Object.fromEntries(SERAGAM.map((n) => [n, gl.getUniformLocation(p, n)]));
        hasil = { p, aPos: gl.getAttribLocation(p, 'a_pos'), u };
      } else {
        console.error('[asap-gl] program:', gl.getProgramInfoLog(p));
        gl.deleteProgram(p);
      }
    }
    if (vs) gl.deleteShader(vs);
    if (fs) gl.deleteShader(fs);
    program.set(s.variantName, hasil);
    return hasil;
  }

  return {
    id: 'smoke-layer',
    type: 'custom',
    renderingMode: '2d',

    onAdd(map, gl) {
      peta = map;
      // Satu dunia Mercator: peta tanpa salinan dunia (renderWorldCopies mati).
      jala = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, jala);
      gl.bufferData(gl.ARRAY_BUFFER, jalaSatuan(), gl.STATIC_DRAW);
      const tekstur = () => {
        const t = gl.createTexture();
        gl.bindTexture(gl.TEXTURE_2D, t);
        gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        // Filter bilinear perangkat keras, seperti situs: asap halus di semua zoom.
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE, GRID_KOLOM, GRID_BARIS, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, KOSONG);
        return t;
      };
      texA = tekstur();
      texB = tekstur();
      isiA = isiB = '';
    },

    render(gl, options) {
      // MapLibre 5 selalu memberi konteks WebGL2.
      if (!g.tampil || !jala || !texA || !texB) return;
      const pr = siapkan(gl, options.shaderData);
      const d = options.defaultProjectionData;
      if (!pr || pr.aPos < 0 || !d) return;
      if (isiA !== g.kunciA) {
        unggah(gl, texA, g.a, gl.TEXTURE0);
        isiA = g.kunciA;
      }
      if (isiB !== g.kunciB) {
        unggah(gl, texB, g.b, gl.TEXTURE1);
        isiB = g.kunciB;
      }
      const { u } = pr;
      gl.useProgram(pr.p);
      gl.bindVertexArray(null);
      gl.bindBuffer(gl.ARRAY_BUFFER, jala);
      gl.enableVertexAttribArray(pr.aPos);
      gl.vertexAttribPointer(pr.aPos, 2, gl.FLOAT, false, 0, 0);
      // Seragam projectTile dari MapLibre, datar maupun bola.
      gl.uniformMatrix4fv(u.u_projection_matrix, false, d.mainMatrix);
      gl.uniformMatrix4fv(u.u_projection_fallback_matrix, false, d.fallbackMatrix);
      gl.uniform4f(u.u_projection_tile_mercator_coords, ...d.tileMercatorCoords);
      gl.uniform4f(u.u_projection_clipping_plane, ...d.clippingPlane);
      gl.uniform1f(u.u_projection_transition, d.projectionTransition);
      gl.uniform4f(u.u_rect, ...(peta ? persegiTampak(peta) : DUNIA));
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, texA);
      gl.uniform1i(u.u_texA, 0);
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, texB);
      gl.uniform1i(u.u_texB, 1);
      gl.uniform1f(u.u_mix, g.mix);
      gl.uniform1i(u.u_style, g.gaya);
      gl.disable(gl.DEPTH_TEST);
      gl.disable(gl.CULL_FACE);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.drawArrays(gl.TRIANGLES, 0, PECAH * PECAH * 6);
      // Pulihkan keadaan GL supaya lapisan MapLibre sesudahnya tergambar benar.
      gl.disableVertexAttribArray(pr.aPos);
      gl.bindBuffer(gl.ARRAY_BUFFER, null);
      gl.useProgram(null);
    },

    onRemove(_map, gl) {
      if (texA) gl.deleteTexture(texA);
      if (texB) gl.deleteTexture(texB);
      if (jala) gl.deleteBuffer(jala);
      for (const pr of program.values()) if (pr) gl.deleteProgram(pr.p);
      program.clear();
      jala = texA = texB = null;
      peta = null;
    },
  };
}

/* ——— Lapisan hotspot (lib/hotspot-gl.ts) ——— */

const SUMBER_HOTSPOT = 'hotspot';
const LAPIS_HOTSPOT = 'hotspot-sipongi';
/** Ukuran ikon (px CSS) pada icon-size 1; digambar 2× untuk layar rapat. */
const UKURAN_IKON = 13;
const RASIO_IKON = 2;
const PUDAR_KELUAR = 180;
const PUDAR_MASUK = 420;

/** Jalur SVG bentuk penanda satelit `s`, berpusat di (c, c) berjari-jari r, ujung ke atas. */
function jalurBentuk(s, c, r) {
  const sisi = SATELIT_HOTSPOT[s] ? SATELIT_HOTSPOT[s].sisi : 5;
  const n = sisi || 10;
  const luar = sisi ? r : r * 1.12;
  const titik = [];
  for (let i = 0; i < n; i++) {
    const sudut = -Math.PI / 2 + (i * 2 * Math.PI) / n;
    const jari = !sisi && i % 2 ? luar * 0.45 : luar;
    titik.push(`${(c + jari * Math.cos(sudut)).toFixed(2)},${(c + jari * Math.sin(sudut)).toFixed(2)}`);
  }
  return `M${titik.join('L')}Z`;
}

function gambarIkon(s, k) {
  const sisi = UKURAN_IKON * RASIO_IKON;
  const kanvas = document.createElement('canvas');
  kanvas.width = kanvas.height = sisi;
  const ctx = kanvas.getContext('2d');
  if (!ctx) return null;
  const tepi = 1.1 * RASIO_IKON;
  const jalur = new Path2D(jalurBentuk(s, sisi / 2, sisi / 2 - tepi));
  ctx.fillStyle = WARNA_SIPONGI[k];
  ctx.fill(jalur);
  ctx.lineJoin = 'round';
  ctx.lineWidth = tepi;
  ctx.strokeStyle = 'rgba(0, 0, 0, 0.6)';
  ctx.stroke(jalur);
  return ctx.getImageData(0, 0, sisi, sisi);
}

function keGeoJson(titik) {
  return {
    type: 'FeatureCollection',
    features: titik.map(([lng, lat, k, , , s, , , , id], i) => ({
      type: 'Feature',
      geometry: { type: 'Point', coordinates: [lng, lat] },
      properties: { k, s, id: id === undefined ? i : id },
    })),
  };
}

/** Pasang sumber + lapisan hotspot (kosong) di atas lapisan yang sudah ada. */
function pasangLapisanHotspot(map) {
  SATELIT_HOTSPOT.forEach((_, s) => {
    for (let k = 0; k < 3; k++) {
      const id = `hotspot-${s}-${k}`;
      if (map.hasImage(id)) continue;
      const ikon = gambarIkon(s, k);
      if (ikon) map.addImage(id, ikon, { pixelRatio: RASIO_IKON });
    }
  });
  if (map.getSource(SUMBER_HOTSPOT)) return;
  map.addSource(SUMBER_HOTSPOT, { type: 'geojson', data: keGeoJson([]), promoteId: 'id' });
  map.addLayer({
    id: LAPIS_HOTSPOT,
    type: 'symbol',
    source: SUMBER_HOTSPOT,
    layout: {
      'icon-image': ['concat', 'hotspot-', ['to-string', ['get', 's']], '-', ['to-string', ['get', 'k']]],
      'icon-size': ['interpolate', ['linear'], ['zoom'], 3, 0.6, 6, 0.85, 9, 1.15, 12, 1.6],
      'icon-allow-overlap': true,
      'icon-ignore-placement': true,
      'symbol-sort-key': ['get', 'k'],
    },
    paint: { 'icon-opacity': 1 },
  });
}

let tundaPudar = 0;
/** Isi lapisan dengan baris susunTitikTampil; `pudar` (hari berganti) = titik lama memudar dulu. */
function setelDataHotspot(map, titik, pudar) {
  const sumber = map.getSource(SUMBER_HOTSPOT);
  if (!sumber) return;
  clearTimeout(tundaPudar);
  if (!pudar) {
    map.setPaintProperty(LAPIS_HOTSPOT, 'icon-opacity', 1);
    sumber.setData(keGeoJson(titik));
    return;
  }
  map.setPaintProperty(LAPIS_HOTSPOT, 'icon-opacity-transition', { duration: PUDAR_KELUAR, delay: 0 });
  map.setPaintProperty(LAPIS_HOTSPOT, 'icon-opacity', 0);
  tundaPudar = setTimeout(() => {
    if (!map.getSource(SUMBER_HOTSPOT)) return;
    sumber.setData(keGeoJson(titik));
    map.setPaintProperty(LAPIS_HOTSPOT, 'icon-opacity-transition', { duration: PUDAR_MASUK, delay: 0 });
    map.setPaintProperty(LAPIS_HOTSPOT, 'icon-opacity', 1);
  }, PUDAR_KELUAR);
}

/**
 * Indeks titik (kolom ke-10) dalam ±jari px dari titik layar `p`; bila beberapa, kepercayaan tertinggi lalu
 * terdekat. Dari proyeksi, seperti situs: queryRenderedFeatures lapisan simbol tak cocok letaknya dengan layar.
 */
function hotspotDiTitik(map, titik, p, jari) {
  let terbaik = -1;
  let kTerbaik = -1;
  let dTerbaik = Infinity;
  for (let i = 0; i < titik.length; i++) {
    const q = map.project([titik[i][0], titik[i][1]]);
    const dx = q.x - p.x;
    const dy = q.y - p.y;
    if (dx > jari || dx < -jari || dy > jari || dy < -jari) continue;
    const k = titik[i][2];
    const d = dx * dx + dy * dy;
    if (k > kTerbaik || (k === kTerbaik && d < dTerbaik)) {
      terbaik = i;
      kTerbaik = k;
      dTerbaik = d;
    }
  }
  return terbaik < 0 ? null : titik[terbaik][9] === undefined ? terbaik : titik[terbaik][9];
}

/* ——— Keadaan dari Contract ——— */

const kini = {
  mode: 'laporan',
  laporan: [],
  kunciLaporan: '[]',
  pilih: '',
  pulau: '',
  titik: true,
  asap: true,
  satelit: [false, false, false],
  angin: true,
  globe: false,
  gaya: 0,
  tema: 'light',
  lokasiku: null,
  padBawah: 0,
};
/** n perintah terakhir; undefined = belum ada terima (n pertama hanya dicatat). */
let perintahLalu;
/** Hari ini (WIB) saat halaman dibuka: hari terbaru diminta tanpa ?hari supaya dapat jendela 24 jam bergulir. */
const hariIni = hariWib(Date.now());
/** Naik tiap jaringan pulih (saatJaringanPulih aplikasi). */
let versiPulih = 0;

const asapTampil = () => kini.mode === 'asap' && kini.asap;
const adaSatelit = () => kini.satelit.some(Boolean);

/* ——— Data asap (lib/sebaran-asap.ts) ———
 * Linimasa 3-jam (7 hari ke belakang s.d. +72 jam) dari ?mode=metadata, grid global per langkah dari ?mode=frame.
 * Frame disimpan di IndexedDB menurut kuncinya (timeChunk_step_timeInner: isinya tak pernah berubah) bila
 * WebView mengizinkan, beserta linimasa terakhir yang terbukti bisa dimuat: tanpa sinyal lapisan tetap tampil.
 * Jangan pernah mengirim `_ts`/`force`: keduanya memaksa situs mengambil ulang dari ECMWF.
 */

/** Sinyal lemah bisa tersambung tanpa data yang datang. */
const BATAS_META_MS = 20000;
/** Frame ±45 KB terkompresi, tiga diunduh bersamaan. */
const BATAS_FRAME_MS = 30000;
/** Selaras cron warm-asap situs. */
const PERIKSA_ULANG_MS = 30 * 60000;
const UNDUH_SERENTAK = 3;
/** Sekian frame gagal tanpa satu pun berhasil = data asap sedang tidak tersedia (mis. ECMWF mati). */
const GAGAL_MAKS = 3;
/** Gagal karena jaringan: dicoba lagi sendiri selama lapisan menyala, makin jarang bila terus gagal. */
const COBA_PUTUS_MS = 30000;
const COBA_PUTUS_MAKS_MS = 10 * 60000;
const NAMA_META = 'metadata.json';

/**
 * status: diam | memuat | siap | galat. versi naik tiap linimasa dipasang atau Coba lagi. modelRunWib = teks server
 * (Indonesia, boleh kosong), modelRunIso = siklus yang sama untuk disusun di bahasa lain (siklusAsap).
 */
const asapData = { status: 'diam', langkah: [], modelRunWib: null, modelRunIso: '', versi: 0 };
let meta = null;
let metaTerbukti = null;
let berhasil = false;
let tandaGagal = '';
const sedangAsap = new Map();
const gagalAsap = new Set();
let asapAktif = false;
let diperiksa = 0;
let metaGagal = false;
let metaSedang = null;
let periksaan = 0;
let putus = false;
let putusBeruntun = 0;
let jadwalUlang = 0;

/** Simpanan berkas (pengganti Paths.cache/asap): IndexedDB, atau tidak ada sama sekali bila ditolak. */
const berkas = (() => {
  let db;
  function buka() {
    if (!db) {
      db = new Promise((ok) => {
        try {
          const r = indexedDB.open('laporin-asap', 1);
          r.onupgradeneeded = () => r.result.createObjectStore('berkas');
          r.onsuccess = () => ok(r.result);
          r.onerror = () => ok(null);
          r.onblocked = () => ok(null);
        } catch (e) {
          ok(null);
        }
      });
    }
    return db;
  }
  async function jalan(mode, kerja) {
    const d = await buka();
    if (!d) return undefined;
    return new Promise((ok) => {
      try {
        const t = d.transaction('berkas', mode);
        const q = kerja(t.objectStore('berkas'));
        t.oncomplete = () => ok(q && q.result);
        t.onerror = t.onabort = () => ok(undefined);
      } catch (e) {
        ok(undefined);
      }
    });
  }
  return {
    baca: (nama) => jalan('readonly', (s) => s.get(nama)),
    tulis: (nama, isi) => jalan('readwrite', (s) => s.put(isi, nama)),
    /** Buang semua kecuali `sisa` (frame di luar linimasa terbukti: siklus lama, riwayat lewat 7 hari). */
    async sisakan(sisa) {
      const semua = await jalan('readonly', (s) => s.getAllKeys());
      if (!semua) return;
      const buang = semua.filter((k) => !sisa.has(k));
      if (buang.length) await jalan('readwrite', (s) => buang.forEach((k) => s.delete(k)));
    },
  };
})();

function ubahAsap(sebagian) {
  Object.assign(asapData, sebagian);
  selaraskan();
}

function adaGagalAsap() {
  return metaGagal || gagalAsap.size > 0 || asapData.status === 'galat';
}

function jadwalkanUlangAsap() {
  if (jadwalUlang || !asapAktif) return;
  const jeda = Math.min(COBA_PUTUS_MS * 2 ** putusBeruntun, COBA_PUTUS_MAKS_MS);
  putusBeruntun++;
  jadwalUlang = setTimeout(() => {
    jadwalUlang = 0;
    // Tersembunyi: tidak mengunduh; kembali tampak yang menyusul.
    if (asapAktif && !document.hidden) cobaLagiAsap();
  }, jeda);
}

const bulatSah = (v) => Number.isInteger(v) && v >= 0 && v < 1e6;

/** "9 Nov, 19:00 WIB" seperti labelWib situs, dalam bahasa kini; null bila `iso` tak terbaca. */
function formatWibAsap(iso) {
  const ms = Date.parse(iso);
  if (!Number.isFinite(ms)) return null;
  const d = new Date(ms + JAM_WIB);
  return `${d.getUTCDate()} ${teks('bulanAsap')[d.getUTCMonth()]}, ${duaAngka(d.getUTCHours())}:${duaAngka(d.getUTCMinutes())} WIB`;
}

/** Label langkah linimasa: labelWib server/Windy apa adanya ("23 Okt, 01:00 WIB"); bahasa lain disusun dari iso-nya. */
function labelLangkah(l) {
  if (!l) return '';
  return (bahasa !== 'id' && formatWibAsap(l.iso)) || l.labelWib;
}

/** Siklus model untuk asap-info: teks server bila "id" (atau disusun dari ISO bila tak ada), selain itu disusun. */
function siklusAsap() {
  const disusun = asapData.modelRunIso ? formatWibAsap(asapData.modelRunIso) : null;
  return (bahasa === 'id' ? asapData.modelRunWib || disusun : disusun || asapData.modelRunWib) || '';
}

/** Metadata dari jaringan/simpanan: langkah yang bentuknya tidak dikenal dilewati. */
function bacaMeta(teks) {
  let data;
  try {
    data = JSON.parse(teks);
  } catch (e) {
    return null;
  }
  if (!data || !Array.isArray(data.timesteps)) return null;
  const langkah = [];
  for (const x of data.timesteps) {
    const { timeChunk, step, timeInner, labelWib, iso, waktu, adalahPrediksi } = x || {};
    if (!bulatSah(timeChunk) || !bulatSah(step) || (timeInner !== 0 && timeInner !== 1)) continue;
    if (typeof labelWib !== 'string' || typeof iso !== 'string' || typeof waktu !== 'number' || !Number.isFinite(waktu)) continue;
    langkah.push({
      labelWib,
      iso,
      waktu,
      adalahPrediksi: adalahPrediksi === true,
      kunci: `${timeChunk}_${step}_${timeInner}`,
      kueri: `timeChunk=${timeChunk}&step=${step}&timeInner=${timeInner}`,
    });
  }
  if (langkah.length === 0) return null;
  const run = typeof data.latestModelRunIso === 'string' ? data.latestModelRunIso : '';
  // Tanpa latestModelRunWib: siklusAsap menyusunnya dari run, dalam bahasa saat dikabarkan.
  const runWib = typeof data.latestModelRunWib === 'string' ? data.latestModelRunWib : '';
  return { langkah, runWib, runIso: run, tanda: `${run}|${langkah.map((l) => l.kunci).join(',')}`, teks };
}

function pasangMeta(m) {
  meta = m;
  berhasil = false;
  gagalAsap.clear();
  ubahAsap({
    langkah: m.langkah,
    modelRunWib: m.runWib,
    modelRunIso: m.runIso,
    status: asapData.status === 'siap' ? 'siap' : 'memuat',
    versi: asapData.versi + 1,
  });
}

function periksaMeta() {
  if (!metaSedang) {
    metaSedang = (async () => {
      diperiksa = Date.now();
      try {
        const m = await ambil(`${GEO_URL}/api/sebaran-asap?mode=metadata`, BATAS_META_MS, async (r) => (r.ok ? bacaMeta(await r.text()) : null));
        if (!m) throw new Error('metadata asap tidak terbaca');
        metaGagal = false;
        if (m.tanda !== (meta && meta.tanda) && m.tanda !== tandaGagal) pasangMeta(m);
        // Linimasa sama: frame yang tadi gagal (sinyal putus sesaat) dicoba lagi.
        else if (meta && m.tanda === meta.tanda && berhasil && gagalAsap.size > 0) {
          gagalAsap.clear();
          ubahAsap({ versi: asapData.versi + 1 });
        }
      } catch (e) {
        // Luring atau situs galat: linimasa yang sudah ada tetap dipakai.
        metaGagal = true;
        if (!meta) {
          ubahAsap({ status: 'galat' });
          jadwalkanUlangAsap();
        }
      } finally {
        metaSedang = null;
      }
    })();
  }
  return metaSedang;
}

function segarkanAsapBilaBasi() {
  if (!document.hidden && Date.now() - diperiksa > PERIKSA_ULANG_MS - 1000) periksaMeta();
}

/** Slot unduhan: paling banyak UNDUH_SERENTAK, yang terbaru dulu — frame tujuan geseran tidak antre di belakang. */
let unduhan = 0;
const menunggu = [];
function antre() {
  if (unduhan < UNDUH_SERENTAK) {
    unduhan++;
    return Promise.resolve();
  }
  return new Promise((r) => menunggu.push(r));
}
function lepas() {
  const berikut = menunggu.pop();
  if (berikut) berikut();
  else unduhan--;
}

function adaIsi(b) {
  for (let i = 0; i < b.length; i++) if (b[i] !== 0) return true;
  return false;
}

/** null = situs menjawab tapi frame-nya tidak ada; melempar = gagal karena jaringan. */
async function unduhGrid(l) {
  await antre();
  try {
    if (putus) throw new Error('jaringan putus');
    return await ambil(`${GEO_URL}/api/sebaran-asap?mode=frame&${l.kueri}`, BATAS_FRAME_MS, async (r) => {
      if (!r.ok) return null;
      const b = new Uint8Array(await r.arrayBuffer());
      // Situs bisa menjawab 200 berisi nol semua (frame di luar linimasa); grid global asli tak pernah nol semua.
      return b.length === UKURAN_GRID && adaIsi(b) ? b : null;
    });
  } finally {
    lepas();
  }
}

async function ambilGrid(l) {
  const nama = `${l.kunci}.bin`;
  const ada = await berkas.baca(nama);
  if (ada instanceof Uint8Array && ada.length === UKURAN_GRID) return ada;
  const g = await unduhGrid(l);
  if (g) berkas.tulis(nama, g);
  return g;
}

const milikMeta = (l) => !!meta && meta.langkah.some((x) => x.kunci === l.kunci);

function catatBerhasil(l) {
  putus = false;
  putusBeruntun = 0;
  if (berhasil || !meta || !milikMeta(l)) return;
  berhasil = true;
  if (asapData.status !== 'siap') ubahAsap({ status: 'siap' });
  if (metaTerbukti === meta) return;
  metaTerbukti = meta;
  const sisa = new Set([NAMA_META, ...meta.langkah.map((x) => `${x.kunci}.bin`)]);
  berkas.tulis(NAMA_META, meta.teks).then(() => berkas.sisakan(sisa));
}

function catatGagal(l) {
  // Frame linimasa sebelumnya yang baru selesai gagal tidak dihitung untuk linimasa ini.
  if (!meta || !milikMeta(l)) return;
  gagalAsap.add(l.kunci);
  if (berhasil || gagalAsap.size < Math.min(GAGAL_MAKS, meta.langkah.length)) return;
  // Linimasa baru yang tidak bisa dimuat (situs memberi linimasa cadangan saat ECMWF mati): kembali ke yang terakhir jalan.
  tandaGagal = meta.tanda;
  if (metaTerbukti && metaTerbukti !== meta) return pasangMeta(metaTerbukti);
  ubahAsap({ status: 'galat' });
}

/** Frame tak terunduh karena jaringan: bukan tanda linimasanya rusak, jadi tak dihitung ke GAGAL_MAKS. */
function catatPutus(l) {
  putus = true;
  jadwalkanUlangAsap();
  if (!meta || !milikMeta(l)) return;
  gagalAsap.add(l.kunci);
  // Belum ada yang tampil: pemuat berhenti, galat + Coba lagi (dan coba lagi sendiri).
  if (asapData.status === 'memuat') ubahAsap({ status: 'galat' });
}

/** Grid global frame `kunci` (UKURAN_GRID byte, baris 0 = selatan); null = frame ini gagal (tampil tanpa asap). */
function gridAsap(kunci) {
  const l = meta && meta.langkah.find((x) => x.kunci === kunci);
  if (!l || gagalAsap.has(kunci)) return Promise.resolve(null);
  let p = sedangAsap.get(kunci);
  if (!p) {
    p = ambilGrid(l).then(
      (g) => {
        sedangAsap.delete(kunci);
        if (g) catatBerhasil(l);
        else catatGagal(l);
        return g;
      },
      () => {
        sedangAsap.delete(kunci);
        catatPutus(l);
        return null;
      },
    );
    sedangAsap.set(kunci, p);
  }
  return p;
}

function nyalakanAsap() {
  asapAktif = true;
  periksaan = setInterval(segarkanAsapBilaBasi, PERIKSA_ULANG_MS);
  if (asapData.status === 'diam' || asapData.status === 'galat') return cobaLagiAsap();
  segarkanAsapBilaBasi();
}

function matikanAsap() {
  asapAktif = false;
  clearInterval(periksaan);
  clearTimeout(jadwalUlang);
  jadwalUlang = 0;
}

function cobaLagiAsap() {
  tandaGagal = '';
  gagalAsap.clear();
  putus = false;
  clearTimeout(jadwalUlang);
  jadwalUlang = 0;
  ubahAsap({ status: asapData.status === 'siap' ? 'siap' : 'memuat' });
  const lanjut = () => {
    ubahAsap({ versi: asapData.versi + 1 });
    periksaMeta();
  };
  if (meta) return lanjut();
  // Linimasa terakhir yang terbukti (luring): lapisan tampil dari simpanan sambil memeriksa yang baru.
  berkas.baca(NAMA_META).then((teks) => {
    const m = !meta && typeof teks === 'string' ? bacaMeta(teks) : null;
    if (m) {
      metaTerbukti = m;
      pasangMeta(m);
    }
    lanjut();
  });
}

/* ——— Data hotspot (lib/hotspot-muat.ts) ———
 * Hotspot NASA FIRMS satu hari dari GET /api/hotspot (publik). Server menyegarkan salinannya dari FIRMS tiap
 * 30 menit; satu simpanan per hari, jadi bilah waktu yang melintasi hari tak mengunduh ulang hari yang sama.
 */

/** Umur salinan sebelum diminta ulang — selaras cron situs. */
const UMUR_HOTSPOT_MS = 30 * 60000;
/** Gagal: coba lagi semenit kemudian, makin jarang bila terus gagal. */
const COBA_GALAT_MS = 60000;
const COBA_GALAT_MAKS_MS = 10 * 60000;
/** Server melayani salinan basi (FIRMS tak terjangkau): periksa tiap 2 menit. */
const COBA_BASI_MS = 120000;
const BATAS_HOTSPOT_MS = 30000;
const DIAM = { status: 'diam', data: null };
const MEMUAT = { status: 'memuat', data: null };

/** Kunci = hari diminta; '' = hari terbaru (24 jam bergulir). */
const simpananHotspot = new Map();
/** Hari yang sedang dipakai lapisan; null = lapisan mati. */
let kunciHotspot = null;

function ubahHotspot(h, keadaan) {
  h.keadaan = keadaan;
  selaraskan();
}

function segarHotspot(h) {
  return h.keadaan.status === 'siap' && Date.now() - h.diambil < (h.keadaan.data && h.keadaan.data.basi ? COBA_BASI_MS : UMUR_HOTSPOT_MS);
}

/** Penunda berikutnya: coba lagi setelah gagal, periksa salinan basi, atau segarkan saat umurnya habis. */
function jadwalkanHotspot(kunci, h) {
  clearTimeout(h.tunda);
  if (!h.pemakai) return;
  const jeda =
    h.keadaan.status === 'galat'
      ? Math.min(COBA_GALAT_MS * 2 ** Math.max(0, h.gagal - 1), COBA_GALAT_MAKS_MS)
      : Math.max(0, h.diambil + (h.keadaan.data && h.keadaan.data.basi ? COBA_BASI_MS : UMUR_HOTSPOT_MS) - Date.now());
  h.tunda = setTimeout(() => segarkanHotspot(kunci), jeda);
}

function segarkanHotspot(kunci) {
  const h = simpananHotspot.get(kunci);
  if (!h || !h.pemakai || h.muat) return;
  // Tersembunyi: tidak mengunduh; kembali tampak yang menyusul.
  if (document.hidden) return;
  if (segarHotspot(h)) return jadwalkanHotspot(kunci, h);
  h.muat = muatHotspot(kunci, h);
}

async function muatHotspot(kunci, h) {
  ubahHotspot(h, { status: 'memuat', data: h.keadaan.data });
  const versi = versiPulih;
  try {
    const baru = await ambil(`${GEO_URL}/api/hotspot${kunci ? `?hari=${kunci}` : ''}`, BATAS_HOTSPOT_MS, async (r) => {
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      return sahkanHotspot(await r.json());
    });
    if (!baru) throw new Error('bentuk data hotspot tidak dikenal');
    h.diambil = Date.now();
    h.gagal = 0;
    // Salinan server yang sama (cache CDN): objek lama dipakai, jadi peta tidak menyusun ulang penandanya.
    const lama = h.keadaan.data;
    const sama = !!lama && lama.hari === baru.hari && lama.diambil === baru.diambil && lama.basi === baru.basi && lama.titik.length === baru.titik.length;
    ubahHotspot(h, { status: 'siap', data: sama ? lama : baru });
    buangHotspotBasi();
  } catch (e) {
    h.gagal++;
    ubahHotspot(h, { status: 'galat', data: h.keadaan.data });
  } finally {
    h.muat = null;
    jadwalkanHotspot(kunci, h);
  }
  // Jaringan pulih selagi menunggu: coba lagi sekarang, bukan setelah jeda gagal.
  if (h.keadaan.status === 'galat' && versiPulih !== versi) segarkanHotspot(kunci);
}

/** Hari yang tidak dipakai dan sudah basi dibuang: menggeser hari tidak menimbun ±1 MB per tanggal. */
function buangHotspotBasi() {
  for (const [k, h] of simpananHotspot) {
    if (!h.pemakai && !h.muat && Date.now() - h.diambil > UMUR_HOTSPOT_MS) simpananHotspot.delete(k);
  }
}

/** Lapisan memakai hari `kunci` ('' = terbaru; null = mati). */
function pakaiHotspot(kunci) {
  if (kunci === kunciHotspot) return;
  const lama = kunciHotspot !== null && simpananHotspot.get(kunciHotspot);
  if (lama) {
    lama.pemakai = 0;
    clearTimeout(lama.tunda);
  }
  kunciHotspot = kunci;
  if (kunci === null) return;
  let h = simpananHotspot.get(kunci);
  if (!h) simpananHotspot.set(kunci, (h = { keadaan: MEMUAT, diambil: 0, gagal: 0, pemakai: 0, muat: null, tunda: 0 }));
  h.pemakai = 1;
  segarkanHotspot(kunci);
}

function keadaanHotspot() {
  if (kunciHotspot === null) return DIAM;
  const h = simpananHotspot.get(kunciHotspot);
  return h ? h.keadaan : MEMUAT;
}

const bilangan = (x) => typeof x === 'number' && Number.isFinite(x);
const bulatAntara = (x, min, maks) => Number.isInteger(x) && x >= min && x <= maks;
const berupaTeks = (x) => typeof x === 'string';

/** Baris titik yang bentuknya tidak dikenal dilewati; kepercayaan & satelit jadi indeks warna/ikon, jadi 0–2. */
function barisSah(t) {
  return (
    Array.isArray(t) && t.length >= 9 &&
    bilangan(t[0]) && Math.abs(t[0]) <= 180 && bilangan(t[1]) && Math.abs(t[1]) <= 90 &&
    bulatAntara(t[2], 0, 2) && bilangan(t[3]) && bilangan(t[4]) && bulatAntara(t[5], 0, 2) &&
    bulatAntara(t[6], 0, Infinity) && bulatAntara(t[7], 0, 1) && bulatAntara(t[8], -1, Infinity)
  );
}

function sahkanHotspot(d) {
  if (!d || !berupaTeks(d.hari) || !berupaTeks(d.diambil) || !Array.isArray(d.titik)) return null;
  const daftarTeks = (a) => (Array.isArray(a) ? a.filter(berupaTeks) : []);
  return {
    hari: d.hari,
    sejak: berupaTeks(d.sejak) ? d.sejak : undefined,
    hariTersedia: daftarTeks(d.hariTersedia),
    diperbarui: berupaTeks(d.diperbarui) ? d.diperbarui : null,
    diambil: d.diambil,
    satelit: daftarTeks(d.satelit),
    lengkap: d.lengkap !== false,
    basi: d.basi === true,
    // Dipetakan, bukan disaring: indeksnya harus tetap cocok dengan kolom provinsi & kabupaten titik.
    provinsi: Array.isArray(d.provinsi) ? d.provinsi.map((p) => (berupaTeks(p) ? p : '-')) : [],
    kabupaten: Array.isArray(d.kabupaten) ? d.kabupaten.map((k) => (berupaTeks(k) ? k : '-')) : [],
    titik: d.titik.filter(barisSah),
  };
}

/* ——— Peta (components/peta-gl.tsx) ——— */

const elPeta = document.getElementById('peta');
const peta = new maplibregl.Map({
  container: elPeta,
  // Peta dasar situs, persis: citra EOX lewat tembolok situs, karena EOX langsung dari Indonesia 3–14 dtk per ubin.
  style: {
    version: 8,
    sources: {
      'satellite-tiles': { type: 'raster', tiles: [URL_CITRA], tileSize: 256, attribution: 'Sentinel-2 cloudless by EOX IT Services GmbH', maxzoom: 14 },
    },
    layers: [
      { id: 'background', type: 'background', paint: { 'background-color': '#000000' } },
      { id: 'satellite-layer', type: 'raster', source: 'satellite-tiles',
        paint: { 'raster-opacity': 1.0, 'raster-brightness-max': 0.85, 'raster-contrast': 0.15, 'raster-saturation': -0.1 } },
    ],
  },
  center: [PUSAT.lng, PUSAT.lat],
  zoom: zoomLebar(LEBAR_NUSANTARA),
  maxZoom: ZOOM_MAKS,
  // Layar 3× tak terlihat lebih tajam (citra diperbesar), tetapi shader asap menggambar 2–3× lebih banyak piksel.
  pixelRatio: Math.min(window.devicePixelRatio, 2),
  // Lemparan meluncur dari kecepatan jari saat dilepas, seperti peta native.
  dragPan: { linearity: 0.6, deceleration: 2000, maxSpeed: 4000 },
  // Seluruh dunia bisa digeser, seperti Windy; asap global satu salinan dunia.
  renderWorldCopies: false,
  dragRotate: false,
  pitchWithRotate: false,
  touchPitch: false,
  attributionControl: false,
});
peta.touchZoomRotate.disableRotation();

/** Gaya termuat, lapisan asap & hotspot terpasang. */
let gayaSiap = false;
let globeTerpasang = false;
/** Ubin citra yang gagal (luring, pindah jaringan): MapLibre tidak memintanya lagi sendiri. */
const ubinGagal = new Map();

// style.load, bukan load: load menunggu semua ubin citra, jadi asap & lapisan telat muncul di sinyal lemah.
peta.once('style.load', () => {
  peta.addLayer(buatLapisanAsap(gambar));
  pasangLapisanHotspot(peta);
  gayaSiap = true;
  selaraskanSekarang();
});
// Konteks WebGL pulih: MapLibre memasang ulang gaya tanpa lapisan kustom, jadi asap dipasang lagi di bawah hotspot.
peta.on('webglcontextrestored', () =>
  peta.once('style.load', () => {
    if (!peta.getLayer('smoke-layer')) peta.addLayer(buatLapisanAsap(gambar), peta.getLayer(LAPIS_HOTSPOT) ? LAPIS_HOTSPOT : undefined);
  }),
);
// Siap begitu gaya dan penanda laporan terpasang (setelah bingkai pertamanya), bukan menunggu semua ubin citra di
// layar ('load'): penanda langsung tampil di atas citra yang menyusul, tidak di balik layar pemuat.
peta.once('style.load', () => peta.once('render', () => kirim('siap')));
peta.on('error', (e) => {
  const c = e.tile && e.tile.tileID && e.tile.tileID.canonical;
  if (e.sourceId === 'satellite-tiles' && c) ubinGagal.set(`${c.z}/${c.x}/${c.y}`, { x: c.x, y: c.y, z: c.z });
  else console.error(e.error);
});

// Ketukan peta ditunda sebentar: ketuk-ganda hanya memperbesar, tidak ikut menutup kartu/memilih hotspot.
let tundaKetuk = 0;
peta.on('movestart', () => clearTimeout(tundaKetuk));
peta.on('click', (e) => {
  clearTimeout(tundaKetuk);
  if (peta.isMoving()) return;
  tundaKetuk = setTimeout(() => {
    const h = hotspotPeta.titik;
    if (h && peta.getZoom() >= ZOOM_KETUK_HOTSPOT) {
      const indeks = hotspotDiTitik(peta, h, e.point, JARI_HOTSPOT);
      if (indeks !== null && pilihHotspot(indeks)) return;
    }
    ketukKosong();
  }, 250);
});
peta.on('moveend', aturTingkat);

/** Titik yang diketuk di bawah tepi atas kartunya: kamera menggeser titik itu ke tengah. */
function tampakkan(ll, kartu) {
  if (peta.project(ll).y > peta.getContainer().clientHeight - (peta.getPadding().bottom || 0) - kartu) peta.easeTo({ center: ll, duration: 350 });
}

function paskan(titik, maxZoom = 14) {
  if (titik.length === 0) return;
  const b = new maplibregl.LngLatBounds();
  for (const t of titik) b.extend([t.lng, t.lat]);
  peta.fitBounds(b, { padding: { top: 72, right: 44, bottom: 44, left: 44 }, maxZoom, duration: 600 });
}

function ketukKosong() {
  hotspotDipilih = null;
  if (kini.pilih) {
    kini.pilih = '';
    sorot();
  }
  kirim('ketuk');
}

/** Titik hotspot hari yang tampil diketuk; false bila datanya sudah berganti (hari berikutnya masih dimuat). */
function pilihHotspot(indeks) {
  const data = keadaanHotspot().data;
  const t = data && data === hotspotPeta.data ? data.titik[indeks] : null;
  if (!t) return false;
  tampakkan([t[0], t[1]], KARTU.hotspot);
  hotspotDipilih = data;
  titikDipilih = indeks;
  if (kini.pilih) {
    kini.pilih = '';
    sorot();
  }
  kirim(pesanHotspotTitik(data, indeks));
  return true;
}

/**
 * "hotspot <provinsi>\t<k>\t<waktu>\t<satelit>\t<frp>" titik `indeks` data `data`. Waktu (bulan, "(malam)") dan
 * FRP (koma/titik desimal) mengikuti bahasa; provinsi dari server, satelit nama diri.
 */
function pesanHotspotTitik(data, indeks) {
  const [, , k, frp, waktu, s, p, malam] = data.titik[indeks];
  const sat = SATELIT_HOTSPOT[s];
  const kolom = [data.provinsi[p] || '-', k, `${formatWib(waktu * 60000)}${malam ? ` ${teks('malam')}` : ''}`, `${sat.label} (${sat.sensor})`, `${angka(frp)} MW`];
  return `hotspot ${kolom.map((x) => String(x).replace(/[\t\n]/g, ' ')).join('\t')}`;
}

/* ——— Penanda laporan ——— */

let pin = [];
/** Tingkat kisi penanda (kisi-peta.ts); dikelompokkan ulang hanya saat berganti. */
let tingkat = 0;
let kunciPenanda = '';
let titikku = null;
let kunciTitikku = 'null';

/** Label pembaca layar penanda berisi laporan `isi`. */
const labelPin = (isi) => (isi.length > 1 ? teks('nLaporan', { n: isi.length }) : teks('laporan'));

/** Sorot penanda berisi laporan terpilih tanpa membangun ulang penanda. */
function sorot() {
  for (const { isi, el, redup } of pin) {
    const ya = isi.some((b) => b.id === kini.pilih);
    el.firstElementChild.classList.toggle('dipilih', ya);
    el.style.zIndex = ya ? '3' : redup ? '1' : '2';
  }
}

function jarakPilih() {
  const b = kini.laporan.find((x) => x.id === kini.pilih);
  if (!b || !kini.lokasiku) return '-';
  return jarakTeks(jarakKm(kini.lokasiku, b)).replace(/ /g, '\u00a0');
}

function aturPenanda() {
  const tampil = kini.mode === 'laporan' && kini.titik;
  const kunci = tampil ? `${kini.kunciLaporan}|${kini.pulau}|${tingkat}` : '';
  if (kunci === kunciPenanda) return;
  kunciPenanda = kunci;
  for (const p of pin) p.m.remove();
  pin = [];
  if (!tampil) return;
  for (const isi of kelompokkan(kini.laporan, tingkat)) {
    const redup = kini.pulau !== '' && isi[0].pulau !== kini.pulau;
    const el = document.createElement('div');
    el.className = 'pin';
    el.setAttribute('role', 'button');
    el.setAttribute('aria-label', labelPin(isi));
    const tetes = document.createElement('div');
    tetes.className = 'tetes';
    const dalam = document.createElement('div');
    dalam.className = 'isi';
    dalam.innerHTML = API;
    if (isi.length > 1) dalam.append(String(isi.length));
    tetes.append(dalam);
    el.append(tetes);
    el.addEventListener('click', (e) => {
      e.stopPropagation();
      clearTimeout(tundaKetuk);
      // Kelompok yang masih terpisah bila diperbesar: kamera mendekat ke anggotanya, bukan memilih.
      if (!satuTitik(isi) && peta.getZoom() < ZOOM_MAKS) return paskan(isi, ZOOM_MAKS);
      kini.pilih = (isi.find((b) => b.id === kini.pilih) || isi[0]).id;
      hotspotDipilih = null;
      // Disorot langsung, tanpa menunggu pilih kembali dari Contract.
      sorot();
      tampakkan([isi[0].lng, isi[0].lat], KARTU.laporan);
      kirim(`pilih ${kini.pilih} ${jarakPilih()}`);
    });
    // Ujung tetes tepat di titik kejadian: kotak diputar −45°, sudut lancipnya berjari-jari 3.
    // Marker menimpa style.opacity elemennya dengan opsi `opacity`, jadi redup harus lewat opsi itu.
    const m = new maplibregl.Marker({
      element: el,
      anchor: 'center',
      opacity: redup ? 0.35 : 1,
      opacityWhenCovered: BALIK_BOLA,
      offset: [0, -(UK_PIN / Math.SQRT2 - 1)],
    })
      .setLngLat([isi[0].lng, isi[0].lat])
      .addTo(peta);
    pin.push({ isi, el, redup, m });
  }
  sorot();
}

function aturTingkat() {
  const b = peta.getBounds();
  const t = tingkatZoom(b.getEast() - b.getWest());
  if (t !== tingkat) {
    tingkat = t;
    aturPenanda();
  }
}

function aturTitikku() {
  const kunci = JSON.stringify(kini.lokasiku);
  if (kunci === kunciTitikku) return;
  kunciTitikku = kunci;
  if (titikku) titikku.remove();
  titikku = null;
  if (!kini.lokasiku) return;
  const el = document.createElement('div');
  el.className = 'lokasiku';
  titikku = new maplibregl.Marker({ element: el, opacityWhenCovered: BALIK_BOLA }).setLngLat([kini.lokasiku.lng, kini.lokasiku.lat]).addTo(peta);
}

/* ——— Hotspot di peta & Windy ——— */

/** Yang tampil di lapisan hotspot: kunci (hari|diambil|satelit, 'memuat', null), hari, baris tampil, data. */
const hotspotPeta = { kunci: null, hari: null, titik: null, data: null };
/** Data hari titik hotspot yang kartunya terbuka; berganti = kartu ditutup. */
let hotspotDipilih = null;
/** Indeks titik itu di data harinya: kartunya dikirim ulang saat bahasa berganti. */
let titikDipilih = -1;
let kabarHotspotLalu = '';

function aturHotspotPeta(hs) {
  if (!gayaSiap) return;
  // Hari berikutnya masih dimuat: titik yang tampil dibiarkan, lalu pudar ke yang baru.
  const memuat = adaSatelit() && !hs.data && hs.status === 'memuat';
  const kunci = adaSatelit() && hs.data ? `${hs.data.hari}|${hs.data.diambil}|${kini.satelit.join()}` : memuat ? 'memuat' : null;
  if (kunci === hotspotPeta.kunci) return;
  hotspotPeta.kunci = kunci;
  if (kunci === 'memuat') return;
  if (!kunci) {
    Object.assign(hotspotPeta, { hari: null, titik: null, data: null });
    return setelDataHotspot(peta, [], false);
  }
  const titik = susunTitikTampil(hs.data, kini.satelit);
  const pudar = hotspotPeta.hari !== null && hotspotPeta.hari !== hs.data.hari;
  Object.assign(hotspotPeta, { hari: hs.data.hari, titik, data: hs.data });
  setelDataHotspot(peta, titik, pudar);
}

function aturKartuHotspot(hs) {
  // Hotspot terpilih hanya berlaku untuk data hari yang sama (indeks berubah saat hari berganti).
  if (hotspotDipilih && hotspotDipilih !== hs.data) {
    hotspotDipilih = null;
    kirim('ketuk');
  }
}

function kabarHotspot(hs) {
  // Semua satelit mati: tak dikabarkan; dinyalakan lagi = dikabarkan ulang walau sama.
  if (!adaSatelit()) {
    kabarHotspotLalu = '';
    return;
  }
  const status = hs.status === 'siap' ? (hs.data && hs.data.basi ? 'basi' : 'ok') : hs.status === 'galat' ? 'galat' : 'memuat';
  const n = hs.data ? jumlahPerSatelit(hs.data) : ['', '', ''];
  const t = `hotspot-info ${status}\t${n.join('\t')}\t${hs.data ? labelHari(hs.data) : ''}`;
  if (t === kabarHotspotLalu) return;
  kabarHotspotLalu = t;
  kirim(t);
}

/** Hari hotspot mengikuti langkah bilah waktu (WIB), seperti situs; langkah prakiraan memakai hari terbaru. */
function hariHotspotKini() {
  const l = asapTampil() ? asapData.langkah[indeksAsap()] : kini.mode === 'angin' ? udara.langkah[udara.indeks] : undefined;
  const hari = l && !l.adalahPrediksi && l.iso ? hariWib(Date.parse(l.iso)) : null;
  // Hari tak berbentuk YYYY-MM-DD: hari terbaru.
  return hari && hari !== hariIni && /^\d{4}-\d{2}-\d{2}$/.test(hari) ? hari : '';
}

/* ——— Animasi asap (peta-gl.tsx: frame A/B + u_mix tiap frame layar, seperti renderLoop situs) ——— */

const gambar = { kunciA: '', a: null, kunciB: '', b: null, mix: 0, gaya: 0, tampil: false };
/** Frame per kunci; null = gagal (tampil tanpa asap). */
const frame = new Map();
/** langkah = [{k, w}] linimasa yang dipakai peta; kunci = kunci-kuncinya ('' = asap tak tampil). */
const main = { langkah: [], progres: 0, indeks: -1, raf: 0, lalu: 0, gen: 0, kunci: '', versi: -1 };
let putarAsap = true;
let lajuAsap = 1;

const indeksAsap = () => Math.max(0, main.indeks);

function indeksTerdekat(langkah, waktu) {
  let terbaik = 0;
  for (let i = 1; i < langkah.length; i++) {
    if (Math.abs(langkah[i].w - waktu) < Math.abs(langkah[terbaik].w - waktu)) terbaik = i;
  }
  return terbaik;
}

function gambarUlang() {
  const L = main.langkah;
  gambar.tampil = asapTampil() && L.length > 0;
  if (gambar.tampil) {
    const n = L.length;
    const iA = Math.floor(main.progres) % n;
    const kA = L[iA].k;
    const kB = L[(iA + 1) % n].k;
    // Frame yang belum datang: gambar sebelumnya tetap tampil.
    if (frame.has(kA) && gambar.kunciA !== kA) {
      gambar.kunciA = kA;
      gambar.a = frame.get(kA);
    }
    const adaB = frame.has(kB);
    gambar.kunciB = adaB ? kB : gambar.kunciA;
    gambar.b = adaB ? frame.get(kB) : gambar.a;
    gambar.mix = main.progres - Math.floor(main.progres);
    gambar.gaya = kini.gaya;
    if (iA !== main.indeks) {
      main.indeks = iA;
      selaraskan();
    }
  }
  peta.triggerRepaint();
}

const harusPutar = () => gayaSiap && asapTampil() && putarAsap && !document.hidden && main.langkah.length >= 2;

function langkahi(t) {
  main.raf = 0;
  const L = main.langkah;
  if (!harusPutar()) return;
  const dt = main.lalu ? Math.min((t - main.lalu) / 1000, 0.1) : 0;
  main.lalu = t;
  const iA = Math.floor(main.progres) % L.length;
  // Menunggu frame berikutnya (tidak melompatinya); frame gagal (null) tampil tanpa asap.
  if (frame.has(L[(iA + 1) % L.length].k)) main.progres = (main.progres + dt / LAMA_LANGKAH[lajuAsap]) % L.length;
  gambarUlang();
  main.raf = requestAnimationFrame(langkahi);
}

function aturPutarAsap() {
  if (harusPutar()) {
    if (!main.raf) {
      main.lalu = 0;
      main.raf = requestAnimationFrame(langkahi);
    }
  } else if (main.raf) {
    cancelAnimationFrame(main.raf);
    main.raf = 0;
  }
}

/** Minta frame yang belum ada, terdekat dari yang tampil dulu; frame yang tadi gagal dicoba lagi. */
function mulaiUnduhFrame() {
  for (const [k, v] of frame) if (v === null) frame.delete(k);
  const gen = main.gen;
  const L = main.langkah;
  const diminta = new Set();
  const kerja = async () => {
    while (gen === main.gen) {
      const m = Math.max(0, Math.floor(main.progres));
      const k = L.map((_, j) => L[(m + j) % L.length].k).find((x) => !frame.has(x) && !diminta.has(x));
      if (!k) return;
      diminta.add(k);
      const g = await gridAsap(k).catch(() => null);
      if (gen !== main.gen) return;
      frame.set(k, g);
      if (!main.raf) gambarUlang();
    }
  };
  for (let n = 0; n < UNDUH_SERENTAK; n++) kerja();
}

function aturAsapPeta() {
  if (!gayaSiap) return;
  const kunci = asapTampil() ? asapData.langkah.map((l) => l.kunci).join(',') : '';
  const berganti = kunci !== main.kunci;
  if (berganti) {
    main.kunci = kunci;
    // Asap dimatikan: linimasa disimpan, jadi kembali ke mode Asap melanjutkan dari langkah yang sama.
    // Linimasa baru: posisi mengikuti WAKTU langkah yang sedang tampil (atau sekarang).
    if (kunci) {
      const lalu = main.langkah[main.indeks];
      const baru = asapData.langkah.map((l) => ({ k: l.kunci, w: l.waktu }));
      main.langkah = baru;
      const ada = new Set(baru.map((l) => l.k));
      for (const k of frame.keys()) if (!ada.has(k)) frame.delete(k);
      main.progres = indeksTerdekat(baru, lalu ? lalu.w : Date.now());
      main.indeks = -1;
    }
  }
  if (berganti || (kunci && asapData.versi !== main.versi)) {
    main.versi = asapData.versi;
    main.gen++;
    if (kunci) mulaiUnduhFrame();
  }
  aturPutarAsap();
  // Mode/palet berganti saat jeda: gambar ulang sekali.
  if (!main.raf) gambarUlang();
}

/** Bilah waktu digeser/diketuk ke langkah `i`. */
function pilihAsap(i) {
  if (main.langkah.length === 0) return;
  main.progres = Math.min(Math.max(0, i), main.langkah.length - 1);
  main.indeks = Math.floor(main.progres);
  gambarUlang();
  selaraskan();
}

/* ——— Mode Angin (peta-prakiraan.tsx, pesan-prakiraan.ts) ———
 * Halaman /api/forecasting situs (Windy AQI yang diproksikan + medan AQI & partikel angin dari /api/udara) hanya
 * berbicara dengan jendela INDUK-nya, jadi ia dibingkai langsung di sini dan halaman ini induknya.
 * Catatan: /api/forecasting menjawab "frame-ancestors *", yang di Chromium tidak mencakup induk file://; karena itu
 * paket/web memuat halaman ini dari https://appassets.androidplatform.net (dilayani dari aset aplikasi).
 */

/** Windy + /api/forecasting (±300 KB) + bundel Windy sendiri: lama di 3G, tetapi tak selamanya. */
const BATAS_MUAT_MS = 45000;
const elAngin = document.getElementById('angin');
const elMemuatAngin = document.getElementById('angin-memuat');
const elGalatAngin = document.getElementById('angin-galat');

/** Linimasa & pemutaran lapisan Angin; niat warga (Windy memutar otomatis seperti situs). */
const udara = { langkah: [], modelRunWib: null, indeks: 0, putar: true, laju: 1, tunggu: false };
/** Keadaan jembatan ke iframe (Jembatan peta-prakiraan.tsx). */
const jembatan = {
  iframe: null,
  /** FORECASTING_READY sudah tiba untuk muatan ini; sebelum itu pesan tak dikirim (dikirim lengkap saat siap). */
  siap: false,
  status: 'memuat',
  batas: 0,
  /** Kamera terakhir (zoom MapLibre), dari peta, perintah, atau iframe (KAMERA_WINDY). */
  kamera: null,
  /** Progres pecahan terakhir; diteruskan ke iframe yang dimuat ulang supaya melanjutkan. */
  progres: null,
  indeks: -1,
  /** Lompatan bilah waktu yang belum dipantulkan iframe: WAKTU_UDARA yang dikirim sebelumnya diabaikan. */
  lompat: null,
  /** WAKTU_UDARA tiba tiap frame layar selama berputar: diteruskan hanya bila langkah/status berubah. */
  waktuLalu: '',
  /** Yang terakhir dikirim: tema, lokasiku, angin, putar, hotspot. */
  terkirim: null,
};

/** Lintang yang tampil `px` piksel di selatan `lat` pada zoom MapLibre `zoom` (ubin 512, Mercator). */
function geserLintang(lat, zoom, px) {
  const y = Math.log(Math.tan(Math.PI / 4 + (lat * Math.PI) / 360)) - (px * 2 * Math.PI) / (512 * 2 ** zoom);
  return (Math.atan(Math.exp(y)) * 360) / Math.PI - 90;
}

/**
 * Kamera MapLibre berpusat di atas padding bawahnya, Windy layar penuh: pusatnya `sela` px lebih ke selatan.
 * WebView ini berakhir di atas bilah tab, jadi padding peta 0 dan sela 0; tetap dihitung bila suatu saat ada.
 */
const selaAngin = () => (peta.getPadding().bottom || 0) / 2;

/** Zoom Windy (skala Leaflet) = zoom MapLibre + 1. */
function pesanKamera(k) {
  return { type: 'SET_KAMERA', lat: geserLintang(k.lat, k.zoom, selaAngin()), lon: k.lng, zoom: k.zoom + 1 };
}

/** Baris /api/hotspot (susunTitikTampil) → urutan yang dibaca iframe; null = kosongkan. */
function pesanHotspot(data) {
  return {
    type: 'SET_HOTSPOT',
    titik: data ? susunTitikTampil(data, kini.satelit).map((t) => [t[0], t[1], t[2], t[5], t[3], t[4], t[7], t[6]]) : null,
    provinsi: data ? data.provinsi : [],
  };
}

/** Halaman tersembunyi: putar dijeda dan loop partikel iframe berhenti total. */
function pesanPutar() {
  return !document.hidden
    ? [{ type: 'WINDY_ACTIVE' }, { type: 'PUTAR_UDARA', putar: udara.putar, laju: udara.laju }]
    : [{ type: 'PUTAR_UDARA', putar: false }, { type: 'WINDY_NONAKTIF' }];
}

function kirimIframe(daftar) {
  const w = jembatan.siap && jembatan.iframe && jembatan.iframe.contentWindow;
  if (w) daftar.forEach((d) => w.postMessage(d, '*'));
}

function aturStatusAngin(s) {
  jembatan.status = s;
  clearTimeout(jembatan.batas);
  // Windy/proksi tak terjangkau = iframe diam selamanya: tampilkan galat + "Coba lagi".
  if (s === 'memuat') jembatan.batas = setTimeout(() => aturStatusAngin('galat'), BATAS_MUAT_MS);
  // Memudar: FORECASTING_READY tiba sebelum lapisan AQI tergambar, jadi hilang seketika terasa memotong.
  elMemuatAngin.classList.toggle('tampil', s === 'memuat');
  elGalatAngin.classList.toggle('tampil', s === 'galat');
}

/**
 * (Muat ulang) iframe di kamera terakhir; kamera & tema sesudahnya lewat pesan (mengganti src = memuat ulang Windy).
 * Teks di dalam iframe (legenda AQI, Windy) milik situs dan tidak mengenal `bahasa`: tetap seperti situs.
 */
function muatAngin() {
  if (jembatan.iframe) jembatan.iframe.remove();
  jembatan.siap = false;
  jembatan.lompat = null;
  jembatan.waktuLalu = '';
  const k = jembatan.kamera;
  const src = `${URL_PRAKIRAAN}?lat=${geserLintang(k.lat, k.zoom, selaAngin()).toFixed(3)}&lon=${k.lng.toFixed(3)}&zoom=${(k.zoom + 1).toFixed(2)}&tema=${kini.tema}&aplikasi=1`;
  const f = document.createElement('iframe');
  // sandbox sama dengan situs: skrip pihak ketiga berjalan di origin buram.
  f.setAttribute('sandbox', 'allow-scripts allow-popups');
  f.title = teks('judulAngin');
  f.src = src;
  elAngin.prepend(f);
  jembatan.iframe = f;
  aturStatusAngin('memuat');
}

/** Masuk mode Angin: Windy mulai di tempat yang sedang dilihat, di langkah terakhir bilah waktunya. */
function bukaAngin() {
  const c = peta.getCenter();
  jembatan.kamera = { lat: c.lat, lng: c.lng, zoom: peta.getZoom() };
  jembatan.indeks = -1;
  jembatan.progres = udara.indeks;
  jembatan.terkirim = null;
  elAngin.classList.add('tampil');
  muatAngin();
}

/** Keluar mode Angin: iframe dilepas (memori), peta MapLibre melanjutkan dari tempat terakhir Windy. */
function tutupAngin() {
  clearTimeout(jembatan.batas);
  if (jembatan.iframe) jembatan.iframe.remove();
  jembatan.iframe = null;
  jembatan.siap = false;
  elAngin.classList.remove('tampil');
  const k = jembatan.kamera;
  if (k) peta.jumpTo({ center: [k.lng, k.lat], zoom: k.zoom });
}

/** Pesan tema/lokasiku/lapisan/putar/hotspot ke iframe yang berubah sejak terakhir dikirim. */
function aturPesanAngin(hs) {
  if (!jembatan.iframe) return;
  const memuat = adaSatelit() && !hs.data && hs.status === 'memuat';
  const k = {
    tema: kini.tema,
    lok: JSON.stringify(kini.lokasiku),
    angin: kini.angin,
    putar: `${document.hidden}|${udara.putar}|${udara.laju}`,
    // Hari berikutnya masih dimuat: titik yang tampil dibiarkan.
    hotspot: !adaSatelit() ? 'kosong' : hs.data ? `${hs.data.hari}|${hs.data.diambil}|${kini.satelit.join()}` : memuat ? 'memuat' : 'kosong',
  };
  const lalu = jembatan.terkirim;
  jembatan.terkirim = k;
  if (!lalu || !jembatan.siap) return;
  const daftar = [];
  if (k.tema !== lalu.tema) daftar.push({ type: 'SET_TEMA', tema: kini.tema });
  if (k.lok !== lalu.lok && kini.lokasiku) daftar.push({ type: 'SET_LOKASIKU', lat: kini.lokasiku.lat, lng: kini.lokasiku.lng });
  if (k.angin !== lalu.angin) daftar.push({ type: 'SET_LAPISAN', angin: kini.angin, poligon: false, angka: false });
  if (k.hotspot !== lalu.hotspot && k.hotspot !== 'memuat') daftar.push(pesanHotspot(k.hotspot === 'kosong' ? null : hs.data));
  if (k.putar !== lalu.putar) daftar.push(...pesanPutar());
  if (daftar.length) kirimIframe(daftar);
}

const angkaSah = (v) => typeof v === 'number' && Number.isFinite(v);

/** Pesan iframe → pesan bertipe, atau null. Isinya dari halaman pihak ketiga — jangan percaya bentuknya. */
function bacaPesanPrakiraan(p) {
  switch (p.type) {
    case 'FORECASTING_READY':
    case 'ALIH_LAPISAN':
      return { type: p.type };
    case 'LINIMASA_UDARA':
      if (!Array.isArray(p.langkah)) return null;
      return {
        type: 'LINIMASA_UDARA',
        langkah: p.langkah.map((l) => ({
          labelWib: l && typeof l.labelWib === 'string' ? l.labelWib : '',
          adalahPrediksi: Boolean(l && l.adalahPrediksi),
          // Di luar rentang Date (±8,64e15 ms) toISOString() melempar.
          waktu: l && angkaSah(l.waktu) && Math.abs(l.waktu) <= 8.64e15 ? l.waktu : null,
        })),
        modelRunWib: typeof p.modelRunWib === 'string' ? p.modelRunWib : null,
      };
    case 'WAKTU_UDARA':
      if (!angkaSah(p.progres)) return null;
      return {
        type: 'WAKTU_UDARA',
        progres: p.progres,
        memutar: Boolean(p.memutar),
        // Putar tertahan menunggu frame berikutnya diunduh (bukan macet).
        menunggu: Boolean(p.memutar && p.menunggu),
        terunduh: angkaSah(p.terunduh) ? p.terunduh : 0,
        total: angkaSah(p.total) ? p.total : 0,
      };
    case 'KAMERA_WINDY':
      if (!angkaSah(p.lat) || !angkaSah(p.lng) || !angkaSah(p.zoom)) return null;
      return { type: 'KAMERA_WINDY', lat: p.lat, lng: p.lng, zoom: p.zoom };
    default:
      return null;
  }
}

window.addEventListener('message', (e) => {
  const f = jembatan.iframe;
  if (!f || e.source !== f.contentWindow || !e.data || typeof e.data !== 'object') return;
  const d = e.data;
  if (d.type === 'FORECASTING_READY') jembatan.waktuLalu = '';
  if (d.type === 'WAKTU_UDARA') {
    const k = [d.total > 0 ? Math.floor(d.progres) % d.total : 0, !!d.memutar, !!d.menunggu, d.terunduh, d.total].join();
    if (k === jembatan.waktuLalu) return;
    jembatan.waktuLalu = k;
  }
  const p = bacaPesanPrakiraan(d);
  if (!p) return;
  const j = jembatan;
  if (p.type === 'FORECASTING_READY') {
    j.siap = true;
    j.lompat = null;
    aturStatusAngin('siap');
    // Urutan sama dengan pendengar FORECASTING_READY di components/peta.tsx situs.
    const hs = keadaanHotspot();
    const daftar = [{ type: 'SET_TEMA', tema: kini.tema }, pesanKamera(j.kamera), { type: 'SET_KOLOM', kiri: 0 }];
    if (adaSatelit() && hs.data) daftar.push(pesanHotspot(hs.data));
    if (kini.lokasiku) daftar.push({ type: 'SET_LOKASIKU', lat: kini.lokasiku.lat, lng: kini.lokasiku.lng });
    daftar.push({ type: 'SET_LAPISAN', angin: kini.angin, poligon: false, angka: false });
    if (j.progres !== null) daftar.push({ type: 'SET_PROGRES', progres: j.progres });
    kirimIframe([...daftar, ...pesanPutar()]);
    j.terkirim = null;
    aturPesanAngin(hs);
  } else if (p.type === 'LINIMASA_UDARA') {
    udara.langkah = p.langkah.map((l) => ({
      labelWib: l.labelWib,
      adalahPrediksi: l.adalahPrediksi,
      iso: l.waktu === null ? '' : new Date(l.waktu).toISOString(),
    }));
    udara.modelRunWib = p.modelRunWib;
    selaraskan();
  } else if (p.type === 'WAKTU_UDARA') {
    const i = p.total > 0 ? ((Math.floor(p.progres) % p.total) + p.total) % p.total : 0;
    // Dikirim sebelum lompatan terakhir sampai (bilah diseret 3→7 lalu tiba 5, 6): label & kenop tak mundur.
    if (j.lompat && i !== j.lompat.ke && Date.now() - j.lompat.t < 1000) return;
    j.lompat = null;
    j.progres = p.progres;
    j.indeks = i;
    udara.indeks = i;
    udara.tunggu = p.menunggu;
    selaraskan();
  } else if (p.type === 'KAMERA_WINDY') {
    j.kamera = { lat: geserLintang(p.lat, p.zoom, -selaAngin()), lng: p.lng, zoom: p.zoom };
  }
});

document.getElementById('angin-ulang').addEventListener('click', () => muatAngin());

/** Bilah waktu Angin digeser/diketuk: lompat (putar tak dihentikan, sama dengan situs). */
function pilihUdara(i) {
  udara.indeks = i;
  if (i !== jembatan.indeks) {
    jembatan.progres = i;
    if (jembatan.siap) jembatan.lompat = { ke: i, t: Date.now() };
    kirimIframe([{ type: 'SET_PROGRES', progres: i }]);
  }
  selaraskan();
}

/* ——— Logo Copernicus ——— */

const elLogo = document.getElementById('logo');
const gambarLogo = elLogo.querySelector('img');
elLogo.addEventListener('click', () => kirim(`buka ${URL_COPERNICUS}`));
// Logo dari geoplatform: gagal (luring) = tak tampak sama sekali, bukan kotak gambar rusak; dicoba lagi bersama yang lain.
gambarLogo.addEventListener('load', () => (gambarLogo.style.visibility = ''));
gambarLogo.addEventListener('error', () => (gambarLogo.style.visibility = 'hidden'));
function ulangiLogo() {
  if (gambarLogo.style.visibility !== 'hidden') return;
  const src = gambarLogo.src;
  gambarLogo.src = '';
  gambarLogo.src = src;
}

/* ——— Bilah waktu (bilah-waktu.tsx) ———
 * Putar/jeda, 1×/2×, rel yang bisa diketuk atau diseret (berhenti tepat di tiap langkah), label waktu + cip
 * Prediksi/Analisis. Diseret = animasi dijeda lalu dilanjutkan. Seret mendatar memilih waktu; tarikan tegak
 * (> 12 px sebelum seret mulai) diabaikan.
 */

const elBilah = document.getElementById('bilah');
const $b = (s) => elBilah.querySelector(s);
const bilahEl = {
  ikonKabar: $b('.kabar .ikon'),
  teksKabar: $b('.kabar .teks'),
  coba: $b('.kabar .coba'),
  putar: $b('.putar'),
  laju: $b('.laju'),
  putaran: $b('.baris-label .putaran'),
  waktu: $b('.waktu'),
  cip: $b('.cip'),
  teksCip: $b('.cip b'),
  hariAwal: $b('.hari'),
  hariAkhir: $b('.hari.akhir'),
  rel: $b('.wadah-rel'),
  isiRel: $b('.isi-rel'),
  kenop: $b('.kenop'),
};
/** Yang sedang digambar: jenis (asap/udara), indeks & jumlah langkah, tinggi yang terakhir dikabarkan. */
const bilahLalu = { warna: null, indeks: -1, n: 0, tinggi: -1, ikon: '', kabarIkon: '' };
let gestur = null;
let menyeret = false;

/** "23 Okt, 01:00 WIB" → "23 Okt" ("23 Oct" dalam bahasa Inggris). */
const hari = (l) => (l ? labelLangkah(l).split(',')[0] : '');

/** Isi bilah saat ini, atau null bila tak tampil. */
function isiBilah() {
  if (asapTampil()) {
    return {
      warna: 'asap',
      langkah: asapData.langkah,
      indeks: indeksAsap(),
      memutar: putarAsap,
      laju: lajuAsap,
      status: asapData.status === 'diam' ? 'memuat' : asapData.status,
      galat: asapData.status === 'galat' ? teks('gagalAsap') : null,
      cobaLagi: true,
    };
  }
  if (kini.mode === 'angin' && udara.langkah.length > 0) {
    return { warna: 'udara', langkah: udara.langkah, indeks: udara.indeks, memutar: udara.putar, laju: udara.laju, status: udara.tunggu ? 'memuat' : 'siap', galat: null, cobaLagi: false };
  }
  return null;
}

function aturPosisi(p, luncur) {
  elBilah.classList.toggle('luncur', luncur);
  bilahEl.isiRel.style.transform = `scaleX(${p})`;
  bilahEl.kenop.style.left = `${p * 100}%`;
}

function gambarBilah() {
  const b = isiBilah();
  elBilah.dataset.tema = kini.tema;
  elBilah.style.bottom = `${kini.padBawah + SELA_BILAH}px`;
  if (!b) {
    elBilah.classList.remove('tampil');
    bilahLalu.warna = null;
    return kabarTinggiBilah(0);
  }
  elBilah.classList.add('tampil');
  elBilah.dataset.warna = b.warna;
  const n = b.langkah.length;
  const i = Math.min(Math.max(0, b.indeks), n - 1);
  const l = b.langkah[i];
  const berkabar = b.status === 'galat' || !l;
  elBilah.classList.toggle('berkabar', berkabar);
  if (berkabar) {
    const memuat = b.status !== 'galat';
    const ikon = memuat ? IKON_PUTARAN : IKON_AWAN_MATI;
    if (bilahLalu.kabarIkon !== ikon) bilahEl.ikonKabar.innerHTML = bilahLalu.kabarIkon = ikon;
    bilahEl.teksKabar.textContent = memuat ? teks('memuatLinimasa') : b.galat || teks('tidakTersedia');
    bilahEl.coba.hidden = memuat || !b.cobaLagi;
    bilahLalu.warna = null;
  } else {
    const ikon = b.memutar ? IKON_JEDA : IKON_MAIN;
    if (bilahLalu.ikon !== ikon) bilahEl.putar.innerHTML = bilahLalu.ikon = ikon;
    bilahEl.putar.classList.toggle('main', !b.memutar);
    bilahEl.putar.setAttribute('aria-label', teks(b.memutar ? 'jeda' : 'putar'));
    bilahEl.laju.textContent = `${b.laju}×`;
    bilahEl.putaran.style.display = b.status === 'memuat' ? '' : 'none';
    const label = labelLangkah(l);
    const jenis = teks(l.adalahPrediksi ? 'prediksi' : 'analisis');
    bilahEl.waktu.textContent = label;
    bilahEl.cip.classList.toggle('prediksi', l.adalahPrediksi);
    bilahEl.teksCip.textContent = jenis.toUpperCase();
    bilahEl.hariAwal.textContent = hari(b.langkah[0]);
    bilahEl.hariAkhir.textContent = hari(b.langkah[n - 1]);
    bilahEl.rel.setAttribute('aria-valuetext', `${label}, ${jenis}`);
    // Langkah berikutnya saat berputar meluncur; lompatan (putaran ulang, ketukan, masuk mode) langsung di tempat.
    if (!menyeret) {
      const sama = bilahLalu.warna === b.warna && bilahLalu.n === n;
      aturPosisi(n > 1 ? i / (n - 1) : 0, sama && Math.abs(i - bilahLalu.indeks) === 1);
    }
    Object.assign(bilahLalu, { warna: b.warna, indeks: i, n });
  }
  kabarTinggiBilah(Math.round(elBilah.offsetHeight));
}

function kabarTinggiBilah(tinggi) {
  if (tinggi === bilahLalu.tinggi) return;
  bilahLalu.tinggi = tinggi;
  kirim(`bilah ${tinggi}`);
}

function putarBilah(ya) {
  const b = isiBilah();
  if (b && b.warna === 'udara') udara.putar = ya;
  else putarAsap = ya;
  selaraskanSekarang();
}

function pilihLangkah(i) {
  if (asapTampil()) pilihAsap(i);
  else if (kini.mode === 'angin') pilihUdara(i);
}

/** x di rel → langkah terdekat. */
function keLangkah(x) {
  const b = isiBilah();
  const n = b ? b.langkah.length : 0;
  const r = bilahEl.rel.getBoundingClientRect();
  return n > 1 && r.width > 0 ? Math.round(Math.min(1, Math.max(0, (x - r.left) / r.width)) * (n - 1)) : 0;
}

function tandaiLangkah(i) {
  const b = isiBilah();
  const n = b ? b.langkah.length : 0;
  aturPosisi(n > 1 ? i / (n - 1) : 0, false);
}

bilahEl.putar.addEventListener('click', () => {
  const b = isiBilah();
  if (b) putarBilah(!b.memutar);
});
bilahEl.laju.addEventListener('click', () => {
  const b = isiBilah();
  if (!b) return;
  const laju = b.laju === 1 ? 2 : 1;
  if (b.warna === 'udara') udara.laju = laju;
  else lajuAsap = laju;
  selaraskanSekarang();
});
bilahEl.coba.addEventListener('click', () => cobaLagiAsap());

bilahEl.rel.addEventListener('pointerdown', (e) => {
  if (gestur || !isiBilah()) return;
  bilahEl.rel.setPointerCapture(e.pointerId);
  gestur = { id: e.pointerId, x0: e.clientX, y0: e.clientY, seret: false, gagal: false, lalu: 0, lanjutkan: false };
});
bilahEl.rel.addEventListener('pointermove', (e) => {
  const g = gestur;
  if (!g || e.pointerId !== g.id || g.gagal) return;
  if (!g.seret) {
    if (Math.abs(e.clientY - g.y0) > 12) {
      g.gagal = true;
      return;
    }
    if (Math.abs(e.clientX - g.x0) <= 4) return;
    // Seret mulai: animasi dijeda selama rel diseret, lalu dilanjutkan.
    const b = isiBilah();
    g.seret = true;
    menyeret = true;
    g.lalu = b ? Math.min(Math.max(0, b.indeks), b.langkah.length - 1) : 0;
    g.lanjutkan = !!(b && b.memutar);
    if (g.lanjutkan) putarBilah(false);
  }
  const i = keLangkah(e.clientX);
  tandaiLangkah(i);
  if (i === g.lalu) return;
  g.lalu = i;
  pilihLangkah(i);
});
function akhiriGestur(e, batal) {
  const g = gestur;
  if (!g || e.pointerId !== g.id) return;
  gestur = null;
  if (g.seret) {
    menyeret = false;
    if (g.lanjutkan) putarBilah(true);
    else selaraskanSekarang();
  } else if (!batal && !g.gagal && Math.hypot(e.clientX - g.x0, e.clientY - g.y0) <= 10) {
    const i = keLangkah(e.clientX);
    tandaiLangkah(i);
    pilihLangkah(i);
  }
}
bilahEl.rel.addEventListener('pointerup', (e) => akhiriGestur(e, false));
bilahEl.rel.addEventListener('pointercancel', (e) => akhiriGestur(e, true));

/* ——— Selaraskan: keadaan Contract + data → peta, Windy, bilah, kabar (peta-laporan.tsx) ——— */

let modeLalu = null;
let kabarAsapLalu = '';
let dijadwalkan = false;

/** Dikumpulkan per giliran: banyak perubahan data beruntun = satu kali menyelaraskan. */
function selaraskan() {
  if (dijadwalkan) return;
  dijadwalkan = true;
  queueMicrotask(() => {
    dijadwalkan = false;
    selaraskanSekarang();
  });
}

function aturMode() {
  if (kini.mode === modeLalu) return;
  const dari = modeLalu;
  modeLalu = kini.mode;
  hotspotDipilih = null;
  if (kini.mode === 'angin') {
    bukaAngin();
    elPeta.classList.add('sembunyi');
  } else {
    if (dari === 'angin') tutupAngin();
    elPeta.classList.remove('sembunyi');
  }
}

function selaraskanSekarang() {
  aturMode();
  aturPenanda();
  aturTitikku();
  if (gayaSiap && kini.globe !== globeTerpasang) {
    globeTerpasang = kini.globe;
    peta.setProjection({ type: kini.globe ? 'globe' : 'mercator' });
  }
  // Asap: linimasa hanya diperiksa ulang selama lapisannya tampil.
  if (asapTampil() !== asapAktif) {
    if (asapAktif) matikanAsap();
    else nyalakanAsap();
  }
  aturAsapPeta();
  pakaiHotspot(adaSatelit() ? hariHotspotKini() : null);
  const hs = keadaanHotspot();
  aturHotspotPeta(hs);
  aturKartuHotspot(hs);
  aturPesanAngin(hs);
  kabarHotspot(hs);
  if (asapData.status !== 'diam') {
    const st = asapData.status === 'siap' ? 'ok' : asapData.status === 'galat' ? 'galat' : 'memuat';
    const t = `asap-info ${st}\t${siklusAsap()}`;
    if (t !== kabarAsapLalu) {
      kabarAsapLalu = t;
      kirim(t);
    }
  }
  elLogo.classList.toggle('tampil', asapTampil());
  gambarBilah();
}

/** Pindahkan kamera di mode mana pun: peta MapLibre atau Windy. */
function terbangKe(lat, lng, lebar) {
  const zoom = zoomLebar(lebar);
  if (kini.mode === 'angin' && jembatan.iframe) {
    jembatan.kamera = { lat, lng, zoom };
    kirimIframe([pesanKamera(jembatan.kamera)]);
  } else peta.flyTo({ center: [lng, lat], zoom, duration: 600 });
}

/** Muat lagi yang gagal: ubin citra, asap, hotspot, Windy (perintah "ulang", jaringan pulih). */
function ulangiGagal() {
  if (ubinGagal.size > 0) {
    // Hanya yang masih di layar dimuat ulang; sisanya diminta lagi saat digeser ke sana.
    peta.refreshTiles('satellite-tiles', [...ubinGagal.values()]);
    ubinGagal.clear();
  }
  if (asapAktif && adaGagalAsap()) cobaLagiAsap();
  for (const k of simpananHotspot.keys()) segarkanHotspot(k);
  if (jembatan.iframe && jembatan.status === 'galat') muatAngin();
  ulangiLogo();
}

function jalankan(perintah) {
  const n = perintah && perintah.n !== undefined ? perintah.n : null;
  if (perintahLalu === undefined) {
    perintahLalu = n;
    return;
  }
  if (n === null || n === perintahLalu) return;
  perintahLalu = n;
  const jenis = perintah.jenis;
  if (jenis === 'rumah') terbangKe(PUSAT.lat, PUSAT.lng, LEBAR_NUSANTARA);
  else if (jenis === 'lokasi' && kini.lokasiku) terbangKe(kini.lokasiku.lat, kini.lokasiku.lng, LEBAR_LOKASI);
  else if (jenis === 'paskan') paskan(kini.pulau ? kini.laporan.filter((b) => b.pulau === kini.pulau) : kini.laporan);
  else if (jenis === 'ulang') ulangiGagal();
}

/**
 * Bahasa berganti: teks tetap halaman (data-teks, data-teks-label), label penanda laporan, judul iframe Windy.
 * Bilah waktu dan kabar ke Contract digambar/dikirim ulang oleh selaraskanSekarang sesudahnya.
 */
function terapkanBahasa() {
  document.documentElement.lang = bahasa;
  for (const el of document.querySelectorAll('[data-teks]')) el.textContent = teks(el.dataset.teks);
  for (const el of document.querySelectorAll('[data-teks-label]')) el.setAttribute('aria-label', teks(el.dataset.teksLabel));
  for (const p of pin) p.el.setAttribute('aria-label', labelPin(p.isi));
  if (jembatan.iframe) jembatan.iframe.title = teks('judulAngin');
}

window.terima = function (teksJson) {
  let d;
  try {
    d = JSON.parse(teksJson);
  } catch (e) {
    return;
  }
  if (!d || typeof d !== 'object') return;
  // Bahasa dulu: kabar yang dikirim selama terima ini sudah memakai bahasa baru.
  const b = d.bahasa === 'en' ? 'en' : 'id';
  const bahasaBerubah = b !== bahasa;
  if (bahasaBerubah) {
    bahasa = b;
    terapkanBahasa();
  }
  kini.mode = d.mode === 'asap' || d.mode === 'angin' ? d.mode : 'laporan';
  const laporan = (Array.isArray(d.laporan) ? d.laporan : [])
    .map((x) => (Array.isArray(x) ? { id: String(x[0]), lat: Number(x[1]), lng: Number(x[2]), pulau: x[3] == null ? '' : String(x[3]) } : null))
    .filter((b) => b && Number.isFinite(b.lat) && Number.isFinite(b.lng) && (b.lat || b.lng));
  const kunci = JSON.stringify(laporan);
  if (kunci !== kini.kunciLaporan) {
    kini.kunciLaporan = kunci;
    kini.laporan = laporan;
  }
  kini.pulau = typeof d.pulau === 'string' ? d.pulau : '';
  kini.titik = d.titik !== false;
  kini.asap = d.asap !== false;
  kini.satelit = [0, 1, 2].map((i) => Array.isArray(d.satelit) && !!d.satelit[i]);
  kini.angin = d.angin !== false;
  kini.globe = !!d.globe;
  kini.gaya = d.gaya === 1 || d.gaya === 2 ? d.gaya : 0;
  kini.tema = d.tema === 'dark' ? 'dark' : 'light';
  kini.padBawah = Math.max(0, Number(d.padBawah) || 0);
  const lok = Array.isArray(d.lokasiku) && Number.isFinite(+d.lokasiku[0]) && Number.isFinite(+d.lokasiku[1]) ? { lat: +d.lokasiku[0], lng: +d.lokasiku[1] } : null;
  const lokBerubah = JSON.stringify(lok) !== JSON.stringify(kini.lokasiku);
  kini.lokasiku = lok;
  const pilih = d.pilih == null ? '' : String(d.pilih);
  if (pilih !== kini.pilih) {
    kini.pilih = pilih;
    if (pilih) hotspotDipilih = null;
    sorot();
  } else if ((lokBerubah || bahasaBerubah) && pilih) kirim(`jarak ${jarakPilih()}`);
  // Langsung, bukan dikumpulkan: perintah sesudahnya (mis. "rumah" di mode yang baru dibuka) melihat mode baru.
  selaraskanSekarang();
  // Kartu hotspot yang masih terbuka ditulis ulang dalam bahasa baru (hotspot-info & asap-info sudah ikut di atas).
  if (bahasaBerubah && hotspotDipilih && hotspotDipilih === keadaanHotspot().data && hotspotDipilih.titik[titikDipilih]) {
    kirim(pesanHotspotTitik(hotspotDipilih, titikDipilih));
  }
  jalankan(d.perintah);
};

/* ——— Latar belakang & jaringan (AppState, saatJaringanPulih) ——— */

document.addEventListener('visibilitychange', () => {
  if (!document.hidden) {
    // Yang gagal (mungkin selagi tersembunyi) dicoba lagi sekarang; selain itu cukup periksa bila basi.
    if (asapAktif) {
      if (adaGagalAsap()) cobaLagiAsap();
      else segarkanAsapBilaBasi();
    }
    for (const k of simpananHotspot.keys()) segarkanHotspot(k);
  }
  selaraskanSekarang();
});

window.addEventListener('online', () => {
  versiPulih++;
  ulangiGagal();
});

selaraskanSekarang();
// Poppins termuat belakangan: tinggi bilah diukur ulang.
if (document.fonts) document.fonts.ready.then(() => selaraskan());
