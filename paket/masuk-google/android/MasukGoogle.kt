// @laporin/masuk-google di Android: `masuk-google` adalah view kosong tak terlihat (Contract memberinya 1×1 px:
// host Android hanya membuat view native yang ikut tergambar) yang menjalankan "Masuk dengan Google" lewat Credential Manager AndroidX + Google Identity, lalu mengirim ID token
// Google ke Contract lewat `message`. Audiens token itu adalah `klien`, client ID WEB server (bukan client ID Android):
// server memeriksa `aud` dan `email_verified` di POST /api/mobile/masuk/google {idToken}.
//
// Satu percobaan dimulai setiap kali `minta` berganti ke nomor baru > 0 (tombol menaikkan nomornya). Props datang utuh
// pada setiap perubahan, jadi nomor yang sama diabaikan; props pertama hanya dicatat, supaya view yang dibuat ulang
// (pindah layar, row diparkir lalu dipakai lagi) tidak membuka pemilih akun sendiri. Karena itu view-nya dirender
// sejak awal dengan `minta` 0, bukan baru setelah tombol ditekan. Selama satu percobaan berjalan, nomor baru dicatat
// tanpa memulai percobaan kedua: pemilih akun sudah terbuka, dan pesannya tetap satu, dari percobaan yang berjalan.
//
// Setiap percobaan mengirim tepat satu `message`:
//   "token <idToken>"  berhasil;
//   "batal"            orang itu menutup pemilih akun;
//   "galat <pesan>"    selain itu: pesan Indonesia, lalu " · " dan kelas/jenis/pesan asli galatnya untuk men-debug.
//
// Alur tombol (GetSignInWithGoogleOption), bukan lembar bawah (GetGoogleIdOption): itu yang dianjurkan dokumentasi
// Android untuk tombol "Masuk dengan Google". Pemilih akun selalu tampil, akun yang perlu masuk ulang bisa dipakai, dan
// HP tanpa akun Google ditawari menambah akun. API callback (getCredentialAsync), seperti @exact/passkeys: activity host
// bukan LifecycleOwner (tidak ada lifecycleScope), dan kotlinx-coroutines hanya ada sebagai dependensi transitif.
//
// Galat konfigurasi: bila paket + SHA-1 aplikasi belum didaftarkan sebagai klien OAuth Android di proyek Google Cloud
// yang sama dengan klien web (atau `klien` bukan client ID web), Play Services menjawab DEVELOPER_ERROR (kode 10,
// "[28444] Developer console is not set up correctly"). Credential Manager membungkus galat Play Services itu sebagai
// NoCredentialException ("During get sign-in intent, failure response from one tap: 10: …"), jadi jenis dan pesannya
// diperiksa dulu sebelum kelasnya.
package laporin.masukgoogle

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.os.CancellationSignal
import android.view.View
import androidx.credentials.CredentialManager
import androidx.credentials.CredentialManagerCallback
import androidx.credentials.CustomCredential
import androidx.credentials.GetCredentialRequest
import androidx.credentials.GetCredentialResponse
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.GetCredentialInterruptedException
import androidx.credentials.exceptions.GetCredentialProviderConfigurationException
import androidx.credentials.exceptions.GetCredentialUnsupportedException
import androidx.credentials.exceptions.NoCredentialException
import com.google.android.libraries.identity.googleid.GetSignInWithGoogleOption
import com.google.android.libraries.identity.googleid.GoogleIdTokenCredential
import com.google.android.libraries.identity.googleid.GoogleIdTokenParsingException
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

private const val BELUM_DISIAPKAN = "Masuk dengan Google belum disiapkan untuk aplikasi ini (paket & SHA-1 belum terdaftar)."

/** Tanda DEVELOPER_ERROR Play Services di jenis atau pesan galat: paket + SHA-1, atau client ID, tidak cocok. */
private val TANDA_KONFIGURASI = Regex("""28444|developer console|developer_error|one tap: 10:""", RegexOption.IGNORE_CASE)

