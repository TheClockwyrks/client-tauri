// @ts-check
//
// The script the shell injects into the console's pages: the affordances a
// WebView does not supply, each carried over the shell's IPC to the Rust side.
//
// The Rust crate embeds this file with `include_str!` and evaluates it as a
// classic script before any of the page's own, wrapped as
//
//     (function (module) { <this file> installShell(window, <config>); })();
//
// so it declares `installShell` and nothing else: the wrapper's `module`
// parameter shadows any global of that name, and the export at the bottom is
// what the unit tests import. It runs on every page the WebView loads, the
// Cloudflare Access and Microsoft sign-in pages among them, and installs itself
// on the console's origin alone.

/**
 * What the Rust side tells the script.
 *
 * @typedef {object} ShellConfig
 * @property {string} origin The console's origin, such as
 *   `https://console.testcabinet.ai`.
 * @property {string[]} signInHosts The hosts of the sign-in flow, each
 *   standing for itself and its subdomains, which a link may lead to inside
 *   the WebView.
 */

/**
 * What a click on an anchor asks the shell for: the URL opened in the
 * platform's browser, or the target written through the platform's download
 * path under a file name.
 *
 * @typedef {{ kind: "external", href: string }
 *   | { kind: "download", href: string, name: string }} Activation
 */

/**
 * The part of a mouse event that decides what activating an anchor means.
 *
 * @typedef {object} Gesture
 * @property {string} type `click` or `auxclick`.
 * @property {number} button
 * @property {boolean} ctrlKey
 * @property {boolean} metaKey
 * @property {boolean} shiftKey
 */

/**
 * What the Rust side answers a saved download with.
 *
 * @typedef {object} SavedDownload
 * @property {string} name The file name the download was saved under.
 * @property {string} location Where it went, as the operator would look for it.
 */

/** The `window` event a native part of the shell raises to show a notice. */
const NOTICE_EVENT = "tcab:shell-notice";

/** How long a notice stays on the page, in milliseconds. */
const NOTICE_MS = 5000;

/** The window targets that keep a link in the current browsing context. */
const SAME_CONTEXT_TARGETS = ["", "_self", "_parent", "_top"];

/**
 * Installs the shell's affordances into the page `win` holds, when that page
 * is on the console's origin.
 *
 * - `window.open`, an anchor with a `target` of a new browsing context, and a
 *   modifier or middle click on an anchor open the URL in the platform's
 *   browser, and so does a plain link to a site that is neither the console
 *   nor its sign-in flow, so the WebView stays on the console.
 * - Activating an anchor with a `download` attribute whose target the page can
 *   read (a `blob:` or `data:` URL, or one on the console's origin) fetches
 *   the target in the page with its credentials and hands the bytes to the
 *   shell, which writes them through the platform's download path.
 * - `navigator.clipboard.writeText` writes through the shell's clipboard.
 * - A notice the shell raises with the `tcab:shell-notice` event is shown on
 *   the page.
 *
 * @param {Window & typeof globalThis} win
 * @param {ShellConfig} config
 * @returns {boolean} Whether the affordances were installed.
 */
