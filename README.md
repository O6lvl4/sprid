# sprid

> Irish: *sprid* — spirit, ghost.

A terminal emulator written in [Almide](https://github.com/almide/almide),
after [Ghostty](https://ghostty.org). The parser, the screen, the scrollback
are Almide; the only Rust is a thin host for the PTY syscalls
(`native/pty.rs`), the same way [snaidhm](https://github.com/almide-graphics/snaidhm)
hosts the window and the GPU.

## Why

Ghostty ≤ 1.2 leaked memory when it ran Claude Code for long: one user
reached 37 GB after 10 days
([the write-up](https://mitchellh.com/writing/ghostty-memory-leak-fix)). The
leak was in scrollback. Pages were reused from a pool, and a page grown past
the standard size to hold graphemes was mistaken for a pooled page once
reused, so it was never unmapped. Claude Code's output (styled lines full of
emoji and combining marks, scrolling for hours) triggers exactly that path.

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

## Status: M3 — a window you can use

As a Mac app: `scripts/make-app.sh --install` builds `Sprid.app` (icon from
`scripts/make-icon.py`) and puts it in `~/Applications`, for Finder, Spotlight
and the Dock. Started that way it takes the login shell from the password
database and opens at home.

```
sprid                    open a window running your login shell
sprid run <command>      run <command> on a PTY headless, print the screen
sprid bench [MB]         stream Claude-Code-like output, report memory
sprid capture <png>      draw one screen of $SPRID_CAPTURE_CMD and save it
```

The window is [snaidhm](https://github.com/almide-graphics/snaidhm)'s:
winit for the window and input, wgpu for the GPU, snaidhm's glyph cache for
text. A frame is one draw call of quads — background runs, glyphs, lines,
cursor — built straight into a vertex buffer.

What works:

- **Text**: UDEV Gothic 35NFLG when installed (SF Mono, Menlo, DejaVu Sans Mono
  otherwise; `SPRID_FONT=<path>` to choose), falling back to Hiragino for
  Japanese and to Menlo / STIX Two Math / Apple Symbols for the symbols
  Claude Code draws (⏺ ✻ ✔). Bold, faint, underline, strikethrough, inverse,
  256 and direct colour, wide chars, combining marks. Theme: Tokyo Night Storm.
- **Input**: keys with Ctrl / Option-as-Alt / Shift in xterm's encoding,
  DECCKM, function keys, the macOS input method (Japanese composition is drawn
  at the cursor), Cmd+V paste with bracketed paste.
- **Claude Code**: the kitty keyboard protocol's disambiguation, so
  Shift+Enter starts a new line in the prompt; mouse presses, drags and
  motion reported to a program that asks (SGR), with Shift held for
  selecting; XTVERSION and the dark colour scheme answered.
- **Scrollback**: the wheel scrolls back through it (full-screen programs get
  arrow keys, or wheel reports if they asked for the mouse), Shift+PageUp /
  PageDown by pages. The view stays put while new output arrives below.
- **Selection**: drag to select, across scrollback and screen; Cmd+C copies.
- **Colour emoji** from Apple Color Emoji: a wide character the grid face
  has no glyph for is drawn from the font's images, fitted to its two cells.
- **Tabs**: Cmd+T opens one in the current tab's directory, Cmd+W closes,
  Cmd+1-9, Ctrl+Tab / Ctrl+Shift+Tab or Cmd+Shift+[ ] switch, and so does a
  click on the tab bar. A tab is titled by its program's title, else its
  directory; double-click it to name it (Enter keeps, Escape drops, an empty
  name goes back to the automatic title). Closing a tab whose shell is running a
  program (Claude Code, a build) asks first, as do closing the window and
  quitting (Cmd+Q, the Dock) with any; a tab at its prompt closes at once.
- **Resize**: the main screen reflows — lines a program wrapped, on screen
  and in scrollback, are wrapped again at the new width, wide characters
  whole, the cursor at its place in its line. While the window is dragged
  only the screen is reflowed; scrollback follows once the size holds. The
  alternate screen is cut or padded: its program redraws it.
- **Synchronized output** (mode 2026), which Claude Code uses, holds the
  frame until the update is complete.

### Against Ghostty

`scripts/bench_vs.py` runs sprid and Ghostty 1.3.1 (the current release,
with its memory leak fixed) the same way: started as apps (`open -n -a`),
same font (UDEV Gothic 35NFLG 14.5), same grid (100 x 30), same 10 MB
scrollback, same script. A run ends when the script has stamped its marker
file AND the terminal's window is on screen (`scripts/window_shown.c`), so a
terminal that starts its program first is not ready before it shows. The
peak is the kernel's own record (`phys_footprint_peak`), not a sample. Two
sessions on an M-series Mac, each a median of 5 runs.

| | sprid | Ghostty 1.3.1 |
|---|---|---|
| Start: window shown, first command run | 0.23-0.25 s | 0.35-0.37 s |
| `cat` 21 MB of Claude-Code-like output | 0.25-0.26 s | 0.25-0.28 s |
| Peak footprint by the end of that `cat` | 111-112 MB | 130-132 MB |
| CPU, idle (10-30 s after start) | 0.05-0.10 % | 0.00-0.05 % |
| Footprint, idle | 40 MB | 62 MB |

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
| **M4 daily driver** | ✅ idle CPU 0; reflow, config; a week of Claude Code sessions in sprid with the footprint flat |

## Requirements

Almide with the fix for [almide#3049](https://github.com/almide/almide/issues/3049)
([almide#3061](https://github.com/almide/almide/pull/3061), in review). Without
it, calls such as `erase_cells(t, t.y, t.x, t.cols)` silently pass the wrong
arguments and the tests fail. snaidhm comes from its `main` branch.

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
