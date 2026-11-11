// @laporin/web di Android: `web-halaman` adalah WebView yang memuat `alamat`: halaman laporin.app, atau halaman lokal
// di aset aplikasi (`file:///android_asset/app/assets/…`, mis. peta laporan). Halaman laporin.app tetap di dalam
// aplikasi; tautan ke situs lain dibuka di peramban HP. Mengirim `message` "dimuat" saat halaman selesai dimuat dan
// "galat: …" bila gagal. Halaman lokal juga bisa mengirim pesannya sendiri (`laporin.kirim(teks)` di JavaScript) dan
// menerima prop `isi` (teks, biasanya JSON) lewat `window.terima(isi)` setiap kali prop itu berubah.
//
// Sentuhan: host Android Exact tidak meneruskan sentuhan ke view native (hanya `performClick` tanpa titik), jadi
// tombol dan geseran di dalam halaman mati. Node yang diberi `hatch="sentuh-web" data-kunci=K` (modules/android)
// mendengar pointer-nya dan meneruskannya ke [Sentuhan.teruskan]; WebView dengan prop `kunci` = K menerimanya
// sebagai sentuhan sungguhan. Hatch hanya membawa satu jari (tanpa id), jadi untuk cubit-zoom [Sentuhan] juga
// mengamati peristiwa sentuh mentah jendela: begitu jari kedua mendarat selagi jari pertama menyentuh WebView lewat
// hatch, semua jari diteruskan dari peristiwa mentah itu sampai jari terakhir diangkat.
//
// Kamera dan mikrofon (siaran langsung, assets/live): halaman lokal yang memanggil getUserMedia mendapatkannya bila
// izin Android-nya diberikan; izin itu diminta di sini saat halaman pertama memintanya. Halaman laporin.app tidak.
//
// Captcha laporan tanpa akun: halaman Turnstile server (`https://…/api/mobile/captcha`) mengirim tokennya lewat
// `window.ReactNativeWebView.postMessage(json)`, seperti ke react-native-webview laporin-apps. Jembatan itu hanya
// menyampaikan pesan dari halaman di jalur itu, sebagai `message` "rn <json>".
//
// Halaman lokal dimuat dari `https://appassets.androidplatform.net/…` (domain cadangan WebViewAssetLoader) yang
// dilayani dari aset di sini, bukan dari `file://`: origin https boleh membingkai Windy geoplatform (CSP-nya
// `frame-ancestors *` tidak cocok dengan induk `file:`), memakai IndexedDB, dan mengambil API ber-CORS biasa.
package laporin.web

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import android.view.View
import android.view.Window
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.PermissionRequest
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

private const val ASET = "file:///android_asset/"
private const val HOST_LOKAL = "appassets.androidplatform.net"
private const val LOKAL = "https://$HOST_LOKAL/"
private const val JALUR_CAPTCHA = "/api/mobile/captcha"
private const val HOST_TURNSTILE = "challenges.cloudflare.com"

/** Halaman captcha server (https, jalur `/api/mobile/captcha`): satu-satunya yang boleh memakai ReactNativeWebView. */
private fun halamanCaptcha(url: String?): Boolean {
    val u = Uri.parse(url ?: return false)
    return u.scheme == "https" && u.path == JALUR_CAPTCHA
}

/** Alamat `file:///android_asset/…` dari Contract menjadi alamat https lokal yang dilayani [WebModule]. */
private fun alamatMuat(alamat: String) = if (alamat.startsWith(ASET)) LOKAL + alamat.removePrefix(ASET) else alamat

private fun jenisBerkas(jalur: String): String = when (jalur.substringAfterLast('.', "").lowercase()) {
    "html", "htm" -> "text/html"
    "js", "mjs" -> "text/javascript"
    "css" -> "text/css"
    "json" -> "application/json"
    "svg" -> "image/svg+xml"
    "png" -> "image/png"
    "jpg", "jpeg" -> "image/jpeg"
    "ttf" -> "font/ttf"
    "otf" -> "font/otf"
    "woff2" -> "font/woff2"
    "wasm" -> "application/wasm"
    else -> "application/octet-stream"
}

/** Keadaan satu `web-halaman`: `isi` terakhir dan apakah halamannya sudah siap menerimanya. */
private class Halaman(val emit: (String, String) -> Unit) {
    var data = ""
    var dimuat = false
}

