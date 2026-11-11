// @laporin/bahasa di Android: `atur-bahasa` adalah view kosong tak terlihat (Contract memberinya 1×1 px: host Android
// hanya membuat view native yang ikut tergambar) yang menjadikan prop `bahasa` ("ID"/"EN") locale aplikasi. Host Exact
// membaca locale dari konfigurasi activity dan menangani perubahannya di tempat (manifest mendeklarasikan perubahan
// locale), jadi `t()` Contract langsung berganti ke strings/id.json atau strings/en.json tanpa membuat ulang activity.
//
// Android 13+: LocaleManager.applicationLocales (bahasa per aplikasi, disimpan sistem dan tampil di Setelan aplikasi).
// Sebelumnya: konfigurasi resources activity dan aplikasi diganti lalu onConfigurationChanged dipanggil, jalur yang
// sama dengan perubahan dari sistem; tidak disimpan sistem, jadi props pertama setiap peluncuran menerapkannya lagi.
package laporin.bahasa

import android.app.Activity
import android.app.LocaleManager
import android.content.Context
import android.content.ContextWrapper
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import android.view.View
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.util.Locale

class BahasaModule(context: ExtensionContext) : ExactExtension(context) {
    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "atur-bahasa") { "no view $tag" }
        val v = View(context)
        v.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        val bahasa = props.optString("bahasa")
        // Setelah view terpasang: perubahan konfigurasi tidak menyela pembuatan view host.
        v.post { terapkan(v.context, bahasa) }
        return v
    }

    override fun update(view: View, props: JSONObject) {
        val bahasa = props.optString("bahasa")
        view.post { terapkan(view.context, bahasa) }
    }

    private fun terapkan(c: Context, bahasa: String) {
        val tag = when (bahasa.uppercase(Locale.ROOT)) {
            "EN" -> "en"
            "ID" -> "id"
            else -> return
        }
        val act = activity(c) ?: return
        if (Build.VERSION.SDK_INT >= 33) {
            val lm = act.getSystemService(LocaleManager::class.java) ?: return
            if (lm.applicationLocales.toLanguageTags() == tag) return
            lm.applicationLocales = LocaleList.forLanguageTags(tag)
        } else {
            val loc = Locale.forLanguageTag(tag)
            if (act.resources.configuration.locales[0].language == loc.language) return
            Locale.setDefault(loc)
            val cfg = Configuration(act.resources.configuration)
            cfg.setLocales(LocaleList(loc))
            @Suppress("DEPRECATION")
            act.resources.updateConfiguration(cfg, act.resources.displayMetrics)
            val app = act.applicationContext.resources
            @Suppress("DEPRECATION")
            app.updateConfiguration(cfg, app.displayMetrics)
            act.onConfigurationChanged(cfg)
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
