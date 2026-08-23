<br />
<div align="center">
  <a href="#">
    <img src="data/icons/hicolor/scalable/apps/io.github.zaedus.spider.svg" alt="Logo" height="120" alt="Spider Icon">
  </a>

  <h1 align="center">Spider</h1>

  <h3 align="center">
    Install web apps with ease
  </h3>

  <img src="https://raw.githubusercontent.com/Zaedus/spider/refs/heads/assets/screenshots/screenshot-01.png" alt="Spider Screenshot">
  <i>Special thanks to <a href="https://github.com/oiimrosabel">oiimrosabel</a> for the icon!</i>
</div>



## Features ✨

- [x] **Sandboxed**: Each app has an entirely separate instance of the WebKit browser
- [x] **Adaptive window styling**: Each app's titlebar adapts to it's [theme color](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/meta/name/theme-color)
- [x] **High quality favicons**: Scrapes websites for a high quality favicon to use
- [x] **Custom user agents**: Lets you set the app's [user agent](https://en.wikipedia.org/wiki/User_agent) if a website isn't behaving (defaults to a Linux Chrome UA, since WebKitGTK's stock Safari-on-macOS string makes sites like WhatsApp gate features away)
- [x] **Chromium Client-Hints shim**: Injects `navigator.userAgentData` into every page, so Chromium-only feature detection (e.g. WhatsApp Web calling) passes on WebKitGTK
- [x] **Website permissions**: Camera, microphone, location and notification prompts are remembered per site and can be revoked anytime from the app's settings page
- [x] **Notification click-through**: Clicking a site's notification focuses Spider and re-presents that app's window
- [x] **Background process management**: Apps running in the background can be killed straight from their settings page
- [x] **Website data via webview**: App metadata is fetched by rendering the site in a hidden webview, so JS-driven titles/icons work too
- [x] **Autostart & background**: Apps can launch at login and keep running when their window is closed (relaunching re-presents the running window)
- [x] **Domain restriction**: Optionally lock an app to its own domains; everything else opens in the system browser
- [x] **More keybinds**: Reload (<Ctrl>R/F5), force reload (<Ctrl><Shift>R), stop (Esc), zoom (<Ctrl>+/-/0) and start page (<Alt>Home)
- [x] **HTTP proxy settings**: Per-app proxy configuration
- [x] **Pop up handling**: `window.open` pop ups open in a small window sharing the app's session

> ✨ Please let me know if you'd like any more features! ✨

> DO NOT BE AFRIAD TO SUBMIT BUGS!
> I know there is lots of web functionality that you might be missing

### Known limitations ⚠️

- **Native builds can't do WebRTC calls**: most distros compile `RTCPeerConnection` out of
  WebKitGTK (`ENABLE_WEB_RTC` is experimental and off by default in release builds).
  Camera/mic *capture* works, but peer-to-peer calls will report the browser as unsupported.
  This is an engine limitation, not a Spider setting — it goes away once distros ship
  WebKitGTK with WebRTC enabled
  ([libwebrtc migration](https://github.com/WebKit/WebKit/pull/69116)).
- **The Flatpak build has the full calling stack** ✅: it bundles its own WebKitGTK built
  with `-DENABLE_WEB_RTC=ON` *and* `SharedArrayBuffer` enabled (the GTK port never exposes
  SAB, even on cross-origin-isolated pages, which WhatsApp's voip gate requires), grants
  read-only `/run/udev` so GStreamer can enumerate cameras, and injects a
  `navigator.userAgentData` shim. WhatsApp Web voice/video calls are verified working
  end to end.

## Building 🛠️

### GNOME Builder 🏗️

This project is easily buildable with [GNOME Builder](https://apps.gnome.org/Builder/).

### Meson 🖥️

To setup meson, run

```
meson setup target -Dbuildtype=debug --prefix="$HOME/.local"
```

Then to compile, run

```
ninja install -C target/
```

### Flatpak 📦

Cargo dependencies must be vendored for offline builds (`-Dflatpak=true` passes
`--offline` to cargo). If `vendor/` is missing, generate it first:

```
cargo vendor vendor
```

Then build and install:

```
flatpak-builder --user --ccache --force-clean --repo=repo target-flatpak build-aux/io.github.zaedus.spider.json
flatpak --user remote-add --no-gpg-verify --if-not-exists spider-local repo
flatpak --user install --reinstall spider-local io.github.zaedus.spider
```

Note: the first build compiles the bundled WebKitGTK and takes a while; ccache makes
subsequent builds much faster.

## Thanks to these awesome people and projects! ❤️

- [oiimrosabel](https://github.com/oiimrosabel) (the awesome icon!)
- [jbenner-radham/rust-gtk4-css-styling](https://github.com/jbenner-radham/rust-gtk4-css-styling) (per-theme custom css)
- [gtk-rs/gtk4-rs](https://github.com/gtk-rs/gtk4-rs) (obviously lol)
- [eyekay/webapps](https://codeberg.org/eyekay/webapps) (the idea)
- [bilelmoussaoui/ashpd](https://github.com/bilelmoussaoui/ashpd) (the library and the quality examples)
