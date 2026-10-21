// What the shell's script reads from the page beyond the DOM: the IPC the
// Tauri runtime injects into every page the WebView loads.

interface TauriInternals {
  invoke(
    command: string,
    args?: unknown,
    options?: { headers?: Record<string, string> },
  ): Promise<unknown>;
}

interface Window {
  __TAURI_INTERNALS__?: TauriInternals;
}
