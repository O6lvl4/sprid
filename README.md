# sprid

> Irish: *sprid* — spirit, ghost.

A terminal emulator written in [Almide](https://github.com/almide/almide).
The parser, the screen, the scrollback are Almide; the only Rust is a thin
host for the PTY syscalls (`native/pty.rs`), the same way [snaidhm](https://github.com/almide-graphics/snaidhm)
hosts the window and the GPU.

## Why

A terminal left running an interactive CLI for days — styled lines full of
emoji and combining marks, scrolling for hours — puts all of that through
scrollback. It is where memory quietly grows: pages reused from a pool,
pages grown past their standard size to hold graphemes, side tables that
must stay in step with the lines they describe. Any one of them mistaken
for another, and memory is never given back.

sprid's scrollback is built so that this class of bug cannot happen, and
so that its memory is a number you can check, not a hope:

- **Nothing is pooled or reused.** A scrollback block is a plain `Bytes`,
  freed by reference counting the moment it is dropped.
- **A line owns all of its data.** Text, combining marks and styles are
  inline in the line record, with no side table to fall out of sync with it.
- **The budget is enforced on real bytes.** Sealed blocks are trimmed to
  their used length, and eviction drops whole blocks until the total fits.
  `scrollback.bytes_used` is the memory scrollback holds, not an estimate of
  it.

See `src/scrollback.almd`.

## Install

### macOS

Download `Sprid-<version>-arm64.zip` from
[Releases](https://github.com/O6lvl4/sprid/releases), unzip it and move
`Sprid.app` to `/Applications`. It needs Apple Silicon and macOS 13 or later.

The app is signed ad hoc, not notarized, so macOS stops it the first time.
Either open it once from System Settings → Privacy & Security → "Open
Anyway", or clear the quarantine flag:

```
xattr -dr com.apple.quarantine /Applications/Sprid.app
```

### Linux

Download `sprid-<version>-linux-<arch>.tar.gz` (x86_64 or aarch64) from
Releases, unpack it and run `./install.sh` in it: the binary goes to
`~/.local/bin`, its desktop entry and icon to `~/.local/share`. It needs
glibc 2.35 or later (Ubuntu 22.04, Debian 12 and newer).

- **Where it runs**: Wayland — typed into, Japanese composed with fcitx5 +
  Mozc, and pasted into under Hyprland — and X11.
- **Keys**: the Mac's Cmd shortcuts are Ctrl+Shift's — Ctrl+Shift+C / V to
  copy and paste, +T / W / N for tabs and windows, +F to find, +Plus / Minus
  / 0 for the font size, +Q to close the window — with Ctrl+Page Up / Down
  for tabs and F11 for full screen.
- **What it uses**: wl-copy / wl-paste under Wayland, else xclip or xsel, for
  the clipboard; Noto Sans Symbols 2 (`fonts-noto-core` on Debian and Ubuntu)
  for symbols such as ⏺.
- **Not there yet**: colour emoji, and the question before closing a tab or
  window with a program running (it closes without asking).
- **Checking it**: `docker build -t sprid-linux scripts/linux`, then
  `scripts/linux/check.sh` builds, tests and draws a screen under Xvfb with
  Mesa's lavapipe.

## Status: M3 — a window you can use

To build it yourself as a Mac app: `scripts/make-app.sh --install` builds `Sprid.app` (icon from
`scripts/make-icon.py`) and puts it in `~/Applications`, for Finder, Spotlight
and the Dock. Started that way it takes the login shell from the password
database and opens at home.

```
sprid                    open a window running your login shell
sprid run <command>      run <command> on a PTY headless, print the screen
sprid bench [MB]         stream styled, emoji-heavy output, report memory
sprid capture <png>      draw one screen of $SPRID_CAPTURE_CMD and save it
```

The window is [snaidhm](https://github.com/almide-graphics/snaidhm)'s:
winit for the window and input, wgpu for the GPU, snaidhm's glyph cache for
text. A frame is one draw call of quads — background runs, glyphs, lines,
cursor — built straight into a vertex buffer.

What works:

- **Text**: UDEV Gothic 35NFLG when installed (SF Mono, Menlo, DejaVu Sans Mono
  otherwise; `SPRID_FONT=<path>` to choose), falling back to Hiragino for
  Japanese and to Menlo / STIX Two Math / Apple Symbols for symbols such as
  ⏺ ✻ ✔. Bold, faint, underline, strikethrough, inverse,
  256 and direct colour, wide chars, combining marks. Theme: Tokyo Night Storm.
- **Input**: keys with Ctrl / Option-as-Alt / Shift in xterm's encoding,
  DECCKM, function keys, the macOS input method (a composition is drawn
  over the cursor's row, moved left to fit, with its caret and the segment
  being converted boxed; the candidates follow the caret), Cmd+V paste with bracketed paste.
- **Interactive CLIs**: the kitty keyboard protocol's disambiguation, so
  Shift+Enter starts a new line in the prompt; mouse presses, drags and
  motion reported to a program that asks (SGR), with Shift held for
  selecting; XTVERSION and the dark colour scheme answered.
- **Scrollback**: the wheel scrolls back through it (full-screen programs get
  arrow keys, or wheel reports if they asked for the mouse), Shift+PageUp /
  PageDown by pages. The view stays put while new output arrives below.
- **Selection**: drag to select, across scrollback and screen; double-click
  selects a word (a path or URL whole, Japanese by script), triple-click the
  line with the rows it wrapped onto, and a drag after either goes on by
  words or lines; Cmd+A selects it all, Cmd+C copies.
- **Mac keys**: Cmd+Left / Right to the start and end of the line,
  Cmd+Backspace deletes to its start, Option+Left / Right move by word (as
  other macOS terminals send them). Cmd+Home / End, Cmd+PageUp /
  PageDown and Cmd+Up / Down scroll. Cmd+Plus / Minus / 0 size the font
  (Cmd+; on a Japanese keyboard). Cmd+N opens a new window in the current
  tab's directory, Cmd+Shift+W closes it, Cmd+M minimizes, Cmd+Enter or
  Cmd+Ctrl+F goes full screen.
- **Clear** (Cmd+K): scrollback goes; at the shell's prompt the screen too
  (the shell is sent Ctrl+L and redraws its prompt at the top). A program
  running keeps its screen.
- **Colour emoji** from Apple Color Emoji: a wide character the grid face
  has no glyph for is drawn from the font's images, fitted to its two cells.
- **Tabs**: Cmd+T opens one in the current tab's directory, Cmd+W closes,
  Cmd+1-9, Ctrl+Tab / Ctrl+Shift+Tab or Cmd+Shift+[ ] switch, and so does a
  click on the tab bar. A tab is titled by its program's title, else its
  directory; double-click it to name it (Enter keeps, Escape drops, an empty
  name goes back to the automatic title). Closing a tab whose shell is running a
  program (an editor, a build) asks first, as do closing the window and
  quitting (Cmd+Q, the Dock) with any; a tab at its prompt closes at once.
- **Resize**: the main screen reflows — lines a program wrapped, on screen
  and in scrollback, are wrapped again at the new width, wide characters
  whole, the cursor at its place in its line. While the window is dragged
  only the screen is reflowed; scrollback follows once the size holds. The
  alternate screen is cut or padded: its program redraws it.
- **Synchronized output** (mode 2026) holds the
  frame until the update is complete.

### Measured

`scripts/bench.py` runs sprid as an app (`open -n -a`) with UDEV Gothic
35NFLG 14.5, a 100 x 30 grid, 10 MB of scrollback and a fixed script. A run ends when the script has stamped its marker
file AND the window is on screen (`scripts/window_shown.c`), so starting
the program first does not count as ready before the window shows. The
peak is the kernel's own record (`phys_footprint_peak`), not a sample. Two
sessions on an M-series Mac, each a median of 5 runs.

| | sprid |
|---|---|
| Start: window shown, first command run | 0.23-0.25 s |
| `cat` 21 MB of styled, emoji-heavy output | 0.25-0.26 s |
| Peak footprint by the end of that `cat` | 111-112 MB |
| CPU, idle (10-30 s after start) | 0.05-0.10 % |
| Footprint, idle | 40 MB |

Idle CPU is at the resolution of the measurement (one 10 ms tick in 20 s):
sampled, every sprid thread is asleep. `cat` is level; the parser is next —
rows scrolled into scrollback are a third of its time.

What moved memory and start:

- **No GPU transfer commands.** On Metal, the first blit of a process makes
  the driver hold ~46 MB for about a second, again whenever drawing resumes
  after idle. snaidhm writes buffers through mappings (per-frame vertices in
  a ring of mapped copies, the glyph atlas as a mapped storage buffer instead
  of a texture) and zeroes them at creation on the CPU. wgpu itself clears
  its internal zero buffer with a blit on the first submit; the peak above
  is with a wgpu-core patch that zeroes it through a mapping on integrated
  GPUs (see `almide-graphics/wgpu`, branch `v24-zero-buffer-on-cpu`). Without
  it, the peak is ~150 MB.
- **Fonts are mapped, not read.** Their pages are the file's, clean, and not
  counted against sprid — as with Core Text. `native/mapped.rs` is a global
  allocator that owns those mappings, so they can be ordinary `Vec<u8>`s.
- **The shell starts before the window**, and the window shows with its
  first frame, not empty while the GPU gets ready.

Earlier: the parser is 5x what it was (rows scrolled off are encoded
straight into scrollback, a width table, ASCII runs, one store per field
pair, CSI parameters in fixed buffers), a PTY read no longer allocates and
zeroes 64 KB per 1 KB it returns, and the window sleeps on its PTYs and
events at once instead of polling (snaidhm's `wait_fds`).

Not yet: emoji sequences (ZWJ, skin tones, flags), the window title, a config
file, splits.

## Milestones

| Milestone | Done when |
|---|---|
| **M1 core** | ✅ Parser, screen and bounded scrollback pass their tests; real programs run on a PTY headless |
| **M3 window** | ✅ A native window through snaidhm: glyph cache, cell rendering, keyboard and IME, scrollback, selection, resize |
| **M2 speed** | A byte-level fast path for printable runs; ≥ 200 MB/s on `bench` (now ~95) |
| **M4 daily driver** | ✅ idle CPU 0; reflow, config; a week of long-running CLI sessions in sprid with the footprint flat |

## Requirements

Almide with the fix for [almide#3049](https://github.com/almide/almide/issues/3049),
on `develop` since [almide#3087](https://github.com/almide/almide/pull/3087)
(Almide 0.65.1 and earlier releases lack it). Without it, calls such as
`erase_cells(t, t.y, t.x, t.cols)` silently pass the wrong arguments: the
tests fail, and in the app lines never rejoin when the window widens. snaidhm comes from its `main` branch.

Open compiler issues this project works around:
[#3045](https://github.com/almide/almide/issues/3045) (a `Bytes` parameter on
`@extern(rust)`: `pty.write` takes a `String` for now),
[#3050](https://github.com/almide/almide/issues/3050) (arguments bound with
`let` before an in-place stdlib call),
[#3051](https://github.com/almide/almide/issues/3051) (`let empty: List[T] = []`
before `t.field = empty`).

## Layout

```
native/pty.rs        forkpty, read with poll, write, resize (the OS boundary)
native/sys.rs        memory footprint and a clock
src/cell.almd        cell layout: 16 bytes per cell in one flat grid
src/width.almd       cell width of a codepoint
src/scrollback.almd  bounded scrollback
src/terminal.almd    parser + screen
src/view.almd        lines by absolute number: viewport and selection
src/keys.almd        keyboard input to xterm bytes
src/gui/fonts.almd   faces and fallbacks
src/gui/render.almd  the screen as quads
src/gui/app.almd     the window's loop
src/main.almd        CLI
```
