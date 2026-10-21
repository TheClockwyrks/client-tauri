# client-tauri

The operator client for Windows, Linux, macOS and Android.

This repository is one component of
[The Test Cabinet](https://github.com/TheClockwyrks/TheTestCabinet). What it is
and how it fits with the others is stated by the documentation site, which
lives in the superrepo at `apps/docs/src/content/docs/`. Its Repositories page,
`apps/docs/src/content/docs/development/repositories.md`, draws the edges
between the repositories, and the operator client's page,
`apps/docs/src/content/docs/components/client/overview.md`, states what the
client does.

The client is a Tauri v2 shell around the console: it opens the console's URL
in the platform's WebView and carries the affordances a WebView does not
supply. It bundles no page of its own.

## Layout

| Path | What it is |
| --- | --- |
| `crates/client-tauri/` | The shell crate: a library the Android build loads and the desktop binary links, beside a thin `main.rs` |
| `crates/client-tauri/tauri.conf.json` | The configuration both flavors share |
| `crates/client-tauri/tauri.<flavor>.conf.json` | A flavor's overlay, which sets the console's URL and nothing else |
| `crates/client-tauri/app-icon/` | The icon's source, from which `tauri icon app-icon/icon.json` draws `icons/` and the Android launcher icons |
| `crates/client-tauri/gen/android/` | The Gradle project of the Android build, with the activity and the Kotlin side of the shell |
| `packages/shell/` | The script the shell injects into the console's pages, with its tests; the crate embeds it as written |

`tauri icon` writes the Android launcher icons' XML files
(`mipmap-anydpi-v26/ic_launcher*.xml` and `values/ic_launcher_background.xml`)
without a final newline, which the `end-of-file-fixer` hook refuses, so after
drawing the icons again run `pre-commit run end-of-file-fixer --all-files`
before committing. The Gradle wrapper script `gen/android/gradlew` is
Gradle's, and its second line turns `shellcheck` off for it; a wrapper Gradle
writes again needs that line put back.

## The two flavors

The client is built once per flavor, and a flavor differs in the one URL the
shell loads:

| Flavor | Overlay | Console |
| --- | --- | --- |
| `prod` | `tauri.prod.conf.json` | `https://console.testcabinet.ai` |
| `staging` | `tauri.staging.conf.json` | `https://console.staging.testcabinet.ai` |

The Tauri CLI merges the overlay given with `--config` over `tauri.conf.json`.
The URL sits at `plugins.console.url`, a single key an overlay sets, and the
shell creates its window from that key at startup. The base configuration
names no console, so a build without an overlay starts, says so, and exits.

## What the shell does

- It loads the flavor's console. Sign-in happens inside the WebView: the
  console's origin is behind Cloudflare Access, so the WebView follows the
  Access and Microsoft login pages and keeps the cookie Access sets in its
  persistent profile. On Android the activity flushes the cookie store when it
  pauses.
- It injects `packages/shell/src/shell.cjs` into every page and the script
  installs itself on the console's origin alone. The console's origin, and no
  other, is granted the shell's three commands, `open_external`, `copy_text`
  and `save_download`, through a capability the shell adds at startup. Each
  launch also draws a random token that the script keeps in its closure once
  its origin check passes and sends with every call, and every command refuses
  a call without it. Android's WebView injects the script and the IPC into
  every frame and reports the main frame's URL for a call from any of them, so
  without the token a cross-origin frame such as a run's playable build could
  call the commands.
- `window.open`, a link with a new-window target, a modifier or middle click
  on a link, and a link to a site other than the console and its sign-in flow
  open in the platform's browser. A `window.open` naming no page (no URL, an
  empty one or `about:blank`) does nothing and hands back no window.
- On Windows and Android the main frame is also kept on the console and its
  sign-in flow: any other web URL it would navigate to opens in the browser
  instead. Frames are not held to this, so a run's playable build loads in its
  frame. The guard needs a navigation handler that sees the main frame alone,
  which Windows' does and Android's does once the Kotlin side narrows the
  WebView client to the main frame; on Linux and macOS the handler sees every
  frame's navigation requests with nothing to tell them apart, so there is no guard
  there beyond the script's. On Android a window a frame opens, which would
  otherwise replace the console, goes to the browser too. A WebView too old to
  hand its clients back (before the `GET_WEB_VIEW_CLIENT` feature) keeps the
  runtime's clients unwrapped.
