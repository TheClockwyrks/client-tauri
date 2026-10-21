package ai.testcabinet.client

import android.app.Activity
import android.app.DownloadManager
import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.webkit.CookieManager
import android.webkit.URLUtil
import android.webkit.WebView
import androidx.annotation.RequiresApi
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.IOException
import org.json.JSONObject

/** The arguments of [ShellPlugin.saveDownload]. */
@InvokeArg
class SaveDownloadArgs {
  /** A file in the application's cache holding the download's bytes. */
  lateinit var path: String
  /** The file name the console offered, already made safe by the shell. */
  lateinit var name: String
  /** The download's media type, empty where the page named none. */
  var mimeType: String? = null
}

/**
 * The shell's Android side, registered by the Rust crate's `android` module.
 *
 * Every download the console starts lands in the public Downloads collection,
 * where the operator's file manager and the system's Downloads app find it,
 * without any storage permission:
 *
 * - [saveDownload] moves a file the injected script fetched in the page (an
 *   `<a download>` target, such as a blob the console built) into the
 *   collection through `MediaStore`. Before Android 10, which has no
 *   Downloads collection, the file goes to the application's own Downloads
 *   directory and is announced to the system's Downloads app.
 * - The download listener hands a download the WebView starts itself, a
 *   navigation the server answers with an attachment such as a run's
 *   archive, to the system's `DownloadManager`, with the WebView's cookies
 *   and user agent so the console's origin admits the request. The system
 *   shows the transfer's progress in a notification.
 *
 * Either way the console's page shows a notice of where the file went.
 */
@TauriPlugin
class ShellPlugin(private val activity: Activity) : Plugin(activity) {
  override fun load(webView: WebView) {
    webView.setDownloadListener { url, userAgent, contentDisposition, mimeType, _ ->
      val name = URLUtil.guessFileName(url, contentDisposition, mimeType)
      try {
        enqueue(url, userAgent, mimeType, name)
        notice(webView, "Downloading $name to Downloads")
      } catch (error: Exception) {
        notice(webView, "Could not download $name: ${error.message}")
      }
    }
  }

  @Command
  fun saveDownload(invoke: Invoke) {
    val args = invoke.parseArgs(SaveDownloadArgs::class.java)
    val mimeType = args.mimeType?.takeIf { it.isNotBlank() } ?: "application/octet-stream"
    // The copy runs off the main thread, so a large download does not stall
    // the WebView.
    Thread {
      val staged = File(args.path)
      try {
        val saved = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
          saveToCollection(staged, args.name, mimeType)
        } else {
          saveToApplicationDownloads(staged, args.name, mimeType)
        }
        val result = JSObject()
        result.put("name", saved)
        result.put("location", "Downloads")
        invoke.resolve(result)
      } catch (error: Exception) {
        invoke.reject(error.message ?: error.toString())
      } finally {
        staged.delete()
      }
    }.start()
  }

  /** Copies `staged` into the Downloads collection and answers the name it took. */
  @RequiresApi(Build.VERSION_CODES.Q)
  private fun saveToCollection(staged: File, name: String, mimeType: String): String {
    val resolver = activity.contentResolver
    val collection = MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
    val pending = ContentValues().apply {
      put(MediaStore.MediaColumns.DISPLAY_NAME, name)
      put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
      put(MediaStore.MediaColumns.IS_PENDING, 1)
    }
    val item = resolver.insert(collection, pending)
      ?: throw IOException("the Downloads collection refused $name")
    try {
      resolver.openOutputStream(item)?.use { output ->
        staged.inputStream().use { input -> input.copyTo(output) }
      } ?: throw IOException("the Downloads collection gave no stream for $name")
      resolver.update(item, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
    } catch (error: Exception) {
      resolver.delete(item, null, null)
      throw error
    }
    // The collection numbers a name that is taken, so the name is read back.
    resolver.query(item, arrayOf(MediaStore.MediaColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
      if (cursor.moveToFirst()) return cursor.getString(0)
    }
    return name
  }

  /**
   * Copies `staged` into the application's own Downloads directory, numbering
   * a name that is taken, and announces it to the system's Downloads app.
   */
  @Suppress("DEPRECATION")
  private fun saveToApplicationDownloads(staged: File, name: String, mimeType: String): String {
    val directory = activity.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS)
      ?: throw IOException("the shared storage is not available")
    directory.mkdirs()
    val dot = name.lastIndexOf('.').takeIf { it > 0 } ?: name.length
    var target = File(directory, name)
    var attempt = 1
    while (!target.createNewFile()) {
      target = File(directory, "${name.substring(0, dot)} ($attempt)${name.substring(dot)}")
      attempt++
    }
    staged.inputStream().use { input -> target.outputStream().use { output -> input.copyTo(output) } }
    val manager = activity.getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
    manager.addCompletedDownload(target.name, target.name, true, mimeType, target.absolutePath, target.length(), true)
    return target.name
  }

  /** Hands the download at `url` to the system's `DownloadManager`. */
  private fun enqueue(url: String, userAgent: String, mimeType: String?, name: String) {
    val request = DownloadManager.Request(Uri.parse(url)).apply {
      CookieManager.getInstance().getCookie(url)?.let { addRequestHeader("Cookie", it) }
      addRequestHeader("User-Agent", userAgent)
      if (!mimeType.isNullOrBlank()) setMimeType(mimeType)
      setTitle(name)
      setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name)
      } else {
        setDestinationInExternalFilesDir(activity, Environment.DIRECTORY_DOWNLOADS, name)
      }
    }
    val manager = activity.getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
    manager.enqueue(request)
  }

  /** Shows `text` as a notice on the console's page. */
  private fun notice(webView: WebView, text: String) {
    val script =
      "window.dispatchEvent(new CustomEvent(\"tcab:shell-notice\", { detail: ${JSONObject.quote(text)} }));"
    webView.post { webView.evaluateJavascript(script, null) }
  }
}
