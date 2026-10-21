package ai.testcabinet.client

import android.graphics.Color
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.widget.FrameLayout
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * The activity the console runs in.
 *
 * `TauriActivity` is generated beside this file by the build and hosts the
 * WebView. The window is drawn edge to edge with both system bars
 * transparent, and the activity lays the WebView out between the bars itself,
 * since a WebView reports no insets to a page. It keeps the WebView clear of
 * the status bar, the navigation bar, a display cutout and the keyboard, and
 * paints the two bands the bars occupy with the `status_bar` and
 * `navigation_bar` colors, the console's background.
 *
 * Sign-in leaves the Cloudflare Access cookie in the WebView's cookie store,
 * which Android writes to disk on its own schedule. The activity flushes the
 * store whenever it pauses, so a sign-in survives the process being killed in
 * the background.
 */
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)

    val content = findViewById<FrameLayout>(android.R.id.content)
    val statusBand = band(R.color.status_bar, Gravity.TOP)
    val navigationBand = band(R.color.navigation_bar, Gravity.BOTTOM)
    content.addView(statusBand)
    content.addView(navigationBand)

    ViewCompat.setOnApplyWindowInsetsListener(content) { _, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or
          WindowInsetsCompat.Type.displayCutout() or
          WindowInsetsCompat.Type.ime()
      )
      resize(statusBand, bars.top)
      resize(navigationBand, bars.bottom)
      for (index in 0 until content.childCount) {
        val child = content.getChildAt(index)
        if (child === statusBand || child === navigationBand) continue
        val params = child.layoutParams as? ViewGroup.MarginLayoutParams ?: continue
        params.setMargins(bars.left, bars.top, bars.right, bars.bottom)
        child.layoutParams = params
      }
      WindowInsetsCompat.CONSUMED
    }
  }

  override fun onPause() {
    super.onPause()
    CookieManager.getInstance().flush()
  }

  /** A band the width of the window, painted with `color`, against `gravity`. */
  private fun band(color: Int, gravity: Int): View =
    View(this).apply {
      setBackgroundColor(ContextCompat.getColor(context, color))
      layoutParams = FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, 0, gravity)
    }

  /** Gives `band` the height `height`, which is one bar's inset. */
  private fun resize(band: View, height: Int) {
    val params = band.layoutParams
    params.height = height
    band.layoutParams = params
  }
}