- An `<a download>` the console activates is fetched in the page and written
  through the platform's download path, numbered rather than overwriting a
  file. A download the WebView starts itself, such as a run's archive, goes to
  the same place: the user's download directory on the desktop, and on
  Android the public Downloads folder, through `MediaStore` and
  `DownloadManager` from Android 10 on, and on Android 7 to 9 directly into
  the public Downloads directory with the storage permission, which the shell
  asks for at the first download (refused, the download goes nowhere and the
  notice says why). The page shows a brief notice of where the file went. On
  the desktop the paths chosen for downloads still under way are held until
  each finishes, so two at once never share a name.
- `navigator.clipboard.writeText` writes through the shell.
- When the app returns from the background, the script dispatches
  `tcab:resume` on the console's `window`, and the console reopens its event
  stream. A return is seen two ways: the page turning visible after it was
  hidden, and the shell's own `tcab:shell-resume`, which Rust raises when the
  Android activity resumes, when the desktop window is shown again after it
  was minimized or hidden, and when the machine wakes from sleep (which no
  visibility change reports). Signals within 15 seconds of a dispatch are the
  same return and dispatch nothing more; 15 seconds covers the sleep watcher's
  ten-second interval, and a stream reopened that recently is not stale.

## Getting started

Open the superrepo in its devcontainer, which carries every toolchain the
repositories need, and work from this repository's root.
A build here inherits the superrepo's Cargo configuration, which redirects
every dependency on another repository to the sibling checkout, so nothing is
fetched from a mirror.

One command runs every gate:

```sh
make gate
```

Each gate is a file under `ci/gates/` whose stem is the gate's id.
`uv run --quiet --project ci gate list` names them all, and
`azure-pipelines.yml` runs each one as a step of its own on every push, inside
the CI images the superrepo builds. The Rust suite covers the configuration's
console, the download file names and the resume logic; the `typescript` gate
runs the injected script's tests and type check.

### Running the shell

Install the npm workspace first, which puts the Tauri CLI at
`node_modules/.bin/tauri`:

```sh
npm ci
```

A Linux desktop build needs WebKitGTK's development files, which the
devcontainer does not carry:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
```

Run the shell against a flavor from the crate's directory:

```sh
cd crates/client-tauri
../../node_modules/.bin/tauri dev --config tauri.staging.conf.json
```

`--config` also takes inline JSON, so the shell runs against a console served
on this machine, which the shell accepts over `http` on the loopback interface:

```sh
../../node_modules/.bin/tauri dev --config '{"plugins":{"console":{"url":"http://localhost:1430"}}}'
```

### Building the bundles

Build the bundles the way the pipeline does, from the repository root:

```sh
scripts/ci/bundle.sh linux /tmp/bundles
```

`scripts/ci/bundle.sh` builds every flavor for one target (`linux`, `android`,
`windows` or `macos`) into `<directory>/<flavor>/`. The Android target needs
the Android SDK, the NDK and the four Android Rust targets, which the `android`
CI image holds on amd64.

### Versions

The bundles carry the version `tauri.conf.json` names, and the Tauri CLI
derives the Android `versionCode` from it (major × 1000000 + minor × 1000 +
patch); Android would fall back to 1.0 were it absent. A release sets it to
the tag's version together with the workspace's `Cargo.toml`
(`workspace.package.version`), the root `package.json` and
`packages/shell/package.json`; a test holds all four equal.

Released bundles come from the pipeline: a `v*` tag's run builds every target
and publishes the bundles as the run's artifacts `client-windows`,
`client-linux`, `client-macos` and `client-android`, one directory per flavor.
The Android APK is the release build signed with the build machine's debug
key, so it installs by sideloading; the desktop bundles carry no signature.
