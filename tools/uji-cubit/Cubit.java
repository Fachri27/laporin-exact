import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.MotionEvent;
import java.lang.reflect.Method;

/** Cubit dua jari untuk uji: app_process ... Cubit cx cy jarakAwal jarakAkhir langkah msPerLangkah */
public class Cubit {
    static Object im; static Method inject;
    static void kirim(MotionEvent e) throws Exception { inject.invoke(im, e, 2); e.recycle(); }
    static MotionEvent ev(long down, int action, float[][] p) {
        int n = p.length;
        MotionEvent.PointerProperties[] pp = new MotionEvent.PointerProperties[n];
        MotionEvent.PointerCoords[] pc = new MotionEvent.PointerCoords[n];
        for (int i = 0; i < n; i++) {
            pp[i] = new MotionEvent.PointerProperties(); pp[i].id = i; pp[i].toolType = MotionEvent.TOOL_TYPE_FINGER;
            pc[i] = new MotionEvent.PointerCoords(); pc[i].x = p[i][0]; pc[i].y = p[i][1]; pc[i].pressure = 1f; pc[i].size = 1f;
        }
        return MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, n, pp, pc, 0, 0, 1f, 1f, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
    }
    public static void main(String[] a) throws Exception {
        float cx = Float.parseFloat(a[0]), cy = Float.parseFloat(a[1]), d0 = Float.parseFloat(a[2]), d1 = Float.parseFloat(a[3]);
        int n = Integer.parseInt(a[4]); long ms = Long.parseLong(a[5]);
        Class<?> c = Class.forName("android.hardware.input.InputManagerGlobal");
        im = c.getMethod("getInstance").invoke(null);
        inject = c.getMethod("injectInputEvent", InputEvent.class, int.class);
        long down = SystemClock.uptimeMillis();
        kirim(ev(down, MotionEvent.ACTION_DOWN, new float[][]{{cx - d0 / 2, cy}}));
        Thread.sleep(40);
        kirim(ev(down, MotionEvent.ACTION_POINTER_DOWN | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT), new float[][]{{cx - d0 / 2, cy}, {cx + d0 / 2, cy}}));
        for (int i = 1; i <= n; i++) {
            Thread.sleep(ms);
            float d = d0 + (d1 - d0) * i / n;
            kirim(ev(down, MotionEvent.ACTION_MOVE, new float[][]{{cx - d / 2, cy}, {cx + d / 2, cy}}));
        }
        Thread.sleep(ms);
        kirim(ev(down, MotionEvent.ACTION_POINTER_UP | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT), new float[][]{{cx - d1 / 2, cy}, {cx + d1 / 2, cy}}));
        kirim(ev(down, MotionEvent.ACTION_UP, new float[][]{{cx - d1 / 2, cy}}));
    }
}
