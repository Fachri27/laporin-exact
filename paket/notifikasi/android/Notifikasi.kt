// @laporin/notifikasi di Android: notifikasi "siaran langsung" dan "laporan baru" tanpa Firebase.
//
// Tanpa layanan push (FCM), server tidak bisa membangunkan HP. Jadi HP sendiri yang memeriksa, dengan WorkManager:
// - Cepat, untuk siaran: alarm tidak eksak 90 detik lagi ([AlarmKabar]; Android boleh mengundurnya 75% jedanya, jadi
//   tiap 1,5–2,6 menit, dan membatasi alarm aplikasi yang jarang dibuka, mis. 10 per jam) memicu kerja *expedited* bersyarat
//   jaringan. Kerja biasa tidak cukup: kerja berkala minimal 15 menit, Android 15+ memblokir jaringan aplikasi latar di
//   luar kerja bersyarat jaringan, dan Android 16 menahan kerja bersyarat jaringan biasa sampai radio sedang aktif
//   (hingga 31 menit).
// - Cadangan: kerja berkala 15 menit, yang juga memasang lagi alarm yang hilang (HP dinyalakan ulang).
// Laporan baru diperiksa paling sering tiap 15 menit. Doze dan hemat baterai tetap bisa menunda semuanya selama HP lama
// tidak dipakai. [CekKabar] membaca
// alamatnya langsung lewat HTTP (proses aplikasi bisa sedang tidak berjalan, jadi tanpa sesi Exact), membandingkannya
// dengan yang sudah pernah dilihat, lalu menampilkan notifikasi lokal, juga saat aplikasi terbuka (seperti
// laporin-apps). Siaran yang sedang berjalan langsung dikabarkan; laporan yang sudah ada saat pemeriksaan pertama hanya
// dicatat (tanpa banjir notifikasi). Siaran sendiri tidak dikabarkan ke HP sendiri.
//
// `atur-notifikasi` (view tak terlihat di root Contract) menyimpan pilihan dan alamat ke preferensi yang dibaca pekerja,
// menjadwalkan atau membatalkan pekerjaannya, melaporkan izin lewat event `message` ("diberi", "ditolak", "tanya") dan
// meminta izin saat prop `minta` bertambah: dialog Android 13+, atau pengaturan notifikasi HP bila sudah ditolak. Izin
// juga ditanyakan sekali, 4 detik setelah aplikasi pertama kali dibuka, seperti laporin-apps.
//
// Ketukan notifikasi membuka `app://laporan/<id>` atau `app://tonton/<ruang>`: host Exact menjadikannya lokasi rute
// (`navigate` root bila aplikasi berjalan, lokasi peluncuran bila tidak).
package laporin.notifikasi

import android.Manifest
import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.ActivityNotFoundException
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.drawable.Icon
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.Settings
import android.util.Log
import android.view.View
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.InetSocketAddress
import java.net.Socket
import java.net.URL
import java.net.URLEncoder
import java.util.concurrent.TimeUnit
import java.util.zip.GZIPInputStream

