// @laporin/gulir di Android: `tanpa-garis-gulir` adalah view kosong tak terlihat yang menyembunyikan garis gulir di
// jendelanya. Host Android Exact menggambar garis itu dengan view `dev.exact.canvas.Scrolling$Indicator` (scrollbar
// bawaan View, gaya ExactScrollbars) di atas setiap penggulung, dan belum membaca `scrollbar-width="none"`. Setiap kali
// tata letak jendela berubah (indikator baru ditambahkan saat sebuah penggulung pertama kali bergerak), scrollbar
// indikator dimatikan, jadi `awakenScrollBars` miliknya tidak menggambar apa pun. Nama kelas itu tetap di build rilis
// (proguard host: `-keep class dev.exact.canvas.** { *; }`).
package laporin.gulir

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.view.View
import android.view.ViewGroup
import android.view.ViewTreeObserver
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.util.WeakHashMap

private const val INDIKATOR = "dev.exact.canvas.Scrolling\$Indicator"

class GulirModule(context: ExtensionContext) : ExactExtension(context) {
    /** Jendela yang sudah dipasangi pendengar, supaya view yang dibuat ulang tidak menambah pendengar kedua. */
    private val terpasang = WeakHashMap<View, ViewTreeObserver.OnGlobalLayoutListener>()

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "tanpa-garis-gulir") { "no view $tag" }
        val v = View(context)
        v.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        activity(context)?.window?.decorView?.let(::pasang)
        return v
    }

    private fun pasang(akar: View) {
        if (terpasang.containsKey(akar)) return
        val dengar = ViewTreeObserver.OnGlobalLayoutListener { sembunyikan(akar) }
        terpasang[akar] = dengar
        akar.viewTreeObserver.addOnGlobalLayoutListener(dengar)
        sembunyikan(akar)
    }

    private fun sembunyikan(v: View) {
        if (v.javaClass.name == INDIKATOR) {
            if (v.isVerticalScrollBarEnabled) v.isVerticalScrollBarEnabled = false
            if (v.isHorizontalScrollBarEnabled) v.isHorizontalScrollBarEnabled = false
        } else if (v is ViewGroup) {
            for (i in 0 until v.childCount) sembunyikan(v.getChildAt(i))
        }
    }

    private fun activity(c: Context): Activity? {
        var k: Context? = c
        while (k is ContextWrapper) {
            if (k is Activity) return k
            k = k.baseContext
        }
        return null
    }
}
