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
  and `save_download`, through a capability the shell adds at startup.
- `window.open`, a link with a new-window target, a modifier or middle click
  on a link, and a link to a site other than the console and its sign-in flow
  open in the platform's browser.
- An `<a download>` the console activates is fetched in the page and written
  through the platform's download path, numbered rather than overwriting a
  file. A download the WebView starts itself, such as a run's archive, goes to
  the same place: the user's download directory on the desktop, and on
  Android the public Downloads collection through `MediaStore` and
  `DownloadManager`. The page shows a brief notice of where the file went.
- `navigator.clipboard.writeText` writes through the shell.
- When the app returns from the background (Android resuming the activity,
  the desktop window shown again after it was minimized or hidden, or the
  machine waking from sleep), the shell dispatches `tcab:resume` on the
  console's `window`, and the console reopens its event stream.

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

Released bundles come from the pipeline: a `v*` tag's run builds every target
and publishes the bundles as the run's artifacts `client-windows`,
`client-linux`, `client-macos` and `client-android`, one directory per flavor.
The Android APK is the release build signed with the build machine's debug
key, so it installs by sideloading; the desktop bundles carry no signature.