class NotifikasiModule(context: ExtensionContext) : ExactExtension(context) {
    private val app: Context get() = context.application
    private val prefs get() = Kabar.prefs(app)
    private val main = Handler(Looper.getMainLooper())
    private var emit: ((String, String) -> Unit)? = null
    private var izinTerakhir = ""
    private var mintaTerakhir: Double? = null
    private var dijadwalkan: Boolean? = null

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "atur-notifikasi") { "no view $tag" }
        val v = View(context)
        v.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        this.emit = emit
        izinTerakhir = ""
        // Nilai awal `minta` bukan permintaan; hanya kenaikannya sesudah ini.
        mintaTerakhir = props.optDouble("minta", 0.0)
        terapkan(props)
        v.post {
            laporIzin()
            tanyaPertama(props)
        }
        return v
    }

    override fun update(view: View, props: JSONObject) {
        terapkan(props)
        val minta = props.optDouble("minta", 0.0)
        if (minta != mintaTerakhir) {
            mintaTerakhir = minta
            view.post { mintaIzin() }
        }
    }

    override fun destroyView(view: View) {
        emit = null
    }

    override fun resumed() {
        // Kembali dari dialog izin atau pengaturan notifikasi HP.
        laporIzin()
    }

    // Pilihan dan jadwal

    private fun terapkan(props: JSONObject) {
        val live = props.optBoolean("live", true)
        val laporan = props.optBoolean("laporan", true)
        val bahasa = if (props.optString("bahasa").equals("EN", ignoreCase = true)) "en" else "id"
        val p = prefs
        val e = p.edit()
        // Dinyalakan lagi: mulai dari yang ada sekarang, bukan semua yang terlewat selama mati.
        if (live && !p.getBoolean("live", true)) e.remove(Kabar.LIVE_DILIHAT)
        if (laporan && !p.getBoolean("laporan", true)) e.remove(Kabar.LAPORAN_DILIHAT)
        e.putBoolean("live", live).putBoolean("laporan", laporan).putString("bahasa", bahasa)
            .putString("umpan", props.optString("umpan")).putString("siaran", props.optString("siaran"))
            .putString("situs", props.optString("situs")).apply()
        // Siaran sendiri tidak dikabarkan ke HP sendiri.
        props.optString("abaikan").takeIf { it.isNotEmpty() }?.let { Kabar.tandai(p, Kabar.LIVE_DILIHAT, listOf(it)) }
        Kabar.kanal(app, bahasa)
        // Alamat belum ada: jadwal dibiarkan (membatalkannya lalu membuat ulang memulai hitungan 15 menit dari nol).
        if (props.optString("umpan").isEmpty()) return
        val aktif = live || laporan
        if (aktif != dijadwalkan) {
            dijadwalkan = aktif
            jadwal(aktif)
        }
    }

    private fun jadwal(aktif: Boolean) {
        val wm = try { WorkManager.getInstance(app) } catch (e: IllegalStateException) { return }
        if (!aktif) {
            wm.cancelUniqueWork(Kabar.KERJA)
            wm.cancelUniqueWork(Kabar.KERJA_SEKARANG)
            app.getSystemService(AlarmManager::class.java).cancel(Kabar.alarm(app))
            return
        }
        val minta = PeriodicWorkRequestBuilder<CekKabar>(15, TimeUnit.MINUTES).setConstraints(Kabar.jaringan()).build()
        // KEEP: membuka aplikasi tidak mengulang hitungan 15 menitnya. Pemeriksaan cepat berikutnya 1 menit setelah dibuka.
        wm.enqueueUniquePeriodicWork(Kabar.KERJA, ExistingPeriodicWorkPolicy.KEEP, minta)
        wm.cancelUniqueWork(Kabar.KERJA_LAMA)
        Kabar.sambung(app, 60)
    }

    // Izin

    private fun izin(): String {
        if (Build.VERSION.SDK_INT >= 33) {
            when (context.permission(Manifest.permission.POST_NOTIFICATIONS).state) {
                "prompt" -> return "tanya"
                "denied" -> return "ditolak"
            }
        }
        return if (app.getSystemService(NotificationManager::class.java).areNotificationsEnabled()) "diberi" else "ditolak"
    }

    private fun laporIzin() {
        val kirim = emit ?: return
        val s = izin()
        if (s == izinTerakhir) return
        izinTerakhir = s
        kirim("message", s)
    }

    private fun tanyaPertama(props: JSONObject) {
        if (prefs.getBoolean("sudahDitanya", false)) return
        if (!props.optBoolean("live", true) && !props.optBoolean("laporan", true)) return
        main.postDelayed({
            if (!context.resumed || prefs.getBoolean("sudahDitanya", false)) return@postDelayed
            prefs.edit().putBoolean("sudahDitanya", true).apply()
            if (izin() == "tanya") context.askPermission(Manifest.permission.POST_NOTIFICATIONS) { laporIzin() }
        }, 4000)
    }

    private fun mintaIzin() {
        prefs.edit().putBoolean("sudahDitanya", true).apply()
        when (izin()) {
            "diberi" -> laporIzin()
            "tanya" -> context.askPermission(Manifest.permission.POST_NOTIFICATIONS) { laporIzin() }
            else -> bukaPengaturan()
        }
    }

    private fun bukaPengaturan() {
        val a = context.activity ?: return
        try {
            a.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, app.packageName))
        } catch (e: ActivityNotFoundException) {
            a.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", app.packageName, null)))
        }
    }
}