class WebModule(context: ExtensionContext) : ExactExtension(context) {
    private val ext = context
    private val halaman = HashMap<WebView, Halaman>()

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    @SuppressLint("SetJavaScriptEnabled", "JavascriptInterface")
    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "web-halaman") { "no view $tag" }
        val web = WebView(context)
        val h = Halaman(emit)
        halaman[web] = h
        // Peta memakai MapLibre (WebGL); laporin.app menyimpan pilihan lapisannya di localStorage.
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        // Video siaran yang ditonton diputar dengan suaranya tanpa menunggu ketukan di halaman.
        web.settings.mediaPlaybackRequiresUserGesture = false
        // Bening sampai halaman menggambar latarnya: pratinjau siaran di kartu tidak berkedip putih di atas avatar.
        web.setBackgroundColor(android.graphics.Color.TRANSPARENT)
        web.webChromeClient = object : WebChromeClient() {
            override fun onPermissionRequest(request: PermissionRequest) = izinkanMedia(request)
        }
        // Pesan dari halaman lokal saja: laporin.app tidak bisa memakai jembatan ini.
        web.addJavascriptInterface(object {
            @JavascriptInterface
            fun kirim(pesan: String) {
                web.post { if (web.url.orEmpty().startsWith(LOKAL)) emit("message", pesan) }
            }
        }, "laporin")
        web.addJavascriptInterface(object {
            @JavascriptInterface
            fun postMessage(pesan: String) {
                web.post { if (halamanCaptcha(web.url)) emit("message", "rn $pesan") }
            }
        }, "ReactNativeWebView")
        // Turnstile berjalan di iframe challenges.cloudflare.com di dalam halaman captcha (react-native-webview pun
        // mengizinkan cookie pihak ketiga secara bawaan).
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, true)
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                // Bingkai di dalam halaman (iframe Turnstile, peta Windy) dimuat di tempatnya.
                if (!request.isForMainFrame) return false
                val host = request.url.host.orEmpty()
                if (host == "laporin.app" || host.endsWith(".laporin.app") || host == HOST_LOKAL) return false
                if (host == HOST_TURNSTILE || halamanCaptcha(request.url.toString())) return false
                runCatching { view.context.startActivity(Intent(Intent.ACTION_VIEW, request.url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                return true
            }

            // Halaman lokal dan berkas di sebelahnya (skrip, gaya, huruf) dari aset aplikasi; sisanya ke jaringan.
            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
                if (request.url.host != HOST_LOKAL) return null
                val jalur = request.url.path.orEmpty().removePrefix("/")
                val jenis = jenisBerkas(jalur)
                return runCatching {
                    WebResourceResponse(jenis, if (jenis.startsWith("text/")) "utf-8" else null, view.context.assets.open(jalur))
                }.getOrElse { WebResourceResponse("text/plain", "utf-8", 404, "Not Found", emptyMap(), null) }
            }

            override fun onPageFinished(view: WebView, url: String) {
                h.dimuat = true
                kirimData(view, h)
                emit("message", "dimuat")
            }

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (request.isForMainFrame) emit("message", "galat: ${error.description}")
            }

            override fun onReceivedHttpError(view: WebView, request: WebResourceRequest, response: WebResourceResponse) {
                if (request.isForMainFrame) emit("message", "galat: HTTP ${response.statusCode}")
            }
        }
        update(web, props)
        return web
    }

    override fun update(view: View, props: JSONObject) {
        val web = view as WebView
        val h = halaman[web] ?: return
        Sentuhan.daftar(props.optString("kunci"), web)
        // Props datang utuh pada setiap perubahan: muat ulang hanya bila alamatnya berubah.
        val alamat = props.optString("alamat")
        if (alamat.isNotEmpty() && web.tag != alamat) {
            web.tag = alamat
            h.dimuat = false
            web.loadUrl(Uri.parse(alamatMuat(alamat)).toString())
        }
        val data = props.optString("isi")
        if (data != h.data) {
            h.data = data
            kirimData(web, h)
        }
    }

    // WebView-nya dihancurkan host sesudah ini. Halamannya dikosongkan dulu: kamera, mikrofon dan sambungan siaran
    // dilepas saat itu juga, bukan menunggu WebView dibereskan.
    override fun destroyView(view: View) {
        halaman.remove(view)
        Sentuhan.lepas(view as WebView)
        view.loadUrl("about:blank")
    }

    /** getUserMedia halaman lokal: kamera dan mikrofon yang izin Android-nya ada (diminta dulu bila belum). */
    private fun izinkanMedia(request: PermissionRequest) {
        if (request.origin.host != HOST_LOKAL) return request.deny()
        val android = HashMap<String, String>()
        for (r in request.resources) when (r) {
            PermissionRequest.RESOURCE_VIDEO_CAPTURE -> android[r] = Manifest.permission.CAMERA
            PermissionRequest.RESOURCE_AUDIO_CAPTURE -> android[r] = Manifest.permission.RECORD_AUDIO
        }
        if (android.isEmpty()) return request.deny()
        ext.requestPermissions(android.values.toTypedArray()) {
            val boleh = android.filterValues { ext.permission(it).state == "granted" }.keys
            if (boleh.isEmpty()) request.deny() else request.grant(boleh.toTypedArray())
        }
    }

    /** `isi` diberikan ke `window.terima` halaman, bila halaman sudah dimuat dan memasangnya. */
    private fun kirimData(web: WebView, h: Halaman) {
        if (!h.dimuat || h.data.isEmpty()) return
        web.evaluateJavascript("window.terima && window.terima(${JSONObject.quote(h.data)})", null)
    }
}

