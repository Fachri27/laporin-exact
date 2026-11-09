// @laporin/web di Android: `web-halaman` adalah WebView yang memuat `alamat`. Halaman laporin.app tetap di
// dalam aplikasi; tautan ke situs lain dibuka di peramban HP. Mengirim `message` "dimuat" saat halaman selesai
// dimuat dan "galat: …" bila gagal, supaya Contract bisa menampilkan statusnya.
package laporin.web

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.view.View
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class WebModule(context: ExtensionContext) : ExactExtension(context) {
    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    @SuppressLint("SetJavaScriptEnabled")
    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "web-halaman") { "no view $tag" }
        val web = WebView(context)
        // Peta laporin.app memakai MapLibre (WebGL) dan menyimpan pilihan lapisannya di localStorage.
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val host = request.url.host.orEmpty()
                if (host == "laporin.app" || host.endsWith(".laporin.app")) return false
                runCatching { view.context.startActivity(Intent(Intent.ACTION_VIEW, request.url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
                return true
            }

            override fun onPageFinished(view: WebView, url: String) = emit("message", "dimuat")

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (request.isForMainFrame) emit("message", "galat: ${error.description}")
            }
        }
        update(web, props)
        return web
    }

    override fun update(view: View, props: JSONObject) {
        val web = view as WebView
        val alamat = props.optString("alamat")
        // Props datang utuh pada setiap perubahan: muat ulang hanya bila alamatnya berubah.
        if (alamat.isNotEmpty() && web.tag != alamat) {
            web.tag = alamat
            web.loadUrl(Uri.parse(alamat).toString())
        }
    }
}
