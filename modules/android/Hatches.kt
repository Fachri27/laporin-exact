package exact.app

import dev.exact.canvas.ExactApp
import dev.exact.canvas.ExactElement
import dev.exact.canvas.ExactHatches
import dev.exact.canvas.ExactInput
import dev.exact.canvas.ExactTicket
import laporin.web.Sentuhan

/**
 * Laporin's hatches on Android. Written by `exact hatch`; yours from here. Called on the main thread. A node is
 * painted by exact2: a hatch has its box and words, acts on it as a person would (click, focus, blur, input), hears
 * its input (observe), shows a view of its own in its box (host), and keeps time by exact2's clock (frames, after).
 */
class Hatches : ExactHatches() {
    /** Pendengar `sentuh-web` per node. */
    private val dengar = HashMap<Int, ExactTicket>()

    override fun element(element: ExactElement) {
        when (element.hatch) {
            "sentuh-web" -> sentuhWeb(element)
            // exact:element (exact hatch adds a word's arm above this one)
        }
    }

    override fun elementEnded(element: ExactElement) {
        when (element.hatch) {
            "sentuh-web" -> sentuhWebEnded(element)
            // exact:elementEnded
        }
    }

    override fun app(app: ExactApp) {
        // exact:app
    }

    /**
     * `sentuh-web` di sebuah `web-halaman` (paket/web): host Android Exact tidak menyentuhkan apa pun ke view native,
     * jadi pointer yang mendarat di node itu diteruskan ke WebView dengan `kunci` = `data-kunci` node, sebagai
     * sentuhan sungguhan (tombol dan geseran di halaman hidup). Handle node tetap sama saat kotaknya berubah, jadi
     * lebarnya selalu yang terbaru.
     */
    private fun sentuhWeb(element: ExactElement) {
        if (dengar.containsKey(element.node)) return
        dengar[element.node] = observe(element) { input ->
            if (input is ExactInput.Pointer) {
                Sentuhan.teruskan(element.data("kunci").orEmpty(), input.phase, input.localX, input.localY, element.frame.width())
            }
        }
    }

    /** The node has left: stop what the hatch started for it. */
    private fun sentuhWebEnded(element: ExactElement) {
        dengar.remove(element.node)?.stop()
    }

    // exact:functions (exact hatch adds a word's functions above this line)
}