/** Sentuhan yang didengar hatch `sentuh-web`, diteruskan ke WebView dengan `kunci` yang sama. */
object Sentuhan {
    private val web = HashMap<String, WebView>()
    private var turun = 0L
    private var lalu = 0L
    private var sasaran: WebView? = null

    // Cubit: peristiwa mentah terakhir, titik turun jari pertama (piksel jendela, id-nya) dan titiknya di kotak node
    // (poin), skala poin → piksel WebView, kerapatan layar; `jamak` = jari-jari sedang diteruskan dari peristiwa
    // mentah, `tunda` = jari kedua mendarat sebelum hatch menyampaikan jari pertama.
    private var mentah: MotionEvent? = null
    private var turunX = 0f
    private var turunY = 0f
    private var idUtama = 0
    private var lokalX = 0f
    private var lokalY = 0f
    private var skalaNode = 1f
    private var kerapatan = 1f
    private var jamak = false
    private var tunda = false
    private var jendela: Window? = null

    internal fun daftar(kunci: String, v: WebView) {
        web.entries.removeAll { it.value === v }
        if (kunci.isNotEmpty()) web[kunci] = v
        pasang(v.context)
    }

    internal fun lepas(v: WebView) {
        web.entries.removeAll { it.value === v }
        if (sasaran === v) {
            sasaran = null
            jamak = false
        }
    }

    /** Pengamat sentuh mentah dipasang sekali di jendela aktivitas; peristiwa tetap diteruskan ke Exact seperti biasa. */
    private fun pasang(c: Context) {
        var k: Context? = c
        while (k is ContextWrapper && k !is Activity) k = k.baseContext
        val w = (k as? Activity)?.window ?: return
        if (jendela === w) return
        val asli = w.callback ?: return
        w.callback = object : Window.Callback by asli {
            override fun dispatchTouchEvent(event: MotionEvent): Boolean {
                runCatching { amati(event) }
                return asli.dispatchTouchEvent(event)
            }
        }
        jendela = w
    }

