# GazeGuard

GazeGuard is a break reminder that runs a countdown, warns before breaks, and delivers a blocked break screen on supported platforms.

## Install As A User

### Android

Download the latest Android APK from the repository's **Releases** page. The release artifact is a universal APK containing:

- `arm64-v8a` for current phones
- `armeabi-v7a` for older 32-bit ARM phones
- `x86` and `x86_64` for emulators

On the phone, open the APK and approve installation from that source if Android asks. The release APK currently is unsigned for development/testing, so Android may show an extra warning.

### macOS

Download the universal DMG from **Releases**, open it, and drag GazeGuard to Applications. The DMG contains both Apple Silicon (`arm64`) and Intel (`x86_64`) binaries.

The current CI DMG is unsigned and not notarized. macOS may require Control-clicking the app and choosing **Open**, or approving it in **System Settings → Privacy & Security**.

## Android Testing

Install over USB with Android Debug Bridge (ADB):

```bash
adb install -r app-universal-release-unsigned.apk
```

Install over Wi-Fi:

```bash
adb pair PHONE_IP:PAIR_PORT
adb connect PHONE_IP:ADB_PORT
adb install -r app-universal-release-unsigned.apk
```

Enable **Developer options → Wireless debugging** on the phone first. Bluetooth is suitable for transferring the APK, but it does not install or debug the app by itself; open the transferred APK on the phone.

For a short test cycle, edit the app config inside the app sandbox:

```bash
adb shell run-as com.reus.gazeguard ls files/config
adb shell run-as com.reus.gazeguard cat files/config/config.yaml
```

Use a short interval, such as `short_break_interval: 1`, then launch the app and verify:

1. The background process stays alive.
2. A break starts at the configured time.
3. The blocked break screen appears over the current app.
4. Home, Back, notifications, and app switching do not incorrectly end the break.
5. The break ends and normal use resumes.

## Build Locally

## macOS WebdriverIO E2E tests

The E2E suite uses WebdriverIO's embedded Tauri WebDriver, which is the macOS-compatible path. It requires macOS, Rust, and a debug Tauri build:

```bash
npm run test:e2e
```

The runner builds `src-tauri/target/debug/gazeguard` first when needed. Tests use an isolated app session and cover settings persistence, break controls, and the fullscreen break window. The embedded driver is provided by `tauri-plugin-wdio-webdriver`; no `tauri-driver` process is used on macOS.

### Requirements

- Rust stable and Cargo
- Tauri CLI `2.10.1`
- Java 21
- Android SDK platform 36, build-tools 36.1.0, and NDK 29.0.14206865 for Android
- macOS with Xcode command-line tools for the macOS build

Install the pinned Tauri CLI:

```bash
cargo install tauri-cli --version 2.10.1 --locked
```

### Rust Tests

```bash
cd src-tauri
cargo test
```

Linux also needs the Tauri system libraries used by CI:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential libayatana-appindicator3-dev libssl-dev \
  libwebkit2gtk-4.1-dev libxdo-dev librsvg2-dev
```

### Android Universal APK

```bash
cd src-tauri
cargo tauri android init --ci --skip-targets-install
cargo tauri android build \
  --apk \
  --target aarch64 armv7 i686 x86_64 \
  --ci
```

Output:

```text
src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

`tauri.settings.gradle` is generated and ignored by Git. A clean checkout must run Android init before Gradle commands.

### macOS Universal DMG

On macOS:

```bash
cd src-tauri
cargo tauri build \
  --target universal-apple-darwin \
  --bundles dmg \
  --no-sign \
  --ci
```

Output:

```text
src-tauri/target/universal-apple-darwin/release/bundle/dmg/gazeguard_0.1.0_universal.dmg
```

Verify the binary contains both architectures:

```bash
lipo -info src-tauri/target/universal-apple-darwin/release/gazeguard
```

## Configuration

The runtime config is YAML. The embedded defaults are in `src-tauri/config/defaults.yaml`.

Useful values include:

```yaml
short_break_interval: 15
short_break_duration: 15
long_break_duration: 60
log_level: off
```

For desktop diagnostic logging, set:

```yaml
log_level: debug
```

Logs are written to stderr when the app is launched from a terminal. Logging is off by default.

## CI And Releases

### Pull Requests

`.github/workflows/tests.yml` runs on every pull request:

- Rust unit tests on Ubuntu
- Android unit tests using the universal debug variant
- macOS WebdriverIO E2E tests

### Release Flow

Pushes to `main` run Release Please. It opens or updates a release PR. Merging that PR creates a version tag such as `v0.1.0`.

Tags matching `v*` run `.github/workflows/build-binaries.yml`, which:

1. Builds the universal Android APK.
2. Builds the universal macOS DMG.
3. Publishes both files to the GitHub Release.

Release Please updates the version in all app manifests:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

The repository secret `RELEASE_PLEASE_TOKEN` must contain a PAT or GitHub App token with access to this repository and permission to read/write contents, issues, and pull requests.

To manually rerun Release Please after the workflow is available:

```bash
gh workflow run "Release Please" --repo JorgeReus/GazeGuard --ref main
```

## Signing And Distribution

The current CI artifacts are for development/testing:

- Android APK: unsigned
- macOS DMG: unsigned and not notarized

Before public distribution, configure Android signing and macOS Developer ID signing/notarization. Do not put signing keys or certificates in the repository. Store them as encrypted GitHub Actions secrets and use short-lived credentials where possible.

The desktop updater also requires `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` GitHub Actions secrets. The matching public key is stored in `src-tauri/tauri.conf.json`; never replace it unless all existing users migrate to a newly signed build.

## Troubleshooting

`tauri.settings.gradle does not exist`:

```bash
cd src-tauri
cargo tauri android init --ci --skip-targets-install
```

`glib-2.0 was not found` on Linux: install the Linux dependencies listed above.

`cargo install tauri-cli` reports missing Tauri bundler fields: use the pinned command with `--locked`.

Android cannot install the APK: uninstall an incompatible signed build or use `adb install -r`; Android may reject an update signed by a different key.

macOS says the app cannot be opened: the artifact is unsigned. Use **Open** from Finder's context menu or install a signed/notarized build.

## Development Layout

- `src/`: frontend assets
- `src-tauri/src/`: Rust application and break engine
- `src-tauri/config/defaults.yaml`: embedded defaults
- `src-tauri/gen/android/`: generated Android project
- `.github/workflows/tests.yml`: pull request tests
- `.github/workflows/release-please.yml`: release PR automation
- `.github/workflows/build-binaries.yml`: tagged binary builds
