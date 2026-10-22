package ai.testcabinet.client

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.os.Message
import android.view.View
import android.webkit.ConsoleMessage
import android.webkit.GeolocationPermissions
import android.webkit.JsPromptResult
import android.webkit.JsResult
import android.webkit.PermissionRequest
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient

/**
 * The runtime's `WebViewClient`, with its navigation handler asked about the
 * main frame alone.
 *
 * The runtime hands every navigation `shouldOverrideUrlLoading` reports to
 * the shell's navigation handler, a frame's as much as the page's, and the
 * handler cannot tell them apart. The shell's handler keeps the WebView on
 * the console and its sign-in flow, which a frame of the console's page,
 * such as a run's playable build on its own origin, is not held to, so a
 * frame's navigation is let through here without asking. Every other
 * callback the runtime answers is passed on to it unchanged.
 */
class MainFrameClient(private val inner: WebViewClient) : WebViewClient() {
  override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
    inner.shouldInterceptRequest(view, request)

  override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
    request.isForMainFrame && inner.shouldOverrideUrlLoading(view, request)

  override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) =
    inner.onPageStarted(view, url, favicon)

  override fun onPageFinished(view: WebView, url: String) = inner.onPageFinished(view, url)

  override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) =
    inner.onReceivedError(view, request, error)
}

/**
 * The runtime's `WebChromeClient`, with a new window opened in the platform's
 * browser.
 *
 * The console's own `window.open` and links with a target are caught by the
 * shell's script, which runs in the console's page alone. A frame of the
 * page, such as a run's playable build, opening a window would otherwise
 * replace the console in the WebView, so the WebView is told it supports
 * windows and each one it asks for is a throwaway whose first web URL goes
 * to the browser. Every other callback the runtime answers is passed on to
 * it unchanged.
 */
class NewWindowClient(private val context: Context, private val inner: WebChromeClient) :
  WebChromeClient() {
  override fun onCreateWindow(
    view: WebView,
    isDialog: Boolean,
    isUserGesture: Boolean,
    resultMsg: Message
  ): Boolean {
    val transport = resultMsg.obj as? WebView.WebViewTransport ?: return false
    val popup = WebView(context)
    var done = false
    fun finish(url: Uri?) {
      if (done) return
      done = true
      if (url != null && (url.scheme == "https" || url.scheme == "http")) openInBrowser(url)
      popup.post { popup.destroy() }
    }
    popup.webViewClient = object : WebViewClient() {
      override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
        finish(request.url)
        return true
      }

      override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
        // A blank window a page writes into has no page to hand on.
        if (url != "about:blank") {
          view.stopLoading()
          finish(Uri.parse(url))
        }
      }
    }
    // A window that never navigates is let go after a while.
    popup.postDelayed({ finish(null) }, ABANDONED_WINDOW_MS)
    transport.webView = popup
    resultMsg.sendToTarget()
    return true
  }

  private fun openInBrowser(url: Uri) {
    try {
      context.startActivity(
        Intent(Intent.ACTION_VIEW, url).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      )
    } catch (error: ActivityNotFoundException) {
      // No browser is installed; the console stays where it is.
    }
  }

  override fun onShowCustomView(view: View, callback: CustomViewCallback) =
    inner.onShowCustomView(view, callback)

  override fun onHideCustomView() = inner.onHideCustomView()

  override fun onPermissionRequest(request: PermissionRequest) = inner.onPermissionRequest(request)

  override fun getDefaultVideoPoster(): Bitmap? = inner.defaultVideoPoster

  override fun onJsAlert(view: WebView, url: String, message: String, result: JsResult): Boolean =
    inner.onJsAlert(view, url, message, result)

  override fun onJsConfirm(view: WebView, url: String, message: String, result: JsResult): Boolean =
    inner.onJsConfirm(view, url, message, result)

  override fun onJsPrompt(
    view: WebView,
    url: String,
    message: String,
    defaultValue: String?,
    result: JsPromptResult
  ): Boolean = inner.onJsPrompt(view, url, message, defaultValue, result)

  override fun onGeolocationPermissionsShowPrompt(
    origin: String,
    callback: GeolocationPermissions.Callback
  ) = inner.onGeolocationPermissionsShowPrompt(origin, callback)

  override fun onShowFileChooser(
    webView: WebView,
    filePathCallback: ValueCallback<Array<Uri>>,
    fileChooserParams: FileChooserParams
  ): Boolean = inner.onShowFileChooser(webView, filePathCallback, fileChooserParams)

  override fun onConsoleMessage(consoleMessage: ConsoleMessage): Boolean =
    inner.onConsoleMessage(consoleMessage)

  override fun onReceivedTitle(view: WebView, title: String?) = inner.onReceivedTitle(view, title)

  override fun onProgressChanged(view: WebView, newProgress: Int) =
    inner.onProgressChanged(view, newProgress)

  private companion object {
    /** How long a new window that never navigates is kept, in milliseconds. */
    const val ABANDONED_WINDOW_MS = 30_000L
  }
}
