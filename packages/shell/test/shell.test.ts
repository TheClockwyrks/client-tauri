import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { Window as HappyWindow } from "happy-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type Activation =
  | { kind: "external"; href: string }
  | { kind: "download"; href: string; name: string };

interface Gesture {
  type: string;
  button: number;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

interface Config {
  origin: string;
  signInHosts: string[];
  token: string;
}

interface ShellModule {
  installShell(win: unknown, config: Config): boolean;
  decide(anchor: unknown, gesture: Gesture, config: Config): Activation | null;
  NOTICE_EVENT: string;
  SHELL_RESUME_EVENT: string;
  RESUME_EVENT: string;
  RESUME_COALESCE_MS: number;
  TOKEN_HEADER: string;
}

const require = createRequire(import.meta.url);
const shell = require("../src/shell.cjs") as ShellModule;

const ORIGIN = "https://console.staging.testcabinet.ai";
const CONFIG: Config = {
  origin: ORIGIN,
  signInHosts: ["cloudflareaccess.com", "login.microsoftonline.com"],
  token: "0f1e2d3c4b5a69788796a5b4c3d2e1f0",
};

// The options every call of a command carries: the launch's token, and the
// headers the command takes itself.
function withToken(headers: Record<string, string> = {}) {
  return { headers: { ...headers, "x-tcab-shell-token": CONFIG.token } };
}

interface Call {
  command: string;
  args: unknown;
  options: { headers?: Record<string, string> } | undefined;
}

interface Page {
  win: Window & typeof globalThis;
  calls: Call[];
  answer: (command: string, value: unknown) => void;
}

const pages: HappyWindow[] = [];

// A happy-dom window at `url` with a stand-in for the runtime's IPC that
// records every call and answers each command with what the test set.
function page(url = `${ORIGIN}/runs/abc`): Page {
  const happy = new HappyWindow({ url });
  pages.push(happy);
  const win = happy as unknown as Window & typeof globalThis;
  const calls: Call[] = [];
  const answers = new Map<string, unknown>();
  (
    win as unknown as { __TAURI_INTERNALS__: TauriInternals }
  ).__TAURI_INTERNALS__ = {
    invoke(command, args, options) {
      calls.push({ command, args, options });
      const value = answers.get(command);
      return value instanceof Error
        ? Promise.reject(value)
        : Promise.resolve(value);
    },
  };
  return {
    win,
    calls,
    answer: (command, value) => answers.set(command, value),
  };
}

// Lets every promise the shell chained settle.
async function settle(): Promise<void> {
  for (let i = 0; i < 10; i++) await Promise.resolve();
  await new Promise((resolve) => setImmediate(resolve));
}

function anchor(
  win: Window & typeof globalThis,
  attributes: Record<string, string>,
) {
  const element = win.document.createElement("a");
  for (const [name, value] of Object.entries(attributes))
    element.setAttribute(name, value);
  return element;
}

function click(
  win: Window & typeof globalThis,
  init: MouseEventInit = {},
  type = "click",
) {
  return new win.MouseEvent(type, { bubbles: true, cancelable: true, ...init });
}

afterEach(async () => {
  for (const happy of pages.splice(0)) await happy.happyDOM.close();
});

describe("installShell", () => {
  it("installs on the console's origin alone", () => {
    expect(shell.installShell(page().win, CONFIG)).toBe(true);
    const login = page(
      "https://shrill-term-5c16.cloudflareaccess.com/cdn-cgi/access/login",
    );
    const open = login.win.open;
    expect(shell.installShell(login.win, CONFIG)).toBe(false);
    expect(login.win.open).toBe(open);
    const microsoft = page(
      "https://login.microsoftonline.com/common/oauth2/authorize",
    );
    expect(shell.installShell(microsoft.win, CONFIG)).toBe(false);
  });

  it("keeps the token to itself and sends it with every call", async () => {
    const current = page();
    shell.installShell(current.win, CONFIG);
    const globals = Object.getOwnPropertyNames(current.win).filter((name) => {
      try {
        return JSON.stringify(
          (current.win as unknown as Record<string, unknown>)[name],
        )?.includes(CONFIG.token);
      } catch {
        return false;
      }
    });
    expect(globals).toEqual([]);
    current.win.open("https://example.com/");
    await current.win.navigator.clipboard.writeText("x");
    await settle();
    expect(current.calls.map((call) => call.options)).toEqual([
      withToken(),
      withToken(),
    ]);
    expect(shell.TOKEN_HEADER).toBe("x-tcab-shell-token");
  });

  it("installs once however often it runs", () => {
    const { win } = page();
    shell.installShell(win, CONFIG);
    const open = win.open;
    expect(shell.installShell(win, CONFIG)).toBe(true);
    expect(win.open).toBe(open);
  });
});

describe("window.open", () => {
  let current: Page;
  beforeEach(() => {
    current = page();
    shell.installShell(current.win, CONFIG);
  });

  it("opens a new window's URL in the platform's browser", async () => {
    const opened = current.win.open(
      `${ORIGIN}/runs/xyz`,
      "_blank",
      "noopener,noreferrer",
    );
    expect(opened).toBeNull();
    await settle();
    expect(current.calls).toEqual([
      {
        command: "open_external",
        args: { url: `${ORIGIN}/runs/xyz` },
        options: withToken(),
      },
    ]);
  });

  it("resolves a relative URL against the page and treats no target as a new window", async () => {
    current.win.open("/models/m1");
    await settle();
    expect(current.calls.map((call) => call.args)).toEqual([
      { url: `${ORIGIN}/models/m1` },
    ]);
  });

  it("opens only web URLs", async () => {
    current.win.open("javascript:alert(1)", "_blank");
    current.win.open("about:blank");
    current.win.open("mailto:someone@example.com");
    await settle();
    expect(current.calls).toEqual([]);
  });

  it("does nothing for a call naming no page, and hands back no window", () => {
    const native = vi.fn();
    const happy = page();
    Object.assign(happy.win, { open: native });
    shell.installShell(happy.win, CONFIG);
    for (const url of [undefined, "", "  ", "about:blank", "ABOUT:BLANK"]) {
      expect(happy.win.open(url)).toBeNull();
      expect(happy.win.open(url, "_self")).toBeNull();
      expect(happy.win.open(url, "_blank")).toBeNull();
    }
    expect(native).not.toHaveBeenCalled();
    expect(happy.calls).toEqual([]);
  });

  it("leaves a URL named for the current context to the WebView", () => {
    // happy-dom navigates the window itself for `_self`, which is the native
    // behavior the shell defers to.
    expect(() => current.win.open(`${ORIGIN}/runs`, "_self")).not.toThrow();
    expect(current.calls).toEqual([]);
  });

  it("tells the operator when the browser could not be opened", async () => {
    current.answer("open_external", new Error("no browser"));
    current.win.open("https://example.com/");
    await settle();
    const notice = current.win.document.querySelector(
      "[data-tcab-shell-notice]",
    );
    expect(notice?.textContent).toBe(
      "Could not open https://example.com/: no browser",
    );
  });
});

describe("decide", () => {
  const plain: Gesture = {
    type: "click",
    button: 0,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
  };
  const { win } = page();
  const link = (attributes: Record<string, string>) => anchor(win, attributes);

  it("follows a plain link to the console or its sign-in flow in place", () => {
    expect(
      shell.decide(link({ href: `${ORIGIN}/runs` }), plain, CONFIG),
    ).toBeNull();
    expect(
      shell.decide(
        link({
          href: "https://team.cloudflareaccess.com/cdn-cgi/access/logout",
        }),
        plain,
        CONFIG,
      ),
    ).toBeNull();
    expect(
      shell.decide(
        link({ href: "https://login.microsoftonline.com/common/" }),
        plain,
        CONFIG,
      ),
    ).toBeNull();
    expect(
      shell.decide(link({ href: "mailto:a@example.com" }), plain, CONFIG),
    ).toBeNull();
    expect(
      shell.decide(
        link({ href: `${ORIGIN}/runs`, target: "_self" }),
        plain,
        CONFIG,
      ),
    ).toBeNull();
  });

  it("opens a plain link to any other site in the browser", () => {
    for (const href of [
      "https://example.com/",
      "https://console.testcabinet.ai/",
      "https://evilcloudflareaccess.com/",
      "https://login.microsoftonline.com.example.com/",
    ]) {
      expect(shell.decide(link({ href }), plain, CONFIG)).toEqual({
        kind: "external",
        href,
      });
    }
  });

  it("opens a link to a new context in the browser", () => {
    const href = "https://run.pages.dev/";
    const external = { kind: "external", href };
    expect(
      shell.decide(link({ href, target: "_blank" }), plain, CONFIG),
    ).toEqual(external);
    expect(
      shell.decide(link({ href, target: "preview" }), plain, CONFIG),
    ).toEqual(external);
    expect(
      shell.decide(link({ href }), { ...plain, ctrlKey: true }, CONFIG),
    ).toEqual(external);
    expect(
      shell.decide(link({ href }), { ...plain, metaKey: true }, CONFIG),
    ).toEqual(external);
    expect(
      shell.decide(link({ href }), { ...plain, shiftKey: true }, CONFIG),
    ).toEqual(external);
    expect(
      shell.decide(
        link({ href }),
        { ...plain, type: "auxclick", button: 1 },
        CONFIG,
      ),
    ).toEqual(external);
  });

  it("ignores every button but the primary and the middle", () => {
    const href = "https://run.pages.dev/";
    expect(
      shell.decide(
        link({ href, target: "_blank" }),
        { ...plain, button: 2 },
        CONFIG,
      ),
    ).toBeNull();
    expect(
      shell.decide(
        link({ href }),
        { ...plain, type: "auxclick", button: 2 },
        CONFIG,
      ),
    ).toBeNull();
  });

  it("opens only web URLs in a new context", () => {
    expect(
      shell.decide(
        link({ href: "mailto:a@example.com", target: "_blank" }),
        plain,
        CONFIG,
      ),
    ).toBeNull();
  });

  it("downloads what the page can read", () => {
    expect(
      shell.decide(
        link({ href: "blob:https://x/1", download: "clip.webm" }),
        plain,
        CONFIG,
      ),
    ).toEqual({
      kind: "download",
      href: "blob:https://x/1",
      name: "clip.webm",
    });
    expect(
      shell.decide(
        link({ href: "data:text/plain,hi", download: "" }),
        plain,
        CONFIG,
      ),
    ).toEqual({
      kind: "download",
      href: "data:text/plain,hi",
      name: "download",
    });
    expect(
      shell.decide(
        link({
          href: `${ORIGIN}/artifacts/runs/r%201/frame%2001.png`,
          download: "",
        }),
        { ...plain, ctrlKey: true },
        CONFIG,
      ),
    ).toEqual({
      kind: "download",
      href: `${ORIGIN}/artifacts/runs/r%201/frame%2001.png`,
      name: "frame 01.png",
    });
  });

  it("treats a download of another origin as the link it is", () => {
    const href = "https://other.example.com/file.zip";
    expect(
      shell.decide(link({ href, download: "file.zip" }), plain, CONFIG),
    ).toEqual({ kind: "external", href });
    expect(
      shell.decide(
        link({ href, download: "file.zip", target: "_blank" }),
        plain,
        CONFIG,
      ),
    ).toEqual({ kind: "external", href });
  });

  it("ignores an anchor without a usable href", () => {
    expect(shell.decide(link({}), plain, CONFIG)).toBeNull();
  });
});

describe("clicks", () => {
  let current: Page;
  beforeEach(() => {
    current = page();
    shell.installShell(current.win, CONFIG);
  });

  it("opens a target=_blank link in the browser and cancels the WebView's", async () => {
    const link = anchor(current.win, {
      href: "https://run.pages.dev/",
      target: "_blank",
    });
    current.win.document.body.append(link);
    const event = click(current.win);
    link.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    await settle();
    expect(current.calls.map((call) => call.command)).toEqual([
      "open_external",
    ]);
  });

  it("opens a middle-clicked link in the browser", async () => {
    const link = anchor(current.win, { href: `${ORIGIN}/runs/1` });
    current.win.document.body.append(link);
    const event = click(current.win, { button: 1 }, "auxclick");
    link.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    await settle();
    expect(current.calls.map((call) => call.args)).toEqual([
      { url: `${ORIGIN}/runs/1` },
    ]);
  });

  it("finds the anchor around the element clicked", async () => {
    const link = anchor(current.win, { href: "https://run.pages.dev/" });
    const icon = current.win.document.createElement("span");
    link.append(icon);
    current.win.document.body.append(link);
    icon.dispatchEvent(click(current.win, { ctrlKey: true }));
    await settle();
    expect(current.calls.map((call) => call.args)).toEqual([
      { url: "https://run.pages.dev/" },
    ]);
  });

  it("leaves a click the page already handled alone", async () => {
    const link = anchor(current.win, { href: `${ORIGIN}/runs/1` });
    link.addEventListener("click", (event) => event.preventDefault());
    current.win.document.body.append(link);
    link.dispatchEvent(click(current.win, { ctrlKey: true }));
    await settle();
    expect(current.calls).toEqual([]);
  });

  it("catches the modifier click a page synthesizes on a detached anchor", async () => {
    // The background-tab gesture of the console's run menu.
    const link = current.win.document.createElement("a");
    link.href = `${ORIGIN}/runs/2`;
    link.target = "_blank";
    const dispatched = link.dispatchEvent(
      new current.win.MouseEvent("click", {
        ctrlKey: true,
        metaKey: true,
        bubbles: false,
        cancelable: true,
      }),
    );
    expect(dispatched).toBe(false);
    await settle();
    expect(current.calls.map((call) => call.args)).toEqual([
      { url: `${ORIGIN}/runs/2` },
    ]);
  });

  it("catches click() on a detached anchor", async () => {
    const link = current.win.document.createElement("a");
    link.href = "https://run.pages.dev/";
    link.target = "_blank";
    link.click();
    await settle();
    expect(current.calls).toHaveLength(1);
  });

  it("leaves a detached anchor with nothing to do to the WebView", async () => {
    const link = current.win.document.createElement("a");
    link.href = `${ORIGIN}/runs/3`;
    let heard = 0;
    link.addEventListener("click", (event) => {
      heard++;
      event.preventDefault();
    });
    link.click();
    link.dispatchEvent(click(current.win));
    link.dispatchEvent(new current.win.Event("focus"));
    await settle();
    expect(heard).toBe(2);
    expect(current.calls).toEqual([]);
  });
});

describe("downloads", () => {
  let current: Page;
  beforeEach(() => {
    current = page();
    shell.installShell(current.win, CONFIG);
  });

  function serve(response: Response) {
    const fetch = vi.fn((_input: string, _init?: RequestInit) =>
      Promise.resolve(response),
    );
    (current.win as unknown as { fetch: typeof fetch }).fetch = fetch;
    return fetch;
  }

  it("fetches an <a download> target with credentials and saves its bytes", async () => {
    const fetch = serve(
      new Response(new Uint8Array([1, 2, 3]), {
        headers: { "content-type": "image/gif" },
      }),
    );
    current.answer("save_download", {
      name: "run 1.gif",
      location: "/home/op/Downloads",
    });
    const link = anchor(current.win, {
      href: "blob:https://x/42",
      download: "run 1.gif",
    });
    current.win.document.body.append(link);
    // The console's helper clicks an anchor it appended, then revokes the URL.
    link.click();
    expect(fetch).toHaveBeenCalledWith("blob:https://x/42", {
      credentials: "include",
    });
    await settle();
    expect(current.calls).toHaveLength(1);
    const [call] = current.calls;
    expect(call?.command).toBe("save_download");
    expect(call?.args).toBeInstanceOf(Uint8Array);
    expect(Array.from(call?.args as Uint8Array)).toEqual([1, 2, 3]);
    expect(call?.options).toEqual(
      withToken({
        "x-tcab-file-name": "run%201.gif",
        "x-tcab-mime-type": "image/gif",
      }),
    );
    const notice = current.win.document.querySelector(
      "[data-tcab-shell-notice]",
    );
    expect(notice?.textContent).toBe("Saved run 1.gif to /home/op/Downloads");
    expect(notice?.getAttribute("role")).toBe("status");
  });

  it("starts the fetch before the click returns", () => {
    const fetch = serve(new Response("x"));
    const link = anchor(current.win, {
      href: "blob:https://x/1",
      download: "a.txt",
    });
    current.win.document.body.append(link);
    link.dispatchEvent(click(current.win));
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("tells the operator when the download failed", async () => {
    serve(new Response("gone", { status: 404 }));
    const link = anchor(current.win, {
      href: `${ORIGIN}/a.txt`,
      download: "a.txt",
    });
    current.win.document.body.append(link);
    link.click();
    await settle();
    expect(current.calls).toEqual([]);
    const notice = current.win.document.querySelector(
      "[data-tcab-shell-notice]",
    );
    expect(notice?.textContent).toBe(
      "Could not download a.txt: the server answered 404",
    );
  });

  it("tells the operator when the shell could not save", async () => {
    serve(new Response("x"));
    current.answer("save_download", new Error("disk full"));
    const link = anchor(current.win, {
      href: "data:text/plain,x",
      download: "x.txt",
    });
    current.win.document.body.append(link);
    link.click();
    await settle();
    const notice = current.win.document.querySelector(
      "[data-tcab-shell-notice]",
    );
    expect(notice?.textContent).toBe("Could not download x.txt: disk full");
  });
});

describe("clipboard", () => {
  it("writes text through the shell", async () => {
    const current = page();
    shell.installShell(current.win, CONFIG);
    await expect(
      current.win.navigator.clipboard.writeText("https://tcab.ai/r/1"),
    ).resolves.toBeUndefined();
    expect(current.calls).toEqual([
      {
        command: "copy_text",
        args: { text: "https://tcab.ai/r/1" },
        options: withToken(),
      },
    ]);
  });

  it("supplies a clipboard where the WebView offers none", async () => {
    const current = page();
    Object.defineProperty(current.win.navigator, "clipboard", {
      configurable: true,
      value: undefined,
    });
    shell.installShell(current.win, CONFIG);
    await current.win.navigator.clipboard.writeText("x");
    expect(current.calls.map((call) => call.command)).toEqual(["copy_text"]);
  });

  it("rejects when the shell could not copy", async () => {
    const current = page();
    current.answer("copy_text", new Error("no clipboard"));
    shell.installShell(current.win, CONFIG);
    await expect(
      current.win.navigator.clipboard.writeText("x"),
    ).rejects.toThrow("no clipboard");
  });

  it("rejects when the IPC is missing", async () => {
    const current = page();
    delete (current.win as unknown as { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__;
    shell.installShell(current.win, CONFIG);
    await expect(
      current.win.navigator.clipboard.writeText("x"),
    ).rejects.toThrow("the shell's IPC is not available");
  });
});

describe("notices", () => {
  it("shows a notice the shell raises, then removes it", () => {
    const current = page();
    // The page's own timers, held so the test runs the one the notice set.
    const timers = new Map<number, () => void>();
    let next = 0;
    Object.assign(current.win, {
      setTimeout: (callback: () => void, ms: number) => {
        expect(ms).toBe(5000);
        timers.set(++next, callback);
        return next;
      },
      clearTimeout: (id: number) => timers.delete(id),
    });
    shell.installShell(current.win, CONFIG);
    const raise = (text: unknown) =>
      current.win.dispatchEvent(
        new current.win.CustomEvent(shell.NOTICE_EVENT, { detail: text }),
      );
    raise("Downloading run.tar.gz to Downloads");
    const notices = () =>
      current.win.document.querySelectorAll("[data-tcab-shell-notice]");
    expect(notices()).toHaveLength(1);
    expect(notices()[0]?.textContent).toBe(
      "Downloading run.tar.gz to Downloads",
    );
    raise("Saved run.tar.gz");
    expect(notices()).toHaveLength(1);
    expect(notices()[0]?.textContent).toBe("Saved run.tar.gz");
    raise(null);
    expect(notices()[0]?.textContent).toBe("Saved run.tar.gz");
    expect(timers.size).toBe(1);
    for (const callback of timers.values()) callback();
    expect(notices()).toHaveLength(0);
  });
});

describe("resume", () => {
  let current: Page;
  let resumes: number;
  let visibility: DocumentVisibilityState;
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(1_000_000);
    current = page();
    visibility = "visible";
    Object.defineProperty(current.win.document, "visibilityState", {
      configurable: true,
      get: () => visibility,
    });
    shell.installShell(current.win, CONFIG);
    resumes = 0;
    current.win.addEventListener(shell.RESUME_EVENT, () => resumes++);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  function turn(state: DocumentVisibilityState) {
    visibility = state;
    current.win.document.dispatchEvent(
      new current.win.Event("visibilitychange"),
    );
  }
  function shellResume() {
    current.win.dispatchEvent(new current.win.Event(shell.SHELL_RESUME_EVENT));
  }

  it("names the console's event", () => {
    expect(shell.RESUME_EVENT).toBe("tcab:resume");
    expect(shell.SHELL_RESUME_EVENT).toBe("tcab:shell-resume");
  });

  it("dispatches on the page turning visible after it was hidden", () => {
    turn("visible");
    expect(resumes).toBe(0);
    turn("hidden");
    expect(resumes).toBe(0);
    turn("visible");
    expect(resumes).toBe(1);
  });

  it("dispatches on the shell's event, which a wake from sleep raises alone", () => {
    shellResume();
    expect(resumes).toBe(1);
  });

  it("dispatches once for the two signals of one return, in either order", () => {
    turn("hidden");
    turn("visible");
    shellResume();
    expect(resumes).toBe(1);
    vi.setSystemTime(Date.now() + shell.RESUME_COALESCE_MS);
    shellResume();
    turn("hidden");
    turn("visible");
    expect(resumes).toBe(2);
  });

  it("dispatches again for a later return", () => {
    shellResume();
    vi.setSystemTime(Date.now() + shell.RESUME_COALESCE_MS - 1);
    shellResume();
    expect(resumes).toBe(1);
    vi.setSystemTime(Date.now() + 1);
    turn("hidden");
    turn("visible");
    expect(resumes).toBe(2);
  });
});

describe("the embedded form", () => {
  it("runs as the classic script the Rust crate wraps it in", () => {
    // The crate's wrapper, as `script.rs` writes it.
    const source = readFileSync(
      new URL("../src/shell.cjs", import.meta.url),
      "utf8",
    );
    const current = page();
    const run = new Function(
      "window",
      `(function (module) {\n${source}\ninstallShell(window, ${JSON.stringify(CONFIG)});\n})();`,
    );
    const open = current.win.open;
    run(current.win);
    expect(current.win.open).not.toBe(open);
  });
});
