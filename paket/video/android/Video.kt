// @laporin/video di Android: `video-putar` adalah ExoPlayer (AndroidX Media3) yang menggambar ke TextureView.
// TextureView, bukan SurfaceView: host Exact merekam gambar view native ke RenderNode di alur kanvas, jadi tombol
// Contract tetap tampil di atas video. Props: `sumber` (alamat https), `bisu`, `putar` (false = jeda), `pas` ("cover"
// atau "contain"), `lompat` (detik) yang dituju setiap kali `lompatKe` berubah. Video berulang seperti di laporin-apps.
// Mengirim `message` "memuat", "main", "jeda", "galat: …" dan, tiap 250 ms selama main serta sesudah lompat,
// "waktu <posisi ms> <durasi ms>" supaya Contract bisa menggambar status, kemajuan dan penggesernya. `waktu=false`
// mematikan pesan waktu itu: setiap pesan menjalankan aksi Contract (commit dan gambar ulang di thread Exact), jadi
// video yang tidak menampilkan kemajuan (sorotan Beranda) tidak membebani gulir.
//
// Seperti expo-video di aplikasi aslinya (`useCaching`, bufferOptions): semua pemutar berbagi satu tembolok disk
// (SimpleCache LRU 250 MB di cacheDir/video), jadi video yang pernah diputar mulai dari disk; buffernya kecil
// (2–10 s, paling banyak 12 MB per pemutar) dan video main begitu 0,7 s termuat. Berkas .mov yang `moov`-nya di akhir
// dibaca ExoPlayer dengan permintaan Range ke ujung berkas, tanpa mengunduh seluruh isinya lebih dulu.
package laporin.video

import android.content.Context
import android.graphics.Matrix
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.TextureView
import android.view.View
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.database.StandaloneDatabaseProvider
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.HttpDataSource
import androidx.media3.datasource.cache.CacheDataSource
import androidx.media3.datasource.cache.LeastRecentlyUsedCacheEvictor
import androidx.media3.datasource.cache.SimpleCache
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.io.File
import kotlin.math.roundToInt

private const val TAG = "laporin.video"

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

/**
 * Sumber data video untuk seluruh proses: http(s) lewat satu tembolok disk, berkas lokal (file:, content:) langsung.
 * SimpleCache hanya boleh satu per folder (instans kedua melempar IllegalStateException), maka dibuat sekali di sini.
 * Bila tembolok tidak bisa dibuka, video tetap diputar langsung dari jaringan.
 */
@OptIn(UnstableApi::class)
private object Tembolok {
    private var pabrik: DataSource.Factory? = null

    @Synchronized
    fun sumberData(context: Context): DataSource.Factory =
        pabrik ?: buat(context.applicationContext).also { pabrik = it }

    private fun buat(app: Context): DataSource.Factory {
        val http = DefaultHttpDataSource.Factory()
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(10_000)
            .setReadTimeoutMs(15_000)
        val jaringan: DataSource.Factory = try {
            val simpanan = SimpleCache(
                File(app.cacheDir, "video"),
                LeastRecentlyUsedCacheEvictor(250L * 1024 * 1024),
                StandaloneDatabaseProvider(app),
            )
            CacheDataSource.Factory()
                .setCache(simpanan)
                .setUpstreamDataSourceFactory(http)
                // Galat tembolok (berkas terhapus, disk penuh) tidak menghentikan video: bacaannya lanjut dari jaringan.
                .setFlags(CacheDataSource.FLAG_IGNORE_CACHE_ON_ERROR)
        } catch (e: Exception) {
            Log.w(TAG, "tembolok video tidak bisa dibuka; video diputar tanpa tembolok", e)
            http
        }
        return DefaultDataSource.Factory(app, jaringan)
    }
}

