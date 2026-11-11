#!/bin/sh
# Android App Bundle untuk Google Play: paket earth.nusantara.fire.lapor (listing aplikasi asli laporin-apps),
# ditandatangani upload key, profil Rust `release` (LTO), library sejajar 16 KB, Android 10+ (app.json).
# Butuh tambalan Exact (sekali per checkout ../exact):
#   git -C ../exact apply ../laporin-exact/tools/exact-rilis-android.patch
#   git -C ../exact apply ../laporin-exact/tools/exact-notifikasi-android.patch
#
#   sh tools/rilis-play.sh                 build AAB saja
#   sh tools/rilis-play.sh --kirim         build lalu unggah ke Play
#   sh tools/rilis-play.sh --kirim-saja    unggah AAB yang sudah dibuild (dist/laporin-<versi>-<kode>.aab), tanpa build
#
# Kredensial upload paling mudah dari EAS: di laporin-apps `npx eas-cli credentials -p android` › production ›
# credentials.json › Download credentials from EAS to credentials.json. Skrip lalu membaca jalur keystore, alias dan
# kedua kata sandinya dari ../laporin-apps/credentials.json (diabaikan git di sana), tanpa perlu diketik.
#
# Yang tidak diisi lewat lingkungan ditanyakan di terminal: LAPORIN_UPLOAD_KEYSTORE (jalur .jks), LAPORIN_VERSION_CODE,
# LAPORIN_VERSION_NAME, EXACT_UPLOAD_KEY_ALIAS, dan kedua kata sandi (EXACT_UPLOAD_STORE_PASSWORD,
# EXACT_UPLOAD_KEY_PASSWORD) tanpa gema, jadi tidak tampil di layar dan tidak masuk riwayat shell.
#
# Tanpa --kirim: hanya build. Dengan --kirim: versionCode dicek dulu terhadap EAS, lalu AAB-nya diunggah ke Play lewat
# EAS Submit dari proyek laporin-apps (`eas.json` › submit › production: jalur internal, status draft; rilisnya tetap
# ditekan di Play Console). Butuh `npx eas-cli login` dan Google Service Account Key untuk Play di EAS
# (`npx eas-cli credentials -p android` › production › Google Service Account; bila belum ada, EAS Submit menanyakannya).
#
# Upload key aplikasi asli dipegang EAS: `npx eas-cli credentials -p android` di laporin-apps → unduh keystore beserta
# alias dan kata sandinya. LAPORIN_VERSION_CODE harus lebih besar dari unggahan terakhir paket itu di Play
# (`npx eas-cli build:version:get -p android`, atau Play Console → App bundle explorer). Kata sandi hanya lewat
# lingkungan atau terminal, tidak pernah ditulis ke berkas. Hasilnya: dist/laporin-<versionName>-<versionCode>.aab,
# dengan sidik jari SHA-1 tanda tangannya untuk dicocokkan dengan Play Console › Integritas aplikasi › Sertifikat upload.
set -eu
cd "$(dirname "$0")/.."
KIRIM=0
BANGUN=1
for a in "$@"; do
  case "$a" in
    --kirim) KIRIM=1 ;;
    --kirim-saja) KIRIM=1; BANGUN=0 ;;
    *) echo "argumen tidak dikenal: $a (hanya --kirim atau --kirim-saja)" >&2; exit 1 ;;
  esac