/** Jenis kredensial ID token Google: alur klasik, dan alur tombol pada googleid 1.2+ (lewat Credential Manager Android 14+). */
private val JENIS_GOOGLE = setOf(
    GoogleIdTokenCredential.TYPE_GOOGLE_ID_TOKEN_CREDENTIAL,
    GoogleIdTokenCredential.TYPE_GOOGLE_ID_TOKEN_SIWG_CREDENTIAL,
)

/** Keadaan satu `masuk-google`. */
private class Keadaan(val emit: (String, String) -> Unit) {
    var klien = ""
    /** `minta` terakhir yang dilihat; null sebelum props pertama. */
    var terakhir: Double? = null
    /** Percobaan yang sedang berjalan, dibatalkan bila view-nya pergi; null bila tidak ada. */
    var sinyal: CancellationSignal? = null
}

class MasukGoogleModule(context: ExtensionContext) : ExactExtension(context) {
    private val ext = context
    private val keadaan = HashMap<View, Keadaan>()
    // Dibuat dari aplikasi (tidak menahan activity); activity yang tampil diberikan pada setiap permintaan.
    private val manager by lazy { CredentialManager.create(ext.application) }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "masuk-google") { "no view $tag" }
        // Tidak menggambar apa pun dan tidak dibaca pembaca layar: hanya pemegang keadaan untuk node-nya.
        val v = View(context)
        v.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        keadaan[v] = Keadaan(emit)
        update(v, props)
        return v
    }

    override fun update(view: View, props: JSONObject) {
        val k = keadaan[view] ?: return
        k.klien = props.optString("klien").trim()
        val minta = props.optDouble("minta", 0.0).takeUnless { it.isNaN() } ?: 0.0
        val lalu = k.terakhir
        k.terakhir = minta
        // Props datang utuh pada setiap perubahan: hanya nomor baru > 0 yang memulai. Props pertama sekadar dicatat.
        if (lalu == null || minta == lalu || minta <= 0.0) return
        if (k.sinyal != null) return ext.log("masuk-google: minta $minta diabaikan, percobaan sebelumnya masih berjalan")
        mulai(view, k)
    }

    // Percobaan yang masih berjalan dibatalkan: jawabannya, bila tetap datang, tidak dikirim ke mana-mana.
    override fun destroyView(view: View) {
        keadaan.remove(view)?.let { it.sinyal?.cancel(); it.sinyal = null }
    }

    override fun destroy(done: () -> Unit) {
        keadaan.values.forEach { it.sinyal?.cancel(); it.sinyal = null }
        keadaan.clear()
        done()
    }

    /** Satu percobaan masuk: pemilih akun Google di atas activity yang tampil, lalu tepat satu `message`. */
    private fun mulai(view: View, k: Keadaan) {
        if (k.klien.isEmpty()) return k.emit("message", "galat Client ID Google belum diisi saat build.")
        // Credential Manager harus diberi konteks Activity (ia membuka activity tersembunyi Play Services di atasnya).
        val activity = ext.activity ?: aktivitas(view.context)
        if (activity == null || activity.isFinishing || activity.isDestroyed || !ext.resumed)
            return k.emit("message", "galat Aplikasi sedang tidak tampil; coba lagi.")
        val sinyal = CancellationSignal()
        k.sinyal = sinyal
        try {
            val opsi = GetSignInWithGoogleOption.Builder(k.klien).build()
            val permintaan = GetCredentialRequest.Builder().addCredentialOption(opsi).build()
            manager.getCredentialAsync(activity, permintaan, sinyal, activity.mainExecutor,
                object : CredentialManagerCallback<GetCredentialResponse, GetCredentialException> {
                    override fun onResult(result: GetCredentialResponse) = selesai(k, sinyal, hasil(result))
                    override fun onError(e: GetCredentialException) = selesai(k, sinyal, gagal(e))
                })
        } catch (e: Exception) {
            ext.log("masuk-google: ${mentah(e)}")
            selesai(k, sinyal, "galat Masuk dengan Google gagal dimulai. · ${mentah(e)}")
        }
    }

    /** Pesan satu percobaan, sekali: tidak bila percobaannya sudah dibatalkan (view-nya pergi). */
    private fun selesai(k: Keadaan, sinyal: CancellationSignal, pesan: String) {
        if (k.sinyal !== sinyal) return
        k.sinyal = null
        k.emit("message", pesan)
    }

    /** ID token dari jawaban Credential Manager. Tokennya tidak pernah ditulis ke log. */
    private fun hasil(result: GetCredentialResponse): String {
        val c = result.credential
        if (c !is CustomCredential || c.type !in JENIS_GOOGLE) return "galat Google mengirim jenis kredensial yang tidak dikenal. · ${c.type}"
        return try {
            val token = GoogleIdTokenCredential.createFrom(c.data).idToken
            if (token.isEmpty()) "galat Google tidak mengirim ID token. · ${c.type}" else "token $token"
        } catch (e: GoogleIdTokenParsingException) {
            "galat Jawaban Google tidak bisa dibaca. · ${mentah(e)}"
        }
    }

    /** Galat Credential Manager sebagai pesan untuk Contract: "batal" bila orang itu menutup pemilih akunnya. */
    private fun gagal(e: GetCredentialException): String {
        ext.log("masuk-google: ${mentah(e)}")
        val teks = "${e.type} ${e.errorMessage ?: e.message ?: ""}"
        val sebab = when {
            TANDA_KONFIGURASI.containsMatchIn(teks) -> BELUM_DISIAPKAN
            // Akun sudah dipilih tetapi Google menolaknya: bukan orang itu yang membatalkan.
            e is GetCredentialCancellationException && teks.contains("reauth failed", ignoreCase = true) ->
                "Google menolak masuk dengan akun itu (biasanya paket & SHA-1 aplikasi belum terdaftar)."
            e is GetCredentialCancellationException -> return "batal"
            // Galat Play Services lain yang dibungkus sebagai NoCredentialException, bukan HP tanpa akun.
            e is NoCredentialException && teks.contains("failure response from one tap", ignoreCase = true) ->
                "Layanan Google Play menolak permintaan masuk."
            e is NoCredentialException -> "Tidak ada akun Google di HP ini."
            e is GetCredentialProviderConfigurationException ->
                "Layanan Google Play di HP ini belum mendukung Masuk dengan Google; perbarui Layanan Google Play."
            e is GetCredentialInterruptedException -> "Masuk dengan Google terputus (periksa internet); coba lagi."
            e is GetCredentialUnsupportedException -> "HP ini tidak mendukung Masuk dengan Google."
            else -> "Masuk dengan Google gagal."
        }
        return "galat $sebab · ${mentah(e)}"
    }

    /**
     * Kelas, jenis dan pesan asli galat, satu baris, untuk men-debug. Jenis Credential Manager (mis. `TYPE_NO_CREDENTIAL`)
     * ikut ditulis karena R8 menyamarkan nama kelas di build rilis.
     */
    private fun mentah(e: Exception): String {
        val cm = e as? GetCredentialException
        val nama = listOfNotNull(e.javaClass.simpleName, cm?.type?.substringAfterLast('.')).joinToString("/")
        val pesan = (cm?.errorMessage ?: e.message)?.toString().orEmpty()
        return (if (pesan.isEmpty()) nama else "$nama: $pesan").replace('\n', ' ')
    }
}

/** Activity di balik konteks view, bila host tidak sedang menunjuk satu. */
private tailrec fun aktivitas(c: Context?): Activity? = when (c) {
    is Activity -> c
    is ContextWrapper -> aktivitas(c.baseContext)
    else -> null
}