@OptIn(UnstableApi::class)
private class Pemutar(
    private val context: Context,
    private val tekstur: TextureView,
    private val emit: (String, String) -> Unit,
) : Player.Listener, View.OnAttachStateChangeListener {
    private var sumber = ""
    private var bisu = true
    private var putar = true
    private var cover = false
    private var diLatar = false
    /** Sumber kini sudah sekali READY, seperti onPrepared MediaPlayer: baru sesudahnya props diterapkan ke pemutar. */
    private var siap = false
    private var pemain: ExoPlayer? = null
    private var lebarVideo = 0
    private var tinggiVideo = 0
    private var lompatKe: Int? = null
    private var lompatTertunda = -1
    /** Prop `waktu`: kirim "waktu …" tiap 250 ms selama main. */
    private var lapor = true
    private val jam = Handler(Looper.getMainLooper())
    private val detak = object : Runnable {
        override fun run() {
            laporWaktu()
            if (siap && lapor && pemain?.playWhenReady == true) jam.postDelayed(this, 250)
        }
    }
    // Host mengikat `emit` view baru sesudah memasangnya ke jendela: pesan yang dikirim saat terpasang hilang, jadi
    // pemuatan pertama menunggu giliran berikutnya.
    private val muatNanti = Runnable { if (pemain == null) muat() }

    init {
        tekstur.addOnAttachStateChangeListener(this)
        tekstur.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> sesuaikan() }
    }

    /** Props datang utuh pada setiap perubahan: sumber baru dimuat ulang, sisanya diterapkan ke pemutar. */
    fun atur(props: JSONObject) {
        bisu = props.optBoolean("bisu", true)
        lapor = props.optBoolean("waktu", true)
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

    /**
     * Memuat `sumber` dari awal, hanya selagi view terpasang di jendela (dimuat begitu terpasang). Pemutar yang ada
     * dipakai lagi untuk sumber baru, mulai dalam keadaan jeda seperti MediaPlayer yang baru disiapkan.
     */
    private fun muat() {
        jam.removeCallbacks(muatNanti)
        jam.removeCallbacks(detak)
        siap = false
        if (sumber.isEmpty() || !tekstur.isAttachedToWindow) {
            lepasPemutar()
            return
        }
        emit("message", "memuat")
        try {
            val p = pemain ?: buatPemain().also { pemain = it }
            p.pause()
            p.setMediaItem(MediaItem.fromUri(sumber))
            p.prepare()
        } catch (e: Exception) {
            lepasPemutar()
            emit("message", "galat: ${e.message}")
        }
    }

    private fun buatPemain(): ExoPlayer {
        // bufferOptions expo-video aslinya: main begitu 0,7 s termuat (1,5 s sesudah tersendat), simpan 2–10 s ke
        // depan, dan berhenti memuat di 12 MB meski 10 s belum tercapai, supaya beberapa pemutar muat di memori.
        val muatan = DefaultLoadControl.Builder()
            .setBufferDurationsMs(2_000, 10_000, 700, 1_500)
            .setTargetBufferBytes(12 * 1024 * 1024)
            .setPrioritizeTimeOverSizeThresholds(false)
            .build()
        val p = ExoPlayer.Builder(context, DefaultRenderersFactory(context).setEnableDecoderFallback(true))
            .setMediaSourceFactory(DefaultMediaSourceFactory(Tembolok.sumberData(context)))
            .setLoadControl(muatan)
            // Tanpa fokus audio, seperti MediaPlayer: video sorotan yang bisu tidak menghentikan musik aplikasi lain.
            .setAudioAttributes(
                AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(),
                false,
            )
            .build()
        p.repeatMode = Player.REPEAT_MODE_ONE
        // ExoPlayer memasang SurfaceTextureListener-nya sendiri pada tekstur dan membuat Surface-nya saat tersedia.
        p.setVideoTextureView(tekstur)
        p.addListener(this)
        return p
    }

    private fun terapkan() {
        val p = pemain ?: return
        if (!siap) return
        p.volume = if (bisu) 0f else 1f
        if (lompatTertunda >= 0) {
            val durasi = p.duration
            p.seekTo(if (durasi > 0) lompatTertunda.toLong().coerceAtMost(durasi) else lompatTertunda.toLong())
            lompatTertunda = -1
        }
        // playWhenReady = "sedang diputar" MediaPlayer: tetap benar selama tersendat memuat.
        val main = putar && !diLatar
        if (main && !p.playWhenReady) {
            p.play()
            emit("message", "main")
            jam.removeCallbacks(detak)
            jam.post(detak)
        } else if (!main && p.playWhenReady) {
            p.pause()
            emit("message", "jeda")
            laporWaktu()
        }
    }

    private fun laporWaktu() {
        val p = pemain ?: return
        if (!siap || !lapor) return
        emit("message", "waktu ${p.currentPosition} ${p.duration.coerceAtLeast(0)}")
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

    override fun onPlaybackStateChanged(keadaan: Int) {
        val p = pemain ?: return
        when (keadaan) {
            // Tersendat sesudah siap (BUFFERING_START MediaPlayer); "memuat" pemuatan awal sudah dikirim [muat].
            Player.STATE_BUFFERING -> if (siap) emit("message", "memuat")
            Player.STATE_READY ->
                if (!siap) {
                    siap = true
                    terapkan()
                    laporWaktu()
                } else {
                    emit("message", if (p.playWhenReady) "main" else "jeda")
                }
        }
    }

    // Bingkai tampil sesudah siap (VIDEO_RENDERING_START MediaPlayer), mis. permukaan tekstur yang datang belakangan.
    override fun onRenderedFirstFrame() {
        val p = pemain ?: return
        if (siap) emit("message", if (p.playWhenReady) "main" else "jeda")
    }

    override fun onPositionDiscontinuity(lama: Player.PositionInfo, baru: Player.PositionInfo, alasan: Int) {
        if (alasan == Player.DISCONTINUITY_REASON_SEEK || alasan == Player.DISCONTINUITY_REASON_SEEK_ADJUSTMENT) laporWaktu()
    }

    override fun onVideoSizeChanged(ukuran: VideoSize) {
        if (ukuran.width == 0 || ukuran.height == 0) return
        // Ukuran sesudah rotasi diterapkan dekoder; piksel tak persegi dilebarkan ke ukuran tampilnya.
        lebarVideo = (ukuran.width * ukuran.pixelWidthHeightRatio).roundToInt()
        tinggiVideo = ukuran.height
        sesuaikan()
    }

    override fun onPlayerError(galat: PlaybackException) {
        siap = false
        jam.removeCallbacks(detak)
        Log.w(TAG, "video gagal: $sumber", galat)
        // "galat: <kode PlaybackException>/<status HTTP, atau 0>", sebentuk "galat: what/extra" MediaPlayer.
        val http = (galat.cause as? HttpDataSource.InvalidResponseCodeException)?.responseCode ?: 0
        emit("message", "galat: ${galat.errorCode}/$http")
    }

    override fun onViewAttachedToWindow(v: View) {
        jam.removeCallbacks(muatNanti)
        jam.post(muatNanti)
    }

    // Lepas dari jendela = permukaan tekstur hilang: pemutar dilepas, dan dimuat dari awal saat terpasang lagi.
    override fun onViewDetachedFromWindow(v: View) {
        jam.removeCallbacks(muatNanti)
        lepasPemutar()
    }

    private fun lepasPemutar() {
        jam.removeCallbacks(detak)
        siap = false
        pemain?.let {
            it.removeListener(this)
            it.release()
        }
        pemain = null
    }

    fun lepas() {
        tekstur.removeOnAttachStateChangeListener(this)
        jam.removeCallbacks(muatNanti)
        lepasPemutar()
    }
}