done
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
# Gema terminal selalu dinyalakan lagi, juga bila dibatalkan dengan Ctrl-C saat mengetik kata sandi.
if [ -t 0 ]; then trap 'stty echo 2>/dev/null' EXIT; trap 'stty echo 2>/dev/null; exit 130' INT TERM; fi
# tanya NAMA "label" [rahasia]: nilai dari lingkungan, atau ditanyakan di terminal (rahasia = tanpa gema).
tanya() {
  eval "nilai=\${$1:-}"
  if [ -z "$nilai" ]; then
    [ -t 0 ] || { echo "$1 belum diisi: $2" >&2; exit 1; }
    printf '%s: ' "$2" >&2
    if [ "${3:-}" = rahasia ]; then stty -echo; read -r nilai; stty echo; echo >&2; else read -r nilai; fi
    [ -n "$nilai" ] || { echo "$1 kosong" >&2; exit 1; }
  fi
  eval "$1=\$nilai"
  export "$1"
}
tanya LAPORIN_VERSION_CODE "versionCode (lebih besar dari unggahan terakhir di Play)"
case "$LAPORIN_VERSION_CODE" in *[!0-9]*) echo "versionCode harus angka: $LAPORIN_VERSION_CODE" >&2; exit 1 ;; esac
tanya LAPORIN_VERSION_NAME "Nama versi (mis. 1.1.0)"
AAB="$(pwd)/dist/laporin-$LAPORIN_VERSION_NAME-$LAPORIN_VERSION_CODE.aab"
# --kirim-saja: tanpa build, jadi tanpa keystore dan tambalan; AAB-nya harus sudah ada.
[ "$BANGUN" = 1 ] || [ -f "$AAB" ] || { echo "AAB belum ada: $AAB (build dulu: sh tools/rilis-play.sh)" >&2; exit 1; }
KREDENSIAL="$(pwd)/../laporin-apps/credentials.json"
if [ "$BANGUN" = 1 ] && [ -f "$KREDENSIAL" ]; then
  # Satu kolom android.keystore dari credentials.json EAS; kosong bila tidak ada (lalu ditanyakan seperti biasa).
  kolom() { "$HOME/.bun-1.4.2/bin/bun" -e 'const k=require(process.argv[1]).android?.keystore??{};const v=k[process.argv[2]];if(typeof v==="string")process.stdout.write(v)' "$KREDENSIAL" "$1" 2>/dev/null || true; }
  jalur="$(kolom keystorePath)"
  [ -z "$jalur" ] || case "$jalur" in /*) ;; *) jalur="$(cd ../laporin-apps && pwd)/$jalur" ;; esac
  : "${LAPORIN_UPLOAD_KEYSTORE:=$jalur}"
  : "${EXACT_UPLOAD_KEY_ALIAS:=$(kolom keyAlias)}"
  : "${EXACT_UPLOAD_STORE_PASSWORD:=$(kolom keystorePassword)}"
  : "${EXACT_UPLOAD_KEY_PASSWORD:=$(kolom keyPassword)}"
  echo "Kredensial upload dari ../laporin-apps/credentials.json"
fi
if [ "$BANGUN" = 1 ]; then
  tanya LAPORIN_UPLOAD_KEYSTORE "Jalur keystore upload (.jks)"
  case "$LAPORIN_UPLOAD_KEYSTORE" in "~/"*) LAPORIN_UPLOAD_KEYSTORE="$HOME/${LAPORIN_UPLOAD_KEYSTORE#\~/}" ;; esac
  [ -f "$LAPORIN_UPLOAD_KEYSTORE" ] || { echo "keystore tidak ada: $LAPORIN_UPLOAD_KEYSTORE" >&2; exit 1; }
  tanya EXACT_UPLOAD_KEY_ALIAS "Alias kunci"
  tanya EXACT_UPLOAD_STORE_PASSWORD "Kata sandi keystore" rahasia
  tanya EXACT_UPLOAD_KEY_PASSWORD "Kata sandi kunci" rahasia
  grep -q uploadKeystore ../exact/host/android/app/build.gradle.kts || {
    echo "tambalan rilis belum terpasang: git -C ../exact apply ../laporin-exact/tools/exact-rilis-android.patch" >&2; exit 1; }
  # Tanpa tambalan ini izin notifikasi tidak masuk manifest: di Android 13+ notifikasi tidak pernah tampil, tanpa galat.
  grep -q 'android: &\["android.permission.POST_NOTIFICATIONS"\]' ../exact/runner/src/device.rs || {
    echo "tambalan notifikasi belum terpasang: git -C ../exact apply ../laporin-exact/tools/exact-notifikasi-android.patch" >&2; exit 1; }
fi

# Sebelum build (belasan menit): Play menolak versionCode yang tidak lebih besar dari unggahan sebelumnya.
if [ "$KIRIM" = 1 ]; then
  echo "Memeriksa versionCode terakhir di EAS…"
  versi=$(cd ../laporin-apps && npx --yes eas-cli build:version:get -p android --profile production 2>&1) || {
    echo "$versi" >&2; echo "gagal membaca versi dari EAS (sudah npx eas-cli login?)" >&2; exit 1; }
  terakhir=$(printf '%s\n' "$versi" | sed -n 's/.*versionCode - \([0-9][0-9]*\).*/\1/p' | tail -1)
  [ -n "$terakhir" ] || { echo "$versi" >&2; echo "versionCode EAS tidak terbaca" >&2; exit 1; }
  [ "$LAPORIN_VERSION_CODE" -gt "$terakhir" ] || {
    echo "LAPORIN_VERSION_CODE=$LAPORIN_VERSION_CODE harus lebih besar dari versionCode terakhir di EAS ($terakhir)" >&2; exit 1; }
  echo "versionCode EAS $terakhir → unggahan ini $LAPORIN_VERSION_CODE"
