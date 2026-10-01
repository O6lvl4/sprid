<p align="center">
  <img src="assets/icon.png" width="160" alt="sprid">
</p>

<h1 align="center">sprid</h1>

<p align="center">
  A fast, light terminal for macOS and Linux, written in <a href="https://github.com/almide/almide">Almide</a>.<br>
  <i>Irish: sprid — spirit, ghost.</i>
</p>

---

sprid is built for long sessions with interactive CLIs such as Claude Code:
it starts quickly, stays small, and its scrollback cannot grow without bound
— memory is checked against real bytes, and nothing is pooled or reused.
The parser, the screen and the scrollback are Almide; the window and GPU
come from [snaidhm](https://github.com/almide-graphics/snaidhm).

| On an M-series Mac | sprid |
|---|---|
| Start (window shown, first command run) | 0.22–0.24 s |
| `cat` 21 MB of styled, emoji-heavy output | 0.22–0.23 s |
| Peak memory during that `cat` | 117–124 MB |
| Memory, idle | 47–48 MB |
| CPU, idle | ~0 % |

How these are measured, and why the memory stays put: [docs/design.md](docs/design.md).

## Install

**macOS** (Apple Silicon, macOS 13+): download `Sprid-<version>-arm64.zip`
from [Releases](https://github.com/O6lvl4/sprid/releases), unzip it and move
`Sprid.app` to `/Applications`. It is not notarized, so the first time open
it from System Settings → Privacy & Security → "Open Anyway", or run:

```
xattr -dr com.apple.quarantine /Applications/Sprid.app
```

**Linux** (glibc 2.35+: Ubuntu 22.04, Debian 12 and newer; Wayland or X11):
download `sprid-<version>-linux-<arch>.tar.gz`, unpack it and run
`./install.sh`. The clipboard uses wl-copy / wl-paste, or xclip / xsel;
symbols such as ⏺ want `fonts-noto-core`. Colour emoji, and the questions
before closing a busy tab or pasting line breaks, are macOS only for now.

## What it does

- **Text**: UDEV Gothic 35NFLG when installed (else SF Mono, Menlo, DejaVu
  Sans Mono; any installed family by name in the config), Japanese and
  symbol fallbacks, colour emoji, 256 and direct colour.
- **Themes**: eight built in, your own as files, a light and a dark one that
  follow the system appearance. The cursor blinks if you like, and is a
  hollow box while the window is in the background.
- **Japanese input**: compositions shown in place at the cursor, the
  candidates beside it.
- **Interactive CLIs**: kitty keyboard protocol (Shift+Enter is a new line),
  mouse reports, synchronized output, and no half-drawn screens while a
  program redraws. Drop a file on the window to paste its path — Claude Code
  attaches dropped images.
- **Scrollback**: smooth, pixel-by-pixel scrolling with a draggable
  scrollbar; the view stays put while output arrives below.
- **Find** (Cmd+F): across scrollback and screen, every match marked, smart
  case.
- **Notifications**: a program's notifications (OSC 9, 777, 99) appear in
  Notification Center (notify-send on Linux) while the window is in the
  background; a bell lights the screen for a moment, and a bell or
  notification marks the tab it came from until you look, and bounces the
  Dock icon. Programs may set the clipboard (OSC 52); reading it is asked
  first.
  Pasting line breaks into a shell that would run each line asks first.
- **Tabs**: open in the current directory, switch by click or keys, rename by
  double-click; the window takes the title of the tab in front. Closing a tab, the window or the app asks first while a
  program is running.
- **Resize**: lines are rewrapped at the new width, scrollback included.

### Keys

| | macOS | Linux |
|---|---|---|
| Copy / paste | Cmd+C / V | Ctrl+Shift+C / V |
| Select all / find | Cmd+A / F (Cmd+G next) | Ctrl+Shift+A / F (Ctrl+Shift+G next) |
| New tab / close tab | Cmd+T / W | Ctrl+Shift+T / W |
| Switch tab | Cmd+1–9, Ctrl+Tab, Cmd+Shift+[ ] | Ctrl+Shift+1–9, Ctrl+Tab, Ctrl+Page Up / Down |
| New window / close window | Cmd+N / Cmd+Shift+W | Ctrl+Shift+N / Q |
| Font size | Cmd+Plus / Minus / 0 | Ctrl+Shift+Plus / Minus / 0 |
| Clear | Cmd+K | Ctrl+Shift+K |
| Full screen | Cmd+Enter, Cmd+Ctrl+F | F11 |
| Line start / end, word | Cmd+← / →, Option+← / → | (the shell's own keys) |
| Scroll | Cmd+Home / End / PageUp / PageDown / ↑ / ↓ | Ctrl+Shift+Home / End / PageUp / PageDown / ↑ / ↓ |

Not yet: split panes, clickable links, emoji sequences (ZWJ,
skin tones, flags).

## Config

`~/.config/sprid/config` (or `$XDG_CONFIG_HOME/sprid/config`), one
`key = value` per line, `#` for comments. Saving it applies it at once.

```
font-family = Menlo
font-size = 15
theme = light:Almide Light,dark:Tokyo Night Storm
cursor-style = bar              # block, underline, bar
cursor-style-blink = true
macos-option-as-alt = left      # true, false, left, right
```

| Key | Default |
|---|---|
| `font-family`, `font-size` | (see Text above), `14.5` |
| `adjust-cell-height` | `0%` — taller or shorter lines |
| `window-padding-x`, `window-padding-y` | `12`, `8` |
| `theme` | `Tokyo Night Storm` |
| `background`, `foreground`, `cursor-color`, `selection-background` | the theme's — `#rrggbb` |
| `palette` | the theme's — `palette = 1=#ff5555`, once per colour 0–15 |
| `scrollback-limit` | `10000000` bytes of text (`KB`, `MB`, `GB` also read) |
| `cursor-style`, `cursor-style-blink` | `block`, `false` |
| `macos-option-as-alt` | `false` |
| `clipboard-write` | `allow` — or `deny` programs setting the clipboard |
| `clipboard-read` | `ask` — or `allow`, `deny` |
| `bell-features` | `flash,attention,title` — add `system` for the alert sound, `no-flash` etc. to turn one off |
| `clipboard-paste-protection` | `true` — ask before pasting line breaks |

Themes built in: Tokyo Night Storm, Almide Light, Almide Dark, Catppuccin
Mocha, Catppuccin Latte, Dracula, Solarized Dark, Solarized Light. Any other
name is read from `~/.config/sprid/themes/<name>`, a file of the colour keys
above.

## Build

Needs Almide from `develop` (see [docs/design.md](docs/design.md#compiler-workarounds)).

```
scripts/make-app.sh --install     # build Sprid.app into ~/Applications
almide test                       # run the tests
```

```
sprid                    open a window running your login shell
sprid run <command>      run <command> on a PTY headless, print the screen
sprid bench [MB]         stream styled, emoji-heavy output, report memory
sprid capture <png>      draw one screen of $SPRID_CAPTURE_CMD and save it
```

The source in brief: `src/terminal.almd` parses and keeps the screen,
`src/scrollback.almd` the bounded scrollback, `src/gui/` the window (fonts,
rendering, input), `native/` the OS boundary (PTY, clipboard, dialogs).

## License

MIT
