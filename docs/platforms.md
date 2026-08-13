# Platform Support

DryMark shares one Rust policy engine and desktop UI across macOS, Windows,
and Linux. Platform clipboard and shortcut behavior still depends on operating
system facilities.

| Platform | Implemented surface | Verification evidence | Notes |
| --- | --- | --- | --- |
| macOS 10.15+ | Tray, settings, global shortcut, LLM watermark removal | Package build, strict bundle verification, and real clipboard, shortcut, and visual-feedback exercise on Apple Silicon | Source builds are ad-hoc signed and are not notarized distribution artifacts |
| Windows 10/11 | Tray, settings, global shortcut, LLM watermark removal | Automated tests, native compilation, and an NSIS installer build are CI gates; no Windows runtime claim | Requires WebView2, supplied by current Windows installs |
| Linux X11 | Tray, settings, global shortcut, LLM watermark removal | Automated tests, native compilation, and a Debian package build are CI gates; no live X11 runtime claim | AppIndicator support depends on the desktop environment; the native menu opens the full tray surface |
| Linux Wayland | Tray/settings and LLM watermark removal; shortcut capability varies | Linux compilation and Debian packaging are CI gates; no live Wayland runtime claim | The native tray menu remains the activation fallback when compositor support limits custom tray events or shortcuts |

“Implemented” describes code paths, not proof on every desktop combination.
The macOS package can be exercised with `make runtime-smoke-macos`; the harness
builds the package, uses synthetic fixtures, restores the prior clipboard, and
stops the process it launched. A successful package build alone is not runtime
proof.

The default shortcut is `Alt+Shift+V`. If another application owns it, DryMark
keeps the existing binding and reports a conflict instead of silently replacing
it.

## Linux Build Dependencies

Debian and Ubuntu builders need the current Tauri WebKit and tray dependencies:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential curl file libayatana-appindicator3-dev librsvg2-dev \
  libssl-dev libwebkit2gtk-4.1-dev libxdo-dev wget
```

Package names differ on Fedora, Arch, and other distributions. Use the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) as the current
reference.

## Packaging and Signing

`npm run tauri -- build -- --locked` creates the native package formats
available on the host OS. macOS source builds use an ad-hoc signature so the
application bundle and its resource seal can be verified locally. An ad-hoc
signature does not establish developer identity or notarization. Production
distribution should apply platform signing and, on macOS, notarization using
credentials held outside the repository.

## No Automatic Startup

DryMark does not add a login item, scheduled task, service, daemon, or desktop
autostart entry. Closing the process disables the global shortcut until the app
is opened again.
