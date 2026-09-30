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
- **Scrollback**: the wheel scrolls back through it (full-screen programs get
  arrow keys, or wheel reports if they asked for the mouse), Shift+PageUp /
  PageDown by pages. The view stays put while new output arrives below.
- **Selection**: drag to select, across scrollback and screen; Cmd+C copies.
- **Resize**: the grid follows the window (content is cut, not reflowed).
- **Synchronized output** (mode 2026), which Claude Code uses, holds the
  frame until the update is complete.

Measured on an M-series Mac:

| | sprid | Ghostty 1.2.3 |
|---|---|---|
| Footprint, idle, one window | 74 MB | 684 MB (504 MB of it GPU surfaces) |
| Footprint after 256 MB of Claude-Code-like output | flat (`bench`) | grows ([the leak](https://mitchellh.com/writing/ghostty-memory-leak-fix)) |
| `cat` of 20 MB in the window | ~5 MB/s | GB/s |
| CPU, idle | ~2 % | ~0 % |

The last two rows are M2 and M4.

Not yet: reflow on resize, colour emoji (snaidhm reads outlines, not
bitmaps), mouse clicks reported to programs, the window title, a config file,
tabs and splits.

## Milestones

| Milestone | Done when |
|---|---|
| **M1 core** | ✅ Parser, screen and bounded scrollback pass their tests; real programs run on a PTY headless |
| **M3 window** | ✅ A native window through snaidhm: glyph cache, cell rendering, keyboard and IME, scrollback, selection, resize |
| **M2 speed** | A byte-level fast path for printable runs; ≥ 200 MB/s on `bench` |
| **M4 daily driver** | Idle CPU ~0 (the PTY wakes the event loop instead of polling it), reflow, config; a week of Claude Code sessions in sprid with the footprint flat |

## Requirements

Almide with the fix for [almide#3049](https://github.com/almide/almide/issues/3049)
([almide#3056](https://github.com/almide/almide/pull/3056)), and snaidhm with
modifier keys on input (branch `terminal-input`, checked out next to this
repository: `almide.toml` points at `../snaidhm`). Without it, calls such as
`erase_cells(t, t.y, t.x, t.cols)` silently pass the wrong arguments and the
tests fail.

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
