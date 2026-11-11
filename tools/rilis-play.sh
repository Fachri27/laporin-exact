#!/bin/sh
# Android App Bundle untuk Google Play: paket earth.nusantara.fire.lapor (listing aplikasi asli laporin-apps),
# ditandatangani upload key, profil Rust `release` (LTO), library sejajar 16 KB, Android 10+ (app.json).
# Butuh tambalan Exact (sekali per checkout ../exact):
#   git -C ../exact apply ../laporin-exact/tools/exact-rilis-android.patch
#   git -C ../exact apply ../laporin-exact/tools/exact-notifikasi-android.patch
#
#   LAPORIN_UPLOAD_KEYSTORE=/jalur/upload.jks LAPORIN_VERSION_CODE=12 \
#   EXACT_UPLOAD_STORE_PASSWORD=… EXACT_UPLOAD_KEY_ALIAS=… EXACT_UPLOAD_KEY_PASSWORD=… sh tools/rilis-play.sh [--kirim]
#
# Tanpa --kirim: hanya build. Dengan --kirim: versionCode dicek dulu terhadap EAS, lalu AAB-nya diunggah ke Play lewat
# EAS Submit dari proyek laporin-apps (`eas.json` › submit › production: jalur internal, status draft; rilisnya tetap
# ditekan di Play Console). Butuh `npx eas-cli login` dan Google Service Account Key untuk Play di EAS
# (`npx eas-cli credentials -p android` › production › Google Service Account; bila belum ada, EAS Submit menanyakannya).
#
# Upload key aplikasi asli dipegang EAS: `npx eas-cli credentials -p android` di laporin-apps → unduh keystore beserta
# alias dan kata sandinya. LAPORIN_VERSION_CODE harus lebih besar dari unggahan terakhir paket itu di Play
# (`npx eas-cli build:version:get -p android`, atau Play Console → App bundle explorer). Kata sandi hanya lewat
# lingkungan, tidak pernah ditulis ke berkas. Hasilnya: dist/laporin-<versionName>-<versionCode>.aab.
set -eu
cd "$(dirname "$0")/.."
KIRIM=0
for a in "$@"; do
  case "$a" in
    --kirim) KIRIM=1 ;;
    *) echo "argumen tidak dikenal: $a (hanya --kirim)" >&2; exit 1 ;;
  esac
done
: "${LAPORIN_UPLOAD_KEYSTORE:?jalur keystore upload key (.jks/.keystore)}"
: "${LAPORIN_VERSION_CODE:?versionCode, lebih besar dari unggahan terakhir di Play}"
: "${EXACT_UPLOAD_STORE_PASSWORD:?kata sandi keystore}"
: "${EXACT_UPLOAD_KEY_ALIAS:?alias kunci}"
: "${EXACT_UPLOAD_KEY_PASSWORD:?kata sandi kunci}"
grep -q uploadKeystore ../exact/host/android/app/build.gradle.kts || {
  echo "tambalan rilis belum terpasang: git -C ../exact apply ../laporin-exact/tools/exact-rilis-android.patch" >&2; exit 1; }
# Tanpa tambalan ini izin notifikasi tidak masuk manifest: di Android 13+ notifikasi tidak pernah tampil, tanpa galat.
grep -q 'android: &\["android.permission.POST_NOTIFICATIONS"\]' ../exact/runner/src/device.rs || {
  echo "tambalan notifikasi belum terpasang: git -C ../exact apply ../laporin-exact/tools/exact-notifikasi-android.patch" >&2; exit 1; }
[ -f "$LAPORIN_UPLOAD_KEYSTORE" ] || { echo "keystore tidak ada: $LAPORIN_UPLOAD_KEYSTORE" >&2; exit 1; }

# Sebelum build (belasan menit): Play menolak versionCode yang tidak lebih besar dari unggahan sebelumnya.
if [ "$KIRIM" = 1 ]; then
  echo "Memeriksa versionCode terakhir di EAS…"
  versi=$(cd ../laporin-apps && npx --yes eas-cli build:version:get -p android --profile production 2>&1) || {
    echo "$versi" >&2; echo "gagal membaca versi dari EAS (sudah npx eas-cli login?)" >&2; exit 1; }
  terakhir=$(printf '%s\n' "$versi" | sed -n 's/.*versionCode - \([0-9][0-9]*\).*/\1/p' | tail -1)
  [ -n "$terakhir" ] || { echo "$versi" >&2; echo "versionCode EAS tidak terbaca" >&2; exit 1; }
  [ "$LAPORIN_VERSION_CODE" -gt "$terakhir" ] || {
    echo "LAPORIN_VERSION_CODE=$LAPORIN_VERSION_CODE harus lebih besar dari versionCode terakhir di EAS ($terakhir)" >&2; exit 1; }
  echo "versionCode EAS $terakhir → build $LAPORIN_VERSION_CODE"
fi

export PATH="$HOME/.cargo/bin:$HOME/.bun-1.4.2/bin:$PATH"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.1.12297006}"
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
export LAPORIN_API=https://fire.nusantara.earth
# Client ID web Google (audiens ID token Masuk dengan Google) dari .env laporin-apps, seperti build preview.
export LAPORIN_GOOGLE_KLIEN="${LAPORIN_GOOGLE_KLIEN:-$(grep '^EXPO_PUBLIC_GOOGLE_WEB_CLIENT_ID=' ../laporin-apps/.env | cut -d= -f2- | tr -d '"[:space:]')}"
[ -n "$LAPORIN_GOOGLE_KLIEN" ] || { echo "LAPORIN_GOOGLE_KLIEN kosong" >&2; exit 1; }

# Paket dan versi untuk Play; tanpa akhiran .preview milik build uji.
unset ORG_GRADLE_PROJECT_idSuffix
NAMA_VERSI="${LAPORIN_VERSION_NAME:-1.0.0}"
export ORG_GRADLE_PROJECT_releaseApplicationId=earth.nusantara.fire.lapor
export ORG_GRADLE_PROJECT_versionCode="$LAPORIN_VERSION_CODE"
export ORG_GRADLE_PROJECT_versionName="$NAMA_VERSI"
export ORG_GRADLE_PROJECT_uploadKeystore="$(cd "$(dirname "$LAPORIN_UPLOAD_KEYSTORE")" && pwd)/$(basename "$LAPORIN_UPLOAD_KEYSTORE")"
# Paket lokal (paket/*) disalin bun ke node_modules: salinan lama membuat build memakai Kotlin yang usang.
bun install --force
bun exact.mjs android --bundle --release

AAB="$(pwd)/dist/laporin-$NAMA_VERSI-$LAPORIN_VERSION_CODE.aab"
mkdir -p dist
cp target/android/app/outputs/bundle/release/app-release.aab "$AAB"
echo "AAB: $AAB"
[ "$KIRIM" = 1 ] || exit 0

echo "Mengunggah ke Google Play (jalur internal, draft) lewat EAS Submit…"
(cd ../laporin-apps && npx --yes eas-cli submit -p android --profile production --path "$AAB" --wait)
# Build lokal tidak menaikkan penghitung EAS; tanpa ini build EAS laporin-apps berikutnya memakai angka yang sudah dipakai.
echo "Selesai. Samakan penghitung EAS: cd ../laporin-apps && npx eas-cli build:version:set -p android (isi $LAPORIN_VERSION_CODE)"