function installShell(win, config) {
  if (win.location.origin !== config.origin) return false;
  if (installed.has(win)) return true;
  installed.add(win);

  const { document } = win;

  /**
   * Calls a command of the shell. The IPC is resolved at the call, since the
   * runtime's own initialization scripts may run after this one.
   *
   * @param {string} command
   * @param {unknown} [args]
   * @param {{ headers?: Record<string, string> }} [options]
   * @returns {Promise<unknown>}
   */
  function invoke(command, args, options) {
    const internals = win.__TAURI_INTERNALS__;
    if (!internals) {
      return Promise.reject(new Error("the shell's IPC is not available"));
    }
    return internals.invoke(command, args, options);
  }

  /** @type {HTMLElement | null} */
  let noticeElement = null;
  /** @type {number | undefined} */
  let noticeTimer;

  /**
   * Shows `text` in a brief notice at the bottom of the page, replacing the
   * one already shown.
   *
   * @param {string} text
   */
  function notice(text) {
    if (!noticeElement) {
      noticeElement = document.createElement("div");
      noticeElement.setAttribute("role", "status");
      noticeElement.setAttribute("aria-live", "polite");
      noticeElement.dataset.tcabShellNotice = "";
      Object.assign(noticeElement.style, {
        position: "fixed",
        left: "50%",
        bottom: "1rem",
        transform: "translateX(-50%)",
        maxWidth: "min(90vw, 40rem)",
        padding: "0.5rem 0.9rem",
        borderRadius: "0.4rem",
        background: "rgba(18, 10, 28, 0.94)",
        color: "#fdf3ff",
        border: "1px solid #6a3aa0",
        font: "13px ui-monospace, 'DejaVu Sans Mono', Menlo, Consolas, monospace",
        overflowWrap: "anywhere",
        zIndex: "2147483647",
        pointerEvents: "none",
      });
    }
    noticeElement.textContent = text;
    (document.body ?? document.documentElement).append(noticeElement);
    if (noticeTimer !== undefined) win.clearTimeout(noticeTimer);
    noticeTimer = win.setTimeout(() => {
      noticeElement?.remove();
      noticeTimer = undefined;
    }, NOTICE_MS);
  }

  /** @param {string} href */
  function openExternal(href) {
    invoke("open_external", { url: href }).catch(
      (/** @type {unknown} */ error) => {
        notice(`Could not open ${href}: ${describe(error)}`);
      },
    );
  }

  /**
   * Fetches `href` and has the shell save it as `name`. The fetch starts
   * before this function returns, since a page revokes a `blob:` URL as soon
   * as the click that downloads it has been dispatched.
   *
   * @param {string} href
   * @param {string} name
   * @returns {Promise<void>}
   */
  function download(href, name) {
    const fetched = win.fetch(href, { credentials: "include" });
    return fetched
      .then(async (response) => {
        if (!response.ok)
          throw new Error(`the server answered ${response.status}`);
        const blob = await response.blob();
        const bytes = new Uint8Array(await blob.arrayBuffer());
        const type = blob.type || response.headers.get("content-type") || "";
        const saved = /** @type {SavedDownload} */ (
          await invoke("save_download", bytes, {
            headers: {
              "x-tcab-file-name": encodeURIComponent(name),
              "x-tcab-mime-type": type,
            },
          })
        );
        notice(`Saved ${saved.name} to ${saved.location}`);
      })
      .catch((/** @type {unknown} */ error) => {
        notice(`Could not download ${name}: ${describe(error)}`);
      });
  }

  /** @param {Activation} activation */
  function perform(activation) {
    if (activation.kind === "external") openExternal(activation.href);
    else void download(activation.href, activation.name);
  }

  // window.open opens every URL in a new browsing context but the ones named
  // for this one, so a web URL goes to the platform's browser. The page is
  // handed no window back, which a caller that focuses the result tolerates.
  const nativeOpen = win.open;
  win.open = function open(url, target, features) {
    const name = target === undefined ? "_blank" : String(target).toLowerCase();
    if (SAME_CONTEXT_TARGETS.includes(name) && name !== "") {
      return nativeOpen.call(win, url, target, features);
    }
    const href = resolve(
      url === undefined ? "" : String(url),
      win.location.href,
    );
    if (href && isWeb(href)) openExternal(href);
    return null;
  };

  // A click is judged once the page's own handlers have run, from the window
  // in the bubbling phase, so a click a router already handled is left alone.
  /** @param {MouseEvent} event */
  function onActivate(event) {
    if (event.defaultPrevented) return;
    const anchor = anchorOf(event, win);
    if (!anchor) return;
    const activation = decide(anchor, event, config);
    if (!activation) return;
    event.preventDefault();
    perform(activation);
  }
  win.addEventListener("click", onActivate);
  win.addEventListener("auxclick", onActivate);

  // An anchor activated outside the document, such as the throwaway anchor a
  // page synthesizes a modifier click on to open a background tab, never
  // reaches the window, so its activation is caught where it is dispatched.
  // Such an anchor carries no listener of the page's, so the shell acts in
  // place of the dispatch.
  const anchorPrototype = win.HTMLAnchorElement.prototype;
  const nativeClick = anchorPrototype.click;
  const nativeDispatch = anchorPrototype.dispatchEvent;
  Object.defineProperty(anchorPrototype, "click", {
    configurable: true,
    writable: true,
    value: /** @this {HTMLAnchorElement} */ function click() {
      if (!this.isConnected) {
        const activation = decide(this, PLAIN_CLICK, config);
        if (activation) {
          perform(activation);
          return;
        }
      }
      nativeClick.call(this);
    },
  });
  Object.defineProperty(anchorPrototype, "dispatchEvent", {
    configurable: true,
    writable: true,
    value: /** @this {HTMLAnchorElement} */ function dispatchEvent(
      /** @type {Event} */ event,
    ) {
      if (!this.isConnected && isActivation(event, win)) {
        const activation = decide(this, event, config);
        if (activation) {
          perform(activation);
          return false;
        }
      }
      return nativeDispatch.call(this, event);
    },
  });

  // Copying goes through the shell, so it works whatever the WebView grants
  // the page's own clipboard.
  /** @param {string} text */
  const writeText = (text) =>
    invoke("copy_text", { text: String(text) }).then(() => undefined);
  const clipboard = win.navigator.clipboard;
  if (clipboard) {
    Object.defineProperty(clipboard, "writeText", {
      configurable: true,
      writable: true,
      value: writeText,
    });
  } else {
    Object.defineProperty(win.navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
  }

  win.addEventListener(NOTICE_EVENT, (event) => {
    const detail = /** @type {CustomEvent} */ (event).detail;
    if (detail !== undefined && detail !== null) notice(String(detail));
  });

  return true;
}

/** The windows the shell is installed into, so a second call changes nothing. */
const installed = new WeakSet();

/** @type {Gesture} */
const PLAIN_CLICK = {
  type: "click",
  button: 0,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
};

/**
 * What activating `anchor` with `gesture` asks of the shell, or null for what
 * the WebView does itself: following a link in place to the console or its
 * sign-in flow, or to a URL that is not a web URL.
 *
 * @param {HTMLAnchorElement} anchor
 * @param {Gesture} gesture
 * @param {ShellConfig} config
 * @returns {Activation | null}
 */
function decide(anchor, gesture, config) {
  const middle = gesture.type === "auxclick" && gesture.button === 1;
  const primary = gesture.type === "click" && gesture.button === 0;
  if (!middle && !primary) return null;

  const href = anchor.href;
  if (!href) return null;
  let url;
  try {
    url = new URL(href);
  } catch {
    return null;
  }

  if (anchor.hasAttribute("download") && readable(url, config.origin)) {
    return {
      kind: "download",
      href: url.href,
      name: anchor.getAttribute("download") || nameOf(url),
    };
  }

  const target = (anchor.getAttribute("target") ?? "").toLowerCase();
  const newContext =
    middle ||
    gesture.ctrlKey ||
    gesture.metaKey ||
    gesture.shiftKey ||
    !SAME_CONTEXT_TARGETS.includes(target);
  if (!isWeb(url.href)) return null;
  if (newContext) return { kind: "external", href: url.href };
  if (
    url.origin === config.origin ||
    isSignIn(url.hostname, config.signInHosts)
  )
    return null;
  return { kind: "external", href: url.href };
}

/**
 * Whether `host` is one of the sign-in flow's, or a subdomain of one.
 *
 * @param {string} host
 * @param {string[]} signInHosts
 */
function isSignIn(host, signInHosts) {
  return signInHosts.some(
    (entry) => host === entry || host.endsWith(`.${entry}`),
  );
}

/**
 * Whether the page can read `url` itself: a `blob:` or `data:` URL, or one on
 * the console's origin. A browser ignores the `download` attribute of any
 * other.
 *
 * @param {URL} url
 * @param {string} origin
 */
function readable(url, origin) {
  return (
    url.protocol === "blob:" ||
    url.protocol === "data:" ||
    url.origin === origin
  );
}

/**
 * The file name a download of `url` takes when its anchor names none: the
 * last segment of the path, or `download`.
 *
 * @param {URL} url
 */
function nameOf(url) {
  if (url.protocol === "http:" || url.protocol === "https:") {
    const segment = url.pathname.split("/").pop() ?? "";
    try {
      const decoded = decodeURIComponent(segment);
      if (decoded) return decoded;
    } catch {
      if (segment) return segment;
    }
  }
  return "download";
}

/**
 * `url` resolved against `base`, or null when it is not a URL.
 *
 * @param {string} url
 * @param {string} base
 */
function resolve(url, base) {
  try {
    return new URL(url, base).href;
  } catch {
    return null;
  }
}

/**
 * Whether `href` is an `http:` or `https:` URL, the only kind the shell opens
 * in the platform's browser.
 *
 * @param {string} href
 */
function isWeb(href) {
  return href.startsWith("https://") || href.startsWith("http://");
}

/**
 * The anchor with an `href` an event was dispatched through, if any.
 *
 * @param {Event} event
 * @param {Window & typeof globalThis} win
 * @returns {HTMLAnchorElement | null}
 */
function anchorOf(event, win) {
  for (const target of event.composedPath()) {
    if (
      target instanceof win.HTMLAnchorElement &&
      target.hasAttribute("href")
    ) {
      return target;
    }
  }
  return null;
}

/**
 * Whether `event` activates an anchor: a click, or the auxiliary click of a
 * middle button.
 *
 * @param {Event} event
 * @param {Window & typeof globalThis} win
 * @returns {event is MouseEvent}
 */
function isActivation(event, win) {
  return (
    event instanceof win.MouseEvent &&
    (event.type === "click" || event.type === "auxclick")
  );
}

/**
 * A short description of what went wrong, for a notice.
 *
 * @param {unknown} error
 */
function describe(error) {
  if (error instanceof Error) return error.message;
  return String(error);
}

if (typeof module !== "undefined" && module) {
  module.exports = { installShell, decide, NOTICE_EVENT };
}