/** Alarm pemeriksaan cepat: menyerahkan pemeriksaannya ke WorkManager, yang memberinya jaringan. */
class AlarmKabar : BroadcastReceiver() {
    override fun onReceive(c: Context, i: Intent) = Kabar.cekSekarang(c)
}

/** Satu pemeriksaan, di utas WorkManager. Galat jaringan cukup ditunggu sampai pemeriksaan berikutnya. */
class CekKabar(c: Context, p: WorkerParameters) : Worker(c, p) {
    override fun doWork(): Result {
        val c = applicationContext
        val p = Kabar.prefs(c)
        try {
            // Rantai cepat dan kerja berkala bisa berbarengan: satu per satu, supaya tidak mengabari dua kali.
            synchronized(Kabar) { periksa(c, p) }
        } finally {
            // Alarm pemeriksaan cepat berikutnya; kerja berkala memasang lagi alarm yang hilang.
            if (p.getBoolean("live", true) || p.getBoolean("laporan", true)) Kabar.sambung(c, 90)
        }
        return Result.success()
    }

    private fun periksa(c: Context, p: SharedPreferences) {
        // Notifikasi dimatikan di HP: tanpa jaringan, dan mulai dari yang ada saat dinyalakan lagi.
        if (!c.getSystemService(NotificationManager::class.java).areNotificationsEnabled()) {
            p.edit().remove(Kabar.LIVE_DILIHAT).remove(Kabar.LAPORAN_DILIHAT).apply()
            return
        }
        if (p.getBoolean("live", true)) try { Kabar.cekLive(c, p) } catch (e: Exception) { Log.i("laporin", "notifikasi live: $e") }
        val kini = System.currentTimeMillis()
        if (p.getBoolean("laporan", true) && kini - p.getLong("laporanDicek", 0) >= 14 * 60_000) {
            p.edit().putLong("laporanDicek", kini).apply()
            try { Kabar.cekLaporan(c, p) } catch (e: Exception) { Log.i("laporin", "notifikasi laporan: $e") }
        }
    }
}

internal object Kabar {
    const val KERJA = "laporin.notifikasi.cek"
    const val KERJA_SEKARANG = "laporin.notifikasi.sekarang"
    /** Rantai kerja tertunda dari versi uji sebelumnya: dibatalkan, supaya tidak menahan pemeriksaan. */
    const val KERJA_LAMA = "laporin.notifikasi.cepat"
    const val LIVE_DILIHAT = "liveDilihat"
    const val LAPORAN_DILIHAT = "laporanDilihat"
    private const val WARNA = 0xFFFF4A1C.toInt()
    /** Lebih dari ini laporan baru sekaligus: satu notifikasi ringkasan, bukan banjir. */
    private const val SATU_SATU = 3

    fun prefs(c: Context): SharedPreferences = c.getSharedPreferences("laporin.notifikasi", Context.MODE_PRIVATE)

    fun jaringan(): Constraints = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

