# Laporin di Exact

Tampilan laporin-apps (bahasa rupa SAKSI) dibangun ulang dengan [Exact](https://github.com/expo/exact) untuk
Android, dengan inti Rust sebagai backend di HP. Exact dipakai lewat path dari `../exact`.

| Berkas | Isi |
|---|---|
| `app.contract` | Seluruh tampilan: tab Beranda, Peta (hotspot), Live, Profil; Detail, Cari, Lapor, Terkirim |
| `app.tamber` | Token warna SAKSI terang/gelap dan ikon kustom |
| `data/` | Inti Rust: umpan dan komentar (fire.nusantara.earth), olahan hotspot (geoplatform), laporan tersimpan (`app:/data`) |
| `android/` | Pustaka yang dimuat pembaca Canvas Exact (`libexactcanvas.so`) |
| `tools/garis_ke_isi.py` | Ubah ikon bergaris (`tools/ikon-garis/`) jadi path terisi yang bisa digambar Android |

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

- Tampilan digambar ke Canvas; yang berupa View asli hanya kolom teks, bilah, dialog dan menu.
- Gambar hanya JPEG/PNG/SVG: sampul memakai `/_next/image` (membalas JPEG), bukan WebP.
- Belum ada peta interaktif, video, kamera untuk lampiran, atau LiveKit: tab Peta menampilkan ringkasan
  hotspot, Live hanya banner, dan laporan disimpan lokal (mode demo).
- APK masih build pengembangan (kunci debug, tanpa ikon peluncur).
