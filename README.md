# Laporin di Exact

Tampilan laporin-apps (bahasa rupa SAKSI) dibangun ulang dengan [Exact](https://github.com/expo/exact) untuk
Android, dengan inti Rust sebagai backend di HP. Exact dipakai lewat path dari `../exact`.

| Berkas | Isi |
|---|---|
| `app.contract` | Seluruh tampilan: tab Beranda, Peta (Di sekitar Anda: Laporan, Aerosol Karhutla, Angin dan Kualitas Udara), Live, Profil; Detail, Cari, Lapor, Terkirim |
| `app.tamber` | Token warna SAKSI terang/gelap dan ikon kustom |
| `data/` | Inti Rust: umpan dan komentar (`LAPORIN_SITUS`), akun & suka (`akun.rs`), kirim laporan ke server (`kirim.rs`), token dan daftar siaran langsung (`siaran.rs`), salinan untuk Profil (`app:/data`) |
| `android/` | Pustaka yang dimuat pembaca Canvas Exact (`libexactcanvas.so`) |
| `paket/web`, `paket/video` | Paket lokal Kotlin: WebView peta (`web-halaman`, dengan pesan dua arah dan sentuhan dari hatch) dan pemutar video (`video-putar`, dengan lompat dan laporan waktu) |
| `paket/bahasa` | View `atur-bahasa`: tombol ID/EN menjadi locale aplikasi Android (LocaleManager di Android 13+), jadi `t()` berganti tabel di tempat |
| `paket/notifikasi` | View `atur-notifikasi` + pekerja WorkManager `CekKabar`: notifikasi siaran langsung dan laporan baru tanpa Firebase (lihat Notifikasi) |
| `strings/id.json`, `strings/en.json` | Semua teks tampilan (`t("area.kunci", nama=…)`); `id` tabel dasar (`app.json` → `strings.base`), `en` terjemahannya, kuncinya sama |
| `modules/android/Hatches.kt` | Hatch `sentuh-web`: meneruskan pointer di atas peta ke WebView-nya (host Android Exact tidak menyentuhkan apa pun ke view native) |
| `assets/peta/` | Halaman peta tab Peta (`sekitar.html`: citra satelit, titik laporan, asap CAMS, hotspot FIRMS, bilah waktu, Windy) dengan MapLibre GL JS 5.24 (BSD-3, `LICENSE-maplibre-gl.txt`) |
| `assets/live/` | Halaman siaran (`siar.html`) dan tonton/pratinjau (`tonton.html`), bersama `bersama.js` (peran, daftar penonton, label kualitas) dan `hati.js` (hati melayang), dengan livekit-client 2.23 (Apache-2.0, `LICENSE-livekit-client.txt`) |
| `gradle.lockfile`, `gradle-verification.xml` | Kunci dan checksum Maven untuk CameraX (`@exact/camera`) dan WorkManager (`paket/notifikasi`); tulis ulang dengan `bun exact.mjs android --write-locks` bila paket berpustaka Maven berubah |
| `tools/garis_ke_isi.py` | Ubah ikon bergaris (`tools/ikon-garis/`) jadi path terisi yang bisa digambar Android |
| `tools/rilis-play.sh` | AAB Google Play (`earth.nusantara.fire.lapor`, upload key EAS); `--kirim` sekalian mengunggahnya lewat EAS Submit (lihat Rilis Google Play) |
| `tools/glyph_ke_svg.py` | Ambil glyph font ikon (Feather.ttf laporin-apps) sebagai path terisi, mis. gerigi Pengaturan `icons/atur.svg` |

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

Siaran langsung butuh mock-server yang memakai LiveKit Cloud (`wss://`): WebView menolak `ws://` polos, jadi
`livekit-server --dev` lokal tidak bisa dipakai. Secret-nya hanya di environment mock-server, tidak di aplikasi:

```sh
LIVEKIT_URL=wss://<proyek>.livekit.cloud LIVEKIT_API_KEY=… LIVEKIT_API_SECRET=… npm run mock
PORT=4000 npm run simulasi-live -- 1        # opsional: satu "warga lain" sedang live (Chrome headless)
```

Umpan, gambar dan komentar dibaca dari `LAPORIN_SITUS`, bawaannya sama dengan `LAPORIN_API` (seperti `.env`
laporin-apps): mock-server meneruskan fire.nusantara.earth dan menambahkan suka & komentar mock, jadi komentar
dan suka dari satu HP terlihat di HP lain. Tanpa mock-server: `LAPORIN_SITUS=https://fire.nusantara.earth`.

Foto dipilih dari galeri lewat `@exact/photos`. Dialog izinnya muncul saat formulir dibuka: galeri dibaca dengan
`limit` baru di `bukaLapor`, karena bacaan saat boot mendahului terikatnya modul foto native dan tertahan
(Exact pre-1.0).

## Bahasa (ID/EN)

Tombol bahasa di Beranda mengganti seluruh aplikasi tanpa memuat ulang. `t()` Contract membaca locale dari host,
bukan dari state, jadi `atur-bahasa bahasa=bahasa` (paket/bahasa) mengatur locale aplikasi Android dan host Exact
menukar tabel `strings/` di tempat. `t()` tidak boleh di `fn` tingkat atas (`type-strings-missing`): pilihan teks
ada di komponen, atau `fn` menerima teks yang sudah diterjemahkan sebagai parameter. Inti Rust mendapat bahasanya
lewat `aturBahasa(b)` (`mutation bahasaData … refreshes` umpan dan resource lain yang teksnya dibuat inti): tanggal,
koordinat (LS/BT → S/E), label bawaan, galat buatan aplikasi, dan umpan diminta `?bahasa=en` (server membalas judul,
deskripsi dan tanggal berbahasa Inggris). Halaman peta dan live membaca `"bahasa"` di JSON `isi`-nya dan berganti di
tempat. Tetap berbahasa server/aslinya: komentar, nama, galat dari server, legenda Windy. Pesan galat yang sudah
tampil tidak ikut berganti.

## Notifikasi (tanpa Firebase)

Pengaturan › Notifikasi: sakelar *Siaran langsung* dan *Laporan baru*, seperti laporin-apps. Tanpa FCM server tidak
bisa membangunkan HP, jadi HP sendiri yang memeriksa: `atur-notifikasi` (root `app.contract`) menyimpan pilihan, bahasa
dan alamat dari `alamatKabar()` (inti Rust) ke preferensi, lalu menjadwalkan `CekKabar` di WorkManager: alarm tidak
eksak tiap 90 detik (`AlarmKabar`, pertama 1 menit setelah aplikasi dibuka; Android boleh mengundurnya 75% jedanya,
jadi tiap 1,5–2,6 menit, dan membatasi alarm aplikasi yang jarang dibuka, mis. 10 per jam) memicu kerja *expedited* bersyarat
jaringan, dan kerja berkala 15 menit sebagai cadangan yang memasang lagi alarm yang hilang. Kerja biasa tidak cukup:
kerja berkala minimal 15 menit, Android 15+ memblokir jaringan aplikasi latar di luar kerja bersyarat jaringan
(`blocked=APP_BACKGROUND`), dan Android 16 menahan kerja bersyarat jaringan biasa sampai radio sedang aktif (hingga 31
menit). Doze/hemat baterai tetap bisa menundanya selama HP lama tidak dipakai. Pekerja membaca
`GET /api/mobile/live` setiap kali dan `GET /api/umpan/halaman` (15 laporan terbaru, ±7 KB gzip, bukan seluruh umpan
51 KB) paling sering tiap 15 menit, membandingkan dengan id yang sudah dilihat, lalu menampilkan notifikasi lokal, juga
saat aplikasi terbuka (seperti aslinya):

- Kanal `live` (penting, muncul di atas layar): "🔴 Rina sedang live" + judul. Siaran yang sedang berjalan saat
  pemeriksaan pertama juga dikabarkan. Ditarik lagi begitu siarannya selesai.
- Kanal `laporan`: "Laporan karhutla terverifikasi" + judul · provinsi, dengan sampulnya (`/_next/image`). Lebih dari 3
  sekaligus jadi satu ringkasan. Laporan yang sudah ada saat pemeriksaan pertama hanya dicatat, dan yang tidak tampil di
  Beranda (tanpa koordinat/media) tidak dikabarkan.
- Tidak mengabari: siaran sendiri, dan yang terlewat selama notifikasi atau sakelarnya mati.
- Ketuk: `app://tonton/<ruang>` atau `app://laporan/<id>`. Host Exact menjadikannya `navigate` root (`ikuti`, yang
  menyiapkan token tonton dan memuat ulang umpan bila laporannya belum ada) atau lokasi peluncuran (task
  `tontonDariTautan`, lalu `lengkapiTonton` mengisi judul/durasi begitu daftar siaran termuat).
- Izin Android 13+ ditanyakan sekali 4 detik setelah pertama dibuka; sakelar tanpa izin memintanya lagi, dan setelah
  ditolak permanen muncul baris "Notifikasi diblokir" yang membuka pengaturan notifikasi HP. `POST_NOTIFICATIONS`
  berasal dari grant `device.notifications notifikasiAlasan`; Exact belum memetakannya ke izin Android, jadi checkout
  `../exact` perlu `tools/exact-notifikasi-android.patch` (`git apply`).
- Build dev ke mock (`http://127.0.0.1:4001`): kebijakan cleartext Android menolak HttpURLConnection, jadi pekerja
  memakai soket HTTP/1.0 untuk `http:`; produksi (`https:`) lewat HttpURLConnection.
- Paket lokal disalin bun ke `node_modules`: setelah mengubah `paket/*`, jalankan `bun install --force` sebelum build.

## Beranda seketika saat dibuka

Cache gambar Exact di Android hilang setiap aplikasi ditutup (spool per proses), dan transport HTTP-nya tidak meminta
kompresi. Maka: umpan diminta gzip (225 KB → 51 KB) dan disimpan di `app:/data/umpan.json`; `umpanTersimpan` tampil
seketika saat dibuka lalu diganti umpan baru begitu tiba (tanpa jaringan yang tersimpan tetap tampil, dengan pesan
gagal memperbarui). Sampul sorotan + 16 kartu teratas disalin ke `app:/data/sampul/` (`siapkanSampul`, file yang tak
dipakai lagi dihapus) dan dipakai `umpan` sebagai ganti alamat server. Putaran memuat hanya tampil saat belum ada
laporan sama sekali atau saat muat ulang manual. Pembukaan pertama setelah instal (belum ada umpan tersimpan)
menampilkan preload (logo + putaran, tanpa teks karena locale baru diterapkan setelah tampilan pertama) sampai umpan
tiba dan sampul sorotan + 4 kartu pertama termuat (gambar pengamat 1×1 di lapisan preload, berbagi unduhan dengan
kartu di bawahnya), umpan gagal, atau 15 detik; di emulator ±1,6–3 detik. Pada pembukaan itu kartu tetap memakai
alamat server, supaya tidak dimuat ulang dari berkas saat `siapkanSampul` (berurutan, ±8 detik) selesai.

## Akun, suka, peta dan video

- **Masuk** (Profil): *Lanjutkan dengan Google* (paket lokal `paket/masuk-google`: Credential Manager +
  `GetSignInWithGoogleOption`, idToken ditukar di `POST /api/mobile/masuk/google`) atau email (`/masuk/email`, daftar
  lewat `/daftar`). Google butuh client ID web saat build (`LAPORIN_GOOGLE_KLIEN`, sama dengan
  `EXPO_PUBLIC_GOOGLE_WEB_CLIENT_ID` laporin-apps) dan klien OAuth Android untuk paket `id.laporin.exact`
  (`.preview`) + SHA-1 kunci debug di proyek Google Cloud yang sama; sesi `{token, nama, email}` dipegang inti Rust di Store (`secret.keep sesi-laporin`)
  dan disalin ke `app:/data/sesi.json`. Host Android Exact tidak bisa menulis rahasia ke disk (jurnal: `store
  sesi-laporin failed: lock acquisition failed due to I/O error`), jadi `muatSesi()` pertama setelah aplikasi dibuka
  memulihkan sesi dari berkas itu; Keluar dan balasan 401 mengosongkannya. Setelah masuk, Profil membaca
  `/api/mobile/saya` dan `/saya/laporan`, dan Lapor mengirim dengan Bearer token tanpa captcha (kecuali anonim).
- **Suka** (Detail dan sorotan Beranda): `POST`/`DELETE /api/mobile/laporan/{id}/suka`; tab *Disukai* di Profil
  dari `/saya/disukai`. Tamu mendapat dialog "Masuk dulu" (server menolak suka tanpa akun).
- **Logo**: `Merek` menumpuk dua ikon tamber (`icon.merekLapor` berwarna teks, `icon.merekIn` oranye merek) yang
  dijiplak potrace dari LaporinLogo.png, jadi "Lapor" ikut terang/gelap; dipakai di kepala Beranda dan Profil.
- **Captcha** (laporan tamu atau anonim ke server https): lembar *Verifikasi singkat* memuat halaman Turnstile server
  (`/api/mobile/captcha`) di `web-halaman`; paket/web meneruskan `window.ReactNativeWebView.postMessage` halaman itu
  sebagai "rn <json>", tokennya ikut `kirimLaporan` dan ditukar dengan tiket di `/api/mobile/tiket`. Ke mock-server
  (http) tanpa lembar: `mock-captcha`.
- **Beranda**: ketuk ikon Beranda lagi = kembali ke atas dan muat ulang umpan. Kisi tampil 15 kartu sekali dan bertambah
  saat penanda `intersect=` di bawahnya mendekati layar, sampai semua laporan umpan tampil. Sampul 15 kartu berikutnya
  dipramuat sebagai gambar 1×1 tak terlihat (cache unduhan Exact) selagi 15 kartu yang tampil digulir.
  Kisi dibagi potong 15 kartu (`PotongKisi`, kolomnya rata per potong); kartu satu potong hanya terpasang selama
  potong itu ±800 px dari layar, lebih jauh potong menjadi kotak kosong setinggi ukuran terakhirnya (`resize=`).
  Tiap gambar terpasang tetap memegang sumbernya di pipeline gambar Exact, dan tanpa pembatas ini sampul setelah
  ±100 kartu tidak pernah dimuat (terlihat juga di build lama; dugaan: penolakan `QueueFull` di
  `host/linux/src/image/workers.rs`).
- **Gulir Beranda ringan**: setiap aksi Contract (commit) menggambar ulang di thread `exact`, yang juga menggerakkan
  gulir. Jadi selama gulir aksinya dijaga sedikit: satu pengamat per potong kisi (bukan per kartu), pramuat tanpa
  `load=`, dan video sorotan `waktu=false` (paket/video tidak mengirim "waktu" tiap 250 ms) serta berhenti selama
  di luar layar. Diukur dengan `adb shell atrace -a <paket> gfx view sched` (irisan `exact paint`/`commit`/`scroll`):
  sibuk thread `exact` selama 9 detik gulir turun dari 5,7 ke 2,3 detik, commit dari 89 ke 12. Setiap gambar ulang
  dan setiap geser (scroll anchoring, `scroll_anchor::capture`) menelusuri seluruh isi gulungan, jadi kartu di luar
  ±800 px dilepas (lihat Beranda); di tengah umpan CPU thread `exact` turun ±45% dan tak ada lagi kerja >16 ms
  (simpleperf: APK *profileable*, simbol dari `target/android/app/rust/jniLibs`). Emulator untuk mengukur: 4 core,
  RAM 4 GB (1 core membuat thread mengantre CPU, bukan bekerja). Sorotan memakai poster video pertamanya, jadi gambar penutup sama dengan bingkai video.
- **Verifikasi KTP** (syarat siaran; Pengaturan, baris di Siapkan siaran, dan otomatis saat *Mulai siaran* belum
  terverifikasi): NIK + foto KTP dari kamera mode KTP (bingkai kartu 85,6:54, periksa foto: Ulangi/Pakai), diperkecil
  `fs.compressImage` lalu `POST /api/mobile/saya/verifikasi` (multipart `nik`, `ktp`); status belum/menunggu/
  ditolak/disetujui dari `GET` yang sama (`data/src/verifikasi.rs`).
- **Video**: paket lokal `paket/video` (ExoPlayer AndroidX Media3 di TextureView dengan tembolok disk 250 MB dan
  penyangga 2–10 detik seperti expo-video aslinya, Kotlin `paket/video/android/Video.kt`), diputar
  langsung dari `https://fire.nusantara.earth/media/…` karena tumpukan jaringan Android menolak HTTP polos ke
  mock-server. Kartu sorotan Beranda memutar video laporannya tanpa suara selama Beranda tampil (tombol suara di
  kanan atas). Di Detail dan layar penuh (`PemutarVideo`): ketuk video = kontrol gaya Netflix seperti aslinya
  (mundur/maju 10 detik, putar/jeda, penggeser posisi dan waktu), hilang sendiri setelah 3 detik berjalan; tanpa
  kontrol hanya garis tipis posisi.
- **Kamera** (Lapor → Ambil foto): `@exact/camera` di atas `@exact/media-capture` (CameraX). Jendela bidik penuh,
  bingkai bidik dan pesan keselamatan, lampu kilat, balik kamera depan/belakang, rana; setiap foto
  (`app:/tmp/camera/N.jpg`) langsung jadi lampiran, foto terakhir tampil di kiri bawah, Selesai kembali ke formulir.
  Izin kamera diminta saat pertama dibuka (grant `device.camera kameraAlasan` di inti Rust).
- **Live** (LiveKit, seperti laporin-apps `docs/LIVE.md`): tab Live mendaftar siaran publik
  (`GET /api/mobile/live`, tiap 8 detik selama tab Live atau Beranda tampil); inti Rust juga meminta token pratinjau
  tiap siaran (`POST /api/mobile/live/tonton` dengan `pratinjau: true`, peserta tersembunyi), jadi kartu tab Live dan
  sorotan Beranda ("LAPORIN · N LIVE") memutar video bisu pelapor, dan ikon Live di bilah bawah bertitik. *Mulai
  live* → Siapkan siaran (GPS, judul, kualitas video: Full HD 60, Otomatis HD 60, HD 30, Hemat data; diingat) →
  `POST /api/mobile/live/token` dengan sesi (siaran publik: akun dengan KTP sah) → layar siaran dari `assets/live/siar.html` (simulcast tiga lapisan seperti PROFIL_SIARAN):
  LIVE berkedip, durasi, penonton (ketuk = lembar Penonton), pil sinyal (mutu sambungan, resolusi dan laju terkirim;
  ketuk = lembar Kualitas), lokasi, komentar berpenanda peran (PETUGAS/PELAPOR/ANDA), hati melayang, mikrofon, Akhiri
  (dengan konfirmasi; tombol kembali HP juga bertanya), balik kamera → "Terima kasih" dan *Lengkapi laporan*. Layar
  Tonton (`assets/live/tonton.html`): video, kepala, kualitas (Otomatis atau lapisan simulcast; diingat), mode
  bersih, hati (ketuk video atau tombol), lapor siaran (Laporkan dengan jenis `siaran`), komentar; pelapor yang
  putus karena sinyal ditunggu 60 detik. Token diminta inti Rust (WebView menolak `http` polos ke mock) dan dioper
  ke halaman lewat prop `isi`. `paket/web` memberi halaman lokal kamera dan mikrofon (izin Android diminta saat
  halaman memintanya) dan mengosongkan halamannya saat layar ditutup, supaya kamera dan sambungan langsung lepas.
- **Peta** (peta-laporan.tsx laporin-apps): "Di sekitar Anda" dengan tiga mode. *Laporan*: titik tetes api per sel
  kisi, saringan pulau (judul berganti, pulau lain meredup). *Aerosol Karhutla*: asap CAMS (WebGL, tiga palet) dengan
  bilah waktu tiap 3 jam, 7 hari ke belakang sampai prakiraan. *Angin dan Kualitas Udara*: Windy dari geoplatform di
  iframe. Hotspot NASA FIRMS (tiga satelit) bisa dinyalakan di lembar Lapisan di semua mode; harinya mengikuti bilah
  waktu. Tombol kanan: Lapisan, bola dunia, Nusantara, Lokasiku; kiri: Legenda. Kepala: pilih pulau, bagikan,
  Panduan. Petanya halaman lokal `assets/peta/sekitar.html` (MapLibre, citra satelit EOX lewat geoplatform) di
  `web-halaman` (`paket/web`); kepala, chip, tombol, kartu (laporan, hotspot) dan lembar digambar Contract.
  Contract mengirim keadaan peta lewat prop `isi` (JSON ke `window.terima`); halaman membalas lewat `laporin.kirim`
  (`siap`, `pilih`, `hotspot`, `hotspot-info`, `asap-info`, `bilah`, …). Halaman lokal boleh mengambil data
  geoplatform tanpa CORS (`allowUniversalAccessFromFileURLs` hanya untuk `file:///android_asset/`).
- **Sentuhan di peta**: host Android Exact hanya memanggil `performClick()` pada view native, tanpa titik sentuh,
  jadi tombol dan geseran di dalam WebView mati. Pembungkus tiap peta diberi `hatch="sentuh-web" data-kunci=…`;
  `modules/android/Hatches.kt` mendengar pointer-nya (`observe`) dan `Sentuhan.teruskan` di `paket/web` mengirimnya ke
  WebView sebagai sentuhan sungguhan. Tombol Contract di atas peta bukan anak pembungkus itu, jadi ketukannya tidak
  ikut diteruskan. Hatch hanya membawa satu jari, jadi untuk cubit-zoom `Sentuhan` juga mengamati peristiwa sentuh
  mentah jendela (`Window.Callback` dibungkus): begitu jari kedua mendarat selagi jari pertama memegang peta, semua
  jari diteruskan dari peristiwa mentah, dipetakan dari titik turun jari pertama.
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

## Rilis Google Play

Build di Mac, unggah lewat EAS Submit (EAS Build hanya untuk proyek Expo/React Native; build ini butuh `../exact`
bertambalan, Rust, NDK dan bun). Sekali per checkout `../exact`: `git apply` `tools/exact-rilis-android.patch` dan
`tools/exact-notifikasi-android.patch`. Upload key dari `npx eas-cli credentials -p android` di laporin-apps.

```sh
LAPORIN_UPLOAD_KEYSTORE=/jalur/upload.jks LAPORIN_VERSION_CODE=3 LAPORIN_VERSION_NAME=1.1.0 \
EXACT_UPLOAD_STORE_PASSWORD=… EXACT_UPLOAD_KEY_ALIAS=… EXACT_UPLOAD_KEY_PASSWORD=… sh tools/rilis-play.sh --kirim
```

Dengan `--kirim`, versionCode dicek dulu terhadap EAS (`build:version:get`; harus lebih besar), lalu
`dist/laporin-<versi>-<kode>.aab` (build produksi Exact `EXACT_UPDATE_TRUST=production`: tanpa mode pengembangan dan
tanpa updater OTA, `deploy.store.linux` "0" di app.json) diunggah dengan `eas submit --path` dari laporin-apps ke jalur internal sebagai draft
(rilisnya ditekan di Play Console). Butuh `npx eas-cli login` dan Google Service Account Key untuk Play di EAS. Build
lokal tidak menaikkan penghitung EAS: sesudahnya `npx eas-cli build:version:set -p android` di laporin-apps.

## Batasan Exact di Android (per November 2026)

Tampilan mengikuti laporin-apps layar per layar (dibandingkan dengan screenshot aplikasi aslinya di emulator). Yang
belum bisa sama:

- Tampilan digambar ke Canvas; yang berupa View asli hanya kolom teks, bilah, dialog, menu, WebView peta.
- Peta belum punya: geser kartu untuk laporan lain di titik yang sama (i/N) dan Peringkat hotspot.
- Gambar hanya JPEG/PNG/SVG: sampul memakai `/_next/image` (membalas JPEG), bukan WebP.
- Penggeser posisi video menerima ketukan saja (jempolnya melompat ke titik yang diketuk); menyeret belum bisa,
  karena `range` di host Android hanya membaca tekanan. Layar penuh memutar video dari awal.
- Kamera hanya FOTO: mode VIDEO dan LIVE belum ada; ketuk-untuk-fokus dan cubit-zoom belum ada di Android (view
  native hanya menerima klik). `object-fit` belum sampai ke view kamera native, jadi jendela bidik dibuat 3:4
  setinggi layar dan dipotong di tengah.
- Media Detail digeser dengan `pan` (media mengikuti jari, pindah setelah 48 poin atau lemparan cepat, berputar dari
  yang terakhir ke yang pertama) karena scroll-snap belum dibaca host Android; mengetuk media tidak memindahkannya
  (ketukan di video menampilkan kontrolnya).
- Tombol back HP di akar tab Peta, Live dan Profil menekan kontrol Back tak terlihat (`button id="back"`, nama dari
  `navigationBack` root; host Android menekannya sebelum keluar): tutup lembar Peta, lalu kartu Peta, lalu ke Beranda
  seperti aslinya. Di akar Beranda back tetap keluar dari aplikasi.
- Siaran (`assets/live/siar.html`) diterbitkan H.264 tanpa codec cadangan, `degradationPreference` maintain-framerate,
  seperti `OPSI_RUANG` laporin-apps: bawaan LiveKit (VP8) di WebView di-encode perangkat lunak, dan 720p/1080p 60 fps +
  simulcast membuat HP tidak menanggapi ketukan selama live. `<video>` halaman live berposter piksel transparan (tanpa
  ikon play bawaan WebView) dan pratinjau memudar masuk saat bingkai pertama diputar; sorotan live Beranda memakai
  gradasi tipis, bukan pita gelap, di bawah judulnya.
- Tema hanya diatur di Pengaturan → Tampilan (Ikuti HP · Terang · Gelap); tombol tema di kepala Beranda dihapus.
- Ukuran Contract tidak memuat padding (box-sizing CSS, beda dengan React Native aslinya): toggle Lapisan peta
  `width=40 height=22 padding=3` = 46×28.
- Simpan dan Suka butuh akun (tamu diminta masuk dulu); daftar Tersimpan hanya ada di HP ini.
- Live belum punya: slider beberapa siaran di sorotan Beranda
  (hanya yang terbaru), geser ke bawah untuk menutup Tonton, dan siaran tetap jalan saat aplikasi di latar belakang.
- Ikon status bar Android mengikuti skema aplikasi, jadi strip status bar di Detail tetap terang (aslinya gelap).
- Masuk Google hanya Android (paket lokal).
- `secret.keep` tidak tersimpan ke disk di Android (kunci berkasnya gagal); sesi bertahan lewat `app:/data/sesi.json`.
  Daftar kamera (`enumerateDevices`) dibaca saat boot sebelum modul native terikat (NotSupportedError), jadi
  dibaca lagi setelah kamera terbuka.
- APK masih build pengembangan (kunci debug). Ikon peluncur (logo Laporin, `icons` di app.json → ikon adaptif
  Android) butuh tambalan di checkout Exact, karena shell Android Exact belum punya ikon:
  `git -C ../exact apply ../laporin-exact/tools/exact-ikon-android.patch` (pasang ulang setelah Exact diperbarui).