    fun alarm(c: Context): PendingIntent = PendingIntent.getBroadcast(c, 0, Intent(c, AlarmKabar::class.java),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

    /** Alarm pemeriksaan cepat [detik] lagi, menggantikan yang terpasang. Tidak eksak: Android boleh mengundurnya. */
    fun sambung(c: Context, detik: Long) {
        c.getSystemService(AlarmManager::class.java)
            .set(AlarmManager.ELAPSED_REALTIME_WAKEUP, SystemClock.elapsedRealtime() + detik * 1000, alarm(c))
    }

    /** Pemeriksaan sekarang: kerja expedited (Android 12+) bersyarat jaringan; kuota habis = kerja biasa. */
    fun cekSekarang(c: Context) {
        val b = OneTimeWorkRequestBuilder<CekKabar>().setConstraints(jaringan())
        if (Build.VERSION.SDK_INT >= 31) b.setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
        WorkManager.getInstance(c).enqueueUniqueWork(KERJA_SEKARANG, ExistingWorkPolicy.KEEP, b.build())
    }

    private fun en(p: SharedPreferences) = p.getString("bahasa", "id") == "en"

    /** Kanal `live` (penting, muncul di atas layar) dan `laporan` (biasa); namanya mengikuti bahasa aplikasi. */
    fun kanal(c: Context, bahasa: String) {
        val en = bahasa == "en"
        val nm = c.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("live", if (en) "Live broadcasts" else "Siaran langsung",
            NotificationManager.IMPORTANCE_HIGH).apply {
            description = if (en) "When a resident starts a public live broadcast" else "Saat warga mulai siaran langsung publik"
        })
        nm.createNotificationChannel(NotificationChannel("laporan", if (en) "New reports" else "Laporan baru",
            NotificationManager.IMPORTANCE_DEFAULT).apply {
            description = if (en) "When officers publish a verified report" else "Saat petugas menayangkan laporan yang sudah diverifikasi"
        })
    }

    // Yang sudah dilihat: daftar id (urutan masuk), paling banyak 300; kunci yang belum ada = pemeriksaan pertama.

    private fun dilihat(p: SharedPreferences, kunci: String): List<String>? = p.getString(kunci, null)?.let { s ->
        try { JSONArray(s).let { a -> List(a.length()) { a.getString(it) } } } catch (e: Exception) { null }
    }

    fun tandai(p: SharedPreferences, kunci: String, id: List<String>) = synchronized(this) {
        val lama = dilihat(p, kunci).orEmpty()
        val baru = (lama.filter { it !in id } + id).takeLast(300)
        p.edit().putString(kunci, JSONArray(baru).toString()).apply()
    }

    fun cekLive(c: Context, p: SharedPreferences) {
        val alamat = p.getString("siaran", "").orEmpty().ifEmpty { return }
        val daftar = JSONArray(String(ambil(alamat) ?: return, Charsets.UTF_8))
        val siaran = List(daftar.length()) { daftar.getJSONObject(it) }.filter { it.optString("ruang").isNotEmpty() }
        // Tanpa catatan (pemeriksaan pertama): siaran yang sedang berjalan pun dikabarkan, karena hanya berarti selagi live.
        val sebelumnya = dilihat(p, LIVE_DILIHAT).orEmpty()
        val ruang = siaran.map { it.optString("ruang") }
        val nm = c.getSystemService(NotificationManager::class.java)
        // Siaran yang sudah selesai: notifikasinya ditarik.
        sebelumnya.filter { it !in ruang }.forEach { nm.cancel("live:$it", 1) }
        tandai(p, LIVE_DILIHAT, ruang)
        Log.i("laporin", "notifikasi: ${siaran.size} siaran, ${ruang.count { it !in sebelumnya }} baru")
        kanal(c, p.getString("bahasa", "id")!!)
        val en = en(p)
        for (s in siaran.filter { it.optString("ruang") !in sebelumnya }) {
            val nama = s.optString("nama").trim().ifEmpty { if (en) "A resident" else "Warga" }
            val judul = s.optString("judul").trim().ifEmpty { if (en) "Watch the live broadcast now" else "Tonton siaran langsungnya sekarang" }
            val n = dasar(c, "live", "app://tonton/${Uri.encode(s.optString("ruang"))}")
                .setContentTitle(if (en) "🔴 $nama is live" else "🔴 $nama sedang live")
                .setContentText(judul)
                .setCategory(Notification.CATEGORY_EVENT)
                .build()
            nm.notify("live:${s.optString("ruang")}", 1, n)
        }
    }