fi

if [ "$BANGUN" = 1 ]; then
  export PATH="$HOME/.cargo/bin:$HOME/.bun-1.4.2/bin:$PATH"
  export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
  export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.1.12297006}"
  export LAPORIN_API=https://fire.nusantara.earth
  # Build produksi Exact: tanpa mode pengembangan (meta-data dev.exact.development=false, activity tidak menerima kenop
  # uji lewat intent). Butuh deploy.store.linux "0" di app.json (platform bake Android = linux): tanpa updater OTA Exact,
  # pembaruan hanya lewat Play.
  export EXACT_UPDATE_TRUST=production
  # Client ID web Google (audiens ID token Masuk dengan Google) dari .env laporin-apps, seperti build preview.
  export LAPORIN_GOOGLE_KLIEN="${LAPORIN_GOOGLE_KLIEN:-$(grep '^EXPO_PUBLIC_GOOGLE_WEB_CLIENT_ID=' ../laporin-apps/.env | cut -d= -f2- | tr -d '"[:space:]')}"
  [ -n "$LAPORIN_GOOGLE_KLIEN" ] || { echo "LAPORIN_GOOGLE_KLIEN kosong" >&2; exit 1; }

  # Paket dan versi untuk Play; tanpa akhiran .preview milik build uji.
  unset ORG_GRADLE_PROJECT_idSuffix
  export ORG_GRADLE_PROJECT_releaseApplicationId=earth.nusantara.fire.lapor
  export ORG_GRADLE_PROJECT_versionCode="$LAPORIN_VERSION_CODE"
  export ORG_GRADLE_PROJECT_versionName="$LAPORIN_VERSION_NAME"
  export ORG_GRADLE_PROJECT_uploadKeystore="$(cd "$(dirname "$LAPORIN_UPLOAD_KEYSTORE")" && pwd)/$(basename "$LAPORIN_UPLOAD_KEYSTORE")"
  # Paket lokal (paket/*) disalin bun ke node_modules: salinan lama membuat build memakai Kotlin yang usang.
  bun install --force
  bun exact.mjs android --bundle --release
  mkdir -p dist
  cp target/android/app/outputs/bundle/release/app-release.aab "$AAB"
fi

echo "AAB: $AAB ($(date -r "$AAB" '+%d %b %H:%M'))"
# Harus sama dengan Play Console › Integritas aplikasi › Sertifikat kunci upload; beda = Play menolak unggahannya.
"$JAVA_HOME/bin/keytool" -printcert -jarfile "$AAB" | grep -E 'Owner:|SHA1:|SHA256:' | sed 's/^[[:space:]]*/  /'
[ "$KIRIM" = 1 ] || exit 0

echo "Mengunggah ke Google Play (jalur internal, draft) lewat EAS Submit…"
(cd ../laporin-apps && npx --yes eas-cli submit -p android --profile production --path "$AAB" --wait)
# Build lokal tidak menaikkan penghitung EAS; tanpa ini build EAS laporin-apps berikutnya memakai angka yang sudah dipakai.
echo "Selesai. Samakan penghitung EAS: cd ../laporin-apps && npx eas-cli build:version:set -p android (isi $LAPORIN_VERSION_CODE)"
