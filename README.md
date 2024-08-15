# Spider 🕷️

Install and integrate web apps into the GNOME desktop!

## Features ✨

- [x] **Sandboxed**: Each app has an entirely separate instance of the WebKit browser
- [x] **Adaptive Window Styling**: Each app's titlebar adapts to it's [theme color](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/meta/name/theme-color)
- [x] **High quality favicons**: Scrapes websites for a high quality favicon to use

## Planned ✔️

- [ ] Get website data via webview
- [ ] Option to autostart and run apps in background
- [ ] Domain restriction
- [ ] More keybinds in web app
- [ ] Sideloading custom JS and CSS
- [ ] A better name and app icon

> ✨ Please let me know if you'd like any more features! ✨

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

## Thanks to these awesome projects! ❤️

- [jbenner-radham/rust-gtk4-css-styling](https://github.com/jbenner-radham/rust-gtk4-css-styling) (per-theme custom css)
- [gtk-rs/gtk4-rs](https://github.com/gtk-rs/gtk4-rs) (obviously lol)
- [eyekay/webapps](https://codeberg.org/eyekay/webapps) (the idea)
- [bilelmoussaoui/ashpd](https://github.com/bilelmoussaoui/ashpd) (the library and the quality examples)