    private fun amati(e: MotionEvent) {
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                jamak = false
                tunda = false
                turunX = e.getX(0)
                turunY = e.getY(0)
                idUtama = e.getPointerId(0)
            }
            MotionEvent.ACTION_POINTER_DOWN -> if (!jamak) {
                if (sasaran != null) mulaiJamak(e, e.actionIndex) else tunda = true
            } else {
                kirimMentah(e, MotionEvent.ACTION_POINTER_DOWN, e.actionIndex)
            }
            MotionEvent.ACTION_MOVE -> if (jamak) kirimMentah(e, MotionEvent.ACTION_MOVE, 0)
            MotionEvent.ACTION_POINTER_UP -> if (jamak) kirimMentah(e, MotionEvent.ACTION_POINTER_UP, e.actionIndex)
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (jamak) {
                    kirimMentah(e, e.actionMasked, 0)
                    sasaran = null
                    jamak = false
                }
                tunda = false
            }
        }
        mentah?.recycle()
        mentah = MotionEvent.obtain(e)
    }

    /** Jari ke-[indeks] di [e] baru mendarat: WebView yang memegang jari pertama menerima semua jari mulai sekarang. */
    private fun mulaiJamak(e: MotionEvent, indeks: Int) {
        if (e.findPointerIndex(idUtama) < 0 || e.getPointerId(indeks) == idUtama) return
        jamak = true
        tunda = false
        kirimMentah(e, MotionEvent.ACTION_POINTER_DOWN, indeks)
    }

    /**
     * [e] sebagai sentuhan WebView: setiap jari dipetakan dari titik turun jari pertama (posisinya di node + selisih
     * pikselnya dibagi kerapatan, lalu skala node), dan id-nya dibuat tetap: jari pertama 0 seperti dari hatch.
     */
    private fun kirimMentah(e: MotionEvent, aksi: Int, indeks: Int) {
        val v = sasaran ?: return
        val n = e.pointerCount
        val sifat = Array(n) { MotionEvent.PointerProperties() }
        val titik = Array(n) { MotionEvent.PointerCoords() }
        for (i in 0 until n) {
            e.getPointerProperties(i, sifat[i])
            val id = sifat[i].id
            sifat[i].id = if (id == idUtama) 0 else id + 1
            sifat[i].toolType = MotionEvent.TOOL_TYPE_FINGER
            e.getPointerCoords(i, titik[i])
            titik[i].x = (lokalX + (e.getX(i) - turunX) / kerapatan) * skalaNode
            titik[i].y = (lokalY + (e.getY(i) - turunY) / kerapatan) * skalaNode
        }
        val kini = maxOf(SystemClock.uptimeMillis(), lalu + 1)
        lalu = kini
        val penuh = if (aksi == MotionEvent.ACTION_POINTER_DOWN || aksi == MotionEvent.ACTION_POINTER_UP) {
            aksi or (indeks shl MotionEvent.ACTION_POINTER_INDEX_SHIFT)
        } else {
            aksi
        }
        val m = MotionEvent.obtain(turun, kini, penuh, n, sifat, titik, 0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0)
        v.dispatchTouchEvent(m)
        m.recycle()
    }

    /**
     * Satu fase pointer ("down", "move", "up", "cancel") di titik ([x], [y]) kotak node, dalam poin; [lebar] lebar
     * kotak itu (poin), untuk skala ke piksel WebView. Waktu peristiwa dibuat naik terus: pointer yang tiba
     * berbarengan dalam satu giliran tidak bertimestamp sama (lemparan peta dihitung dari kecepatannya).
     */
    fun teruskan(kunci: String, fase: String, x: Float, y: Float, lebar: Float) {
        val aksi = when (fase) {
            "down" -> MotionEvent.ACTION_DOWN
            "move" -> MotionEvent.ACTION_MOVE
            "up" -> MotionEvent.ACTION_UP
            else -> MotionEvent.ACTION_CANCEL
        }
        // Selama cubit, jari-jari datang dari peristiwa mentah ([amati]); jari pertama dari hatch diabaikan.
        if (jamak) return
        val v = if (aksi == MotionEvent.ACTION_DOWN) web[kunci] else sasaran
        if (v == null || lebar <= 0f || v.width == 0) return
        val skala = v.width / lebar
        val kini = maxOf(SystemClock.uptimeMillis(), lalu + 1)
        lalu = kini
        if (aksi == MotionEvent.ACTION_DOWN) {
            // Sentuhan sebelumnya yang belum selesai dibatalkan dulu.
            sasaran?.let { kirim(it, MotionEvent.ACTION_CANCEL, 0f, 0f, kini) }
            turun = kini
            sasaran = v
            lokalX = x
            lokalY = y
            skalaNode = skala
            kerapatan = v.resources.displayMetrics.density
        }
        kirim(v, aksi, x * skala, y * skala, kini)
        if (aksi == MotionEvent.ACTION_UP || aksi == MotionEvent.ACTION_CANCEL) sasaran = null
        // Jari kedua sudah mendarat sebelum hatch menyampaikan jari pertama: cubit dimulai sekarang.
        val m = mentah
        if (aksi == MotionEvent.ACTION_DOWN && tunda && m != null && m.pointerCount >= 2) {
            val kedua = (0 until m.pointerCount).firstOrNull { m.getPointerId(it) != idUtama }
            if (kedua != null) mulaiJamak(m, kedua)
        }
    }

    private fun kirim(v: WebView, aksi: Int, x: Float, y: Float, waktu: Long) {
        val e = MotionEvent.obtain(turun, waktu, aksi, x, y, 0)
        e.source = InputDevice.SOURCE_TOUCHSCREEN
        v.dispatchTouchEvent(e)
        e.recycle()
    }
}
