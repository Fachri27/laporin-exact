# Laporin di Exact

Tampilan laporin-apps (bahasa rupa SAKSI) dibangun ulang dengan [Exact](https://github.com/expo/exact) untuk
Android, dengan inti Rust sebagai backend di HP. Exact dipakai lewat path dari `../exact`.

| Berkas | Isi |
|---|---|
| `app.contract` | Seluruh tampilan: tab Beranda, Peta (hotspot), Live, Profil; Detail, Cari, Lapor, Terkirim |
| `app.tamber` | Token warna SAKSI terang/gelap dan ikon kustom |
| `data/` | Inti Rust: umpan dan komentar (fire.nusantara.earth), olahan hotspot (geoplatform), kirim laporan ke server (`kirim.rs`), salinan untuk Profil (`app:/data`) |
| `android/` | Pustaka yang dimuat pembaca Canvas Exact (`libexactcanvas.so`) |
| `tools/garis_ke_isi.py` | Ubah ikon bergaris (`tools/ikon-garis/`) jadi path terisi yang bisa digambar Android |

## Kirim laporan ke mock-server

Lapor mengirim ke mock-server laporin-apps, dengan aturan yang sama dengan server sungguhan: judul,
deskripsi, 1–6 foto bukti, tiket captcha untuk tamu. Inti Rust menjalankan rantainya: `POST /api/mobile/tiket`
→ perkecil tiap foto (`fs.compressImage`, ≤ 2048 px dan 1,5 MB, tanpa EXIF/GPS) → `POST /api/mobile/laporan`
(multipart) → salinan ke `app:/data/laporan-saya.json` untuk Profil.

```sh
cd ../laporin-apps && npm run mock          # http://0.0.0.0:4000
adb reverse tcp:4000 tcp:4000               # 127.0.0.1:4000 di emulator = Mac
```

Laporan yang diterima ada di `../laporin-apps/mock-server/unggahan/<waktu>/`. Alamat lain (mis. IP laptop untuk
HP sungguhan): `LAPORIN_API=http://192.168.1.10:4000 bun exact.mjs android`. Alamat ini dipanggang saat build,
termasuk grant `net.fetch`-nya.

Foto dipilih dari galeri lewat `@exact/photos`. Dialog izinnya muncul saat formulir dibuka: galeri dibaca dengan
`limit` baru di `bukaLapor`, karena bacaan saat boot mendahului terikatnya modul foto native dan tertahan
(Exact pre-1.0).

## Masuk uji dan peta

- **Masuk uji** (Profil): `POST /api/mobile/masuk/uji` ke mock-server; sesi `{token, nama, email}` disimpan inti
  Rust di rahasia aplikasi (`secret.keep sesi-laporin`). Setelah masuk, Profil membaca `/api/mobile/saya` dan
  `/saya/laporan`, dan Lapor mengirim dengan Bearer token tanpa captcha (kecuali anonim).
- **Peta**: halaman `https://laporin.app/id/embed` di WebView, lewat paket lokal `paket/web` (tag `web-halaman`,
  Kotlin `paket/web/android/Web.kt`). Chip *Ringkasan hotspot* tetap menampilkan olahan Rust dari geoplatform.
- Setelah mengubah `paket/` atau `node_modules`, hapus `target/android` sebelum `bun exact.mjs android`: build
  APK tidak melihat perubahan di sana.

## Menjalankan

```sh
export PATH="$HOME/.bun-1.4.2/bin:$HOME/.cargo/bin:$HOME/Library/Android/sdk/platform-tools:$PATH"
export ANDROID_HOME="$HOME/Library/Android/sdk" ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.1.12297006"
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ANDROID_SERIAL=emulator-5554

bun exact.mjs android --run       # build APK, pasang, buka
bun exact.mjs test android        # app.test.contract di emulator
cargo test -p laporin-exact-data  # uji inti Rust di Mac
cargo run -p laporin-exact-data --example periksa -- umpan berkas.json   # olah JSON asli
```

Build pertama sekitar 6 menit; berikutnya belasan detik. Perubahan `app.contract` atau `data/` selalu butuh
APK baru.

## Batasan Exact di Android (per November 2026)

Tampilan mengikuti laporin-apps layar per layar (dibandingkan dengan screenshot aplikasi aslinya di emulator). Yang
belum bisa sama:

- Tampilan digambar ke Canvas; yang berupa View asli hanya kolom teks, bilah, dialog, menu, WebView peta.
- Gambar hanya JPEG/PNG/SVG: sampul memakai `/_next/image` (membalas JPEG), bukan WebP.
- Video tidak diputar (kartu menampilkan sampul + pil senyap), kamera belum ada (Ambil foto membuka galeri),
  media Detail digeser dengan ketuk kiri/kanan (scroll-snap belum dibaca), siaran live (LiveKit) belum ada.
- Ikon status bar Android mengikuti skema aplikasi, jadi strip status bar di Detail tetap terang (aslinya gelap).
- Masuk Google dan email belum ada; Masuk uji ke mock-server saja. Komentar dikirim ke mock-server, tetapi daftar
  komentar dibaca dari fire.nusantara.earth, jadi komentar baru hanya tampil di HP yang mengirimnya.
- APK masih build pengembangan (kunci debug, tanpa ikon peluncur).
