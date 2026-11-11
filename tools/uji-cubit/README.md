# Uji cubit-zoom di emulator

Emulator image Play Store tidak memberi root (`sendevent` ditolak), `adb emu event send` tidak sampai ke layar sentuh
virtio, dan `PinchZoom` monkey selesai dalam ±60 ms (terlalu cepat untuk hatch). `Cubit.java` menyuntik dua jari
sungguhan lewat `InputManagerGlobal.injectInputEvent`, seperti perintah `input` bawaan.

```sh
SDK=~/Library/Android/sdk; JBR="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
"$JBR/bin/javac" -source 11 -target 11 -cp $SDK/platforms/android-36.1/android.jar -d /tmp/cubit tools/uji-cubit/Cubit.java
JAVA_HOME="$JBR" $SDK/build-tools/36.1.0/d8 --lib $SDK/platforms/android-36.1/android.jar --output /tmp/cubit /tmp/cubit/Cubit.class
adb push /tmp/cubit/classes.dex /data/local/tmp/cubit.dex
# pusat (540,1300), jarak jari 200 → 700 px, 30 langkah × 16 ms (perbesar); tukar jaraknya untuk memperkecil
adb shell CLASSPATH=/data/local/tmp/cubit.dex app_process /system/bin Cubit 540 1300 200 700 30 16
```
