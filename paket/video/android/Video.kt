// @laporin/video di Android: `video-putar` adalah MediaPlayer yang menggambar ke TextureView. TextureView, bukan
// SurfaceView: host Exact merekam gambar view native ke RenderNode di alur kanvas, jadi tombol Contract tetap
// tampil di atas video. Props: `sumber` (alamat https), `bisu`, `putar` (false = jeda), `pas` ("cover" atau
// "contain"), `lompat` (detik) yang dituju setiap kali `lompatKe` berubah. Video berulang seperti di laporin-apps.
// Mengirim `message` "memuat", "main", "jeda", "galat: …" dan, tiap 250 ms selama main serta sesudah lompat,
// "waktu <posisi ms> <durasi ms>" supaya Contract bisa menggambar status, kemajuan dan penggesernya.
package laporin.video

import android.content.Context
import android.graphics.Matrix
import android.graphics.SurfaceTexture
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.view.Surface
import android.view.TextureView
import android.view.View
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class VideoModule(context: ExtensionContext) : ExactExtension(context) {
    private val pemutar = HashMap<View, Pemutar>()

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) =
        reply(JSONObject().put("error", "NotSupportedError"))

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "video-putar") { "no view $tag" }
        val tekstur = TextureView(context).apply { isOpaque = false }
        pemutar[tekstur] = Pemutar(context, tekstur, emit).also { it.atur(props) }
        return tekstur
    }

    override fun update(view: View, props: JSONObject) {
        pemutar[view]?.atur(props)
    }

    override fun destroyView(view: View) {
        pemutar.remove(view)?.lepas()
    }

    // Aplikasi ke latar: semua video dijeda; kembali ke depan: yang tadinya diputar jalan lagi.
    override fun paused() = pemutar.values.forEach { it.latar(true) }

    override fun resumed() = pemutar.values.forEach { it.latar(false) }
}

private class Pemutar(
    private val context: Context,
    private val tekstur: TextureView,
    private val emit: (String, String) -> Unit,
) : TextureView.SurfaceTextureListener {
    private var sumber = ""
    private var bisu = true
    private var putar = true
    private var cover = false
    private var diLatar = false
    private var siap = false
    private var mp: MediaPlayer? = null
    private var permukaan: Surface? = null
    private var lebarVideo = 0
    private var tinggiVideo = 0
    private var lompatKe: Int? = null
    private var lompatTertunda = -1
    private val jam = Handler(Looper.getMainLooper())
    private val detak = object : Runnable {
        override fun run() {
            laporWaktu()
            if (mp?.isPlaying == true) jam.postDelayed(this, 250)
        }
    }

    init {
        tekstur.surfaceTextureListener = this
        tekstur.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> sesuaikan() }
    }

    /** Props datang utuh pada setiap perubahan: sumber baru dimuat ulang, sisanya diterapkan ke pemutar. */
    fun atur(props: JSONObject) {
        bisu = props.optBoolean("bisu", true)
        putar = props.optBoolean("putar", true)
        val coverBaru = props.optString("pas") == "cover"
        if (coverBaru != cover) {
            cover = coverBaru
            sesuaikan()
        }
        // Lompat hanya saat penghitungnya berubah: lompatan pertama yang terlihat adalah nilai awalnya.
        val ke = props.optInt("lompatKe", 0)
        if (lompatKe != null && ke != lompatKe) lompatTertunda = (props.optDouble("lompat", 0.0) * 1000).toInt().coerceAtLeast(0)
        lompatKe = ke
        val s = props.optString("sumber")
        if (s != sumber) {
            sumber = s
            lompatTertunda = -1
            muat()
        } else {
            terapkan()
        }
    }

    fun latar(ya: Boolean) {
        diLatar = ya
        terapkan()
    }

    private fun muat() {
        lepasPemutar()
        val surf = permukaan ?: return // dimuat begitu permukaannya tersedia
        if (sumber.isEmpty()) return
        emit("message", "memuat")
        val m = MediaPlayer()
        mp = m
        m.setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MOVIE)
                .build(),
        )
        m.setSurface(surf)
        m.isLooping = true
        m.setOnPreparedListener {
            if (mp !== it) return@setOnPreparedListener
            siap = true
            terapkan()
            laporWaktu()
        }
        m.setOnSeekCompleteListener { if (mp === it) laporWaktu() }
        m.setOnVideoSizeChangedListener { _, w, h ->
            lebarVideo = w
            tinggiVideo = h
            sesuaikan()
        }
        m.setOnInfoListener { p, what, _ ->
            when (what) {
                MediaPlayer.MEDIA_INFO_BUFFERING_START -> emit("message", "memuat")
                MediaPlayer.MEDIA_INFO_BUFFERING_END, MediaPlayer.MEDIA_INFO_VIDEO_RENDERING_START ->
                    emit("message", if (p.isPlaying) "main" else "jeda")
            }
            false
        }
        m.setOnErrorListener { p, what, extra ->
            if (mp === p) {
                siap = false
                emit("message", "galat: $what/$extra")
            }
            true
        }
        try {
            m.setDataSource(context, Uri.parse(sumber))
            m.prepareAsync()
        } catch (e: Exception) {
            emit("message", "galat: ${e.message}")
        }
    }

    private fun terapkan() {
        val m = mp ?: return
        if (!siap) return
        val volume = if (bisu) 0f else 1f
        m.setVolume(volume, volume)
        if (lompatTertunda >= 0) {
            m.seekTo(lompatTertunda.toLong().coerceAtMost(m.duration.toLong()), MediaPlayer.SEEK_CLOSEST)
            lompatTertunda = -1
        }
        val main = putar && !diLatar
        if (main && !m.isPlaying) {
            m.start()
            emit("message", "main")
            jam.removeCallbacks(detak)
            jam.post(detak)
        } else if (!main && m.isPlaying) {
            m.pause()
            emit("message", "jeda")
            laporWaktu()
        }
    }

    private fun laporWaktu() {
        val m = mp ?: return
        if (!siap) return
        emit("message", "waktu ${m.currentPosition} ${m.duration.coerceAtLeast(0)}")
    }

    /** Video di tengah view: "contain" utuh dengan bilah latar, "cover" memenuhi view dan memotong sisanya. */
    private fun sesuaikan() {
        val vw = tekstur.width.toFloat()
        val vh = tekstur.height.toFloat()
        if (vw == 0f || vh == 0f || lebarVideo == 0 || tinggiVideo == 0) return
        val skala = if (cover) maxOf(vw / lebarVideo, vh / tinggiVideo) else minOf(vw / lebarVideo, vh / tinggiVideo)
        val matriks = Matrix()
        matriks.setScale(lebarVideo * skala / vw, tinggiVideo * skala / vh, vw / 2, vh / 2)
        tekstur.setTransform(matriks)
    }

    override fun onSurfaceTextureAvailable(st: SurfaceTexture, width: Int, height: Int) {
        permukaan = Surface(st)
        muat()
    }

    override fun onSurfaceTextureSizeChanged(st: SurfaceTexture, width: Int, height: Int) = sesuaikan()

    override fun onSurfaceTextureDestroyed(st: SurfaceTexture): Boolean {
        lepasPemutar()
        permukaan?.release()
        permukaan = null
        return true
    }

    override fun onSurfaceTextureUpdated(st: SurfaceTexture) {}

    private fun lepasPemutar() {
        jam.removeCallbacks(detak)
        siap = false
        mp?.release()
        mp = null
    }

    fun lepas() {
        lepasPemutar()
        permukaan?.release()
        permukaan = null
    }
}