    fun cekLaporan(c: Context, p: SharedPreferences) {
        val alamat = p.getString("umpan", "").orEmpty().ifEmpty { return }
        val badan = JSONObject(String(ambil("$alamat?bahasa=${p.getString("bahasa", "id")}") ?: return, Charsets.UTF_8))
        val isi = badan.optJSONArray("isi") ?: return
        // Aturan umpan aplikasi: tanpa koordinat atau media, laporan tidak tampil di Beranda, jadi tidak dikabarkan.
        val laporan = List(isi.length()) { isi.getJSONObject(it) }.filter { l ->
            val lat = l.optDouble("lat", Double.NaN)
            val lng = l.optDouble("lng", Double.NaN)
            l.has("id") && l.optString("judul").isNotEmpty() && lat.isFinite() && lng.isFinite() && !(lat == 0.0 && lng == 0.0) &&
                (l.optJSONArray("media")?.length() ?: 0) > 0
        }
        val sebelumnya = dilihat(p, LAPORAN_DILIHAT)
        tandai(p, LAPORAN_DILIHAT, laporan.map { it.optLong("id").toString() })
        val baru = laporan.filter { sebelumnya != null && it.optLong("id").toString() !in sebelumnya }
        Log.i("laporin", "notifikasi: ${laporan.size} laporan, ${if (sebelumnya == null) "pertama" else "${baru.size} baru"}")
        if (sebelumnya == null) return
        if (baru.isEmpty()) return
        kanal(c, p.getString("bahasa", "id")!!)
        val en = en(p)
        val nm = c.getSystemService(NotificationManager::class.java)
        if (baru.size > SATU_SATU) {
            val gaya = Notification.InboxStyle()
            baru.take(5).forEach { gaya.addLine(it.optString("judul")) }
            val n = dasar(c, "laporan", null)
                .setContentTitle(if (en) "${baru.size} new verified fire reports" else "${baru.size} laporan karhutla baru terverifikasi")
                .setContentText(baru.first().optString("judul"))
                .setStyle(gaya)
                .build()
            nm.notify("laporan:ringkas", 1, n)
            return
        }
        val situs = p.getString("situs", "").orEmpty()
        for (l in baru) {
            val id = l.optLong("id").toString()
            val tempat = l.optString("provinsi").ifEmpty { l.optString("pulau") }
            val teks = if (tempat.isEmpty()) l.optString("judul") else "${l.optString("judul")} · $tempat"
            val b = dasar(c, "laporan", "app://laporan/$id")
                .setContentTitle(if (en) "Verified fire report" else "Laporan karhutla terverifikasi")
                .setContentText(teks)
            val gambar = sampul(l).takeIf { it.isNotEmpty() && situs.isNotEmpty() }?.let { gambar("$situs/_next/image?url=${URLEncoder.encode(it, "UTF-8")}&w=640&q=75") }
            if (gambar != null) {
                b.setLargeIcon(gambar).setStyle(Notification.BigPictureStyle().bigPicture(gambar).bigLargeIcon(null as Icon?).setSummaryText(teks))
            } else {
                b.setStyle(Notification.BigTextStyle().bigText(teks))
            }
            nm.notify("laporan:$id", 1, b.build())
        }
    }

    /** Gambar sampul seperti umpan aplikasi: media pertama, atau posternya bila video. */
    private fun sampul(l: JSONObject): String {
        val m = l.optJSONArray("media")?.optJSONObject(0)
        val dari = if (m == null) "" else if (m.optString("jenis") == "video") m.optString("poster") else m.optString("url")
        return dari.ifEmpty { l.optString("gambar") }
    }

    private fun dasar(c: Context, kanal: String, tautan: String?): Notification.Builder = Notification.Builder(c, kanal)
        .setSmallIcon(ikonKecil(c))
        .setColor(WARNA)
        .setAutoCancel(true)
        .setShowWhen(true)
        .setContentIntent(buka(c, tautan))

    /** Membuka aplikasi di [tautan] (`app://…`), atau seperti dari peluncur bila null. */
    private fun buka(c: Context, tautan: String?): PendingIntent {
        val luncur = c.packageManager.getLaunchIntentForPackage(c.packageName) ?: Intent()
        val i = if (tautan == null) luncur else Intent(Intent.ACTION_VIEW, Uri.parse(tautan)).setComponent(luncur.component)
        i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        return PendingIntent.getActivity(c, 0, i, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }

    @Volatile private var ikon: Icon? = null

    /** Siluet putih logo (assets/ikon/notifikasi.png, sama dengan laporin-apps); ikon aplikasi bila tak terbaca. */
    private fun ikonKecil(c: Context): Icon = ikon ?: (try {
        c.assets.open("app/assets/ikon/notifikasi.png").use { BitmapFactory.decodeStream(it) }?.let { Icon.createWithBitmap(it) }
    } catch (e: Exception) { null } ?: Icon.createWithResource(c, c.applicationInfo.icon)).also { ikon = it }

    private fun gambar(alamat: String): Bitmap? = try {
        ambil(alamat, "image/jpeg,image/png,image/*")?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
    } catch (e: Exception) { null }

    /** Badan GET [alamat] bila 2xx, else null. */
    private fun ambil(alamat: String, terima: String = "application/json"): ByteArray? {
        val url = URL(alamat)
        if (url.protocol == "http") return ambilPolos(url, terima)
        val k = url.openConnection() as HttpURLConnection
        return try {
            k.connectTimeout = 10_000
            k.readTimeout = 15_000
            k.setRequestProperty("Accept", terima)
            if (k.responseCode !in 200..299) null else k.inputStream.use { it.readBytes() }
        } finally {
            k.disconnect()
        }
    }

    /**
     * `http:` hanya untuk mock-server saat pengembangan (build dev ke 127.0.0.1 lewat adb reverse): kebijakan cleartext
     * Android menolaknya di HttpURLConnection, tidak di soket. HTTP/1.0, jadi badannya utuh sampai sambungan ditutup.
     */
    private fun ambilPolos(url: URL, terima: String): ByteArray? = Socket().use { s ->
        s.connect(InetSocketAddress(url.host, if (url.port > 0) url.port else 80), 10_000)
        s.soTimeout = 15_000
        s.getOutputStream().write("GET ${url.file.ifEmpty { "/" }} HTTP/1.0\r\nHost: ${url.host}\r\nAccept: $terima\r\nConnection: close\r\n\r\n".toByteArray())
        val semua = s.getInputStream().readBytes()
        val batas = (0..semua.size - 4).firstOrNull { semua[it] == '\r'.code.toByte() && semua[it + 1] == '\n'.code.toByte() &&
            semua[it + 2] == '\r'.code.toByte() && semua[it + 3] == '\n'.code.toByte() } ?: return null
        val kepala = String(semua, 0, batas, Charsets.ISO_8859_1).lowercase()
        if (!Regex("^http/1\\.[01] 2\\d\\d").containsMatchIn(kepala)) return null
        val badan = semua.copyOfRange(batas + 4, semua.size)
        if ("content-encoding: gzip" in kepala) GZIPInputStream(badan.inputStream()).use { it.readBytes() } else badan
    }
}
