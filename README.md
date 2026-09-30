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

## Status: M1 — the core, headless

```
sprid run <command>    run <command> on a PTY, print the final screen
sprid bench [MB]       stream Claude-Code-like output, report memory
```

`bench 256` on an M-series Mac (10 MB scrollback budget):

```
fed MB   footprint   scrollback   lines kept   MB/s
     0       1 MiB        0 MiB          163   0.0
    32      20 MiB        9 MiB        77258   15.5
   128       7 MiB        9 MiB        77123   16.7
   256       6 MiB        9 MiB        76985   16.9
```

Memory is flat however much output arrives. Throughput is low: 17 MB/s,
where Ghostty does GB/s. That is plenty for Claude Code, which writes KB/s,
but not for `cat`-ing a large file. Making the hot loop fast is M2.

What works: the DEC VT500 parser state machine with UTF-8, CSI cursor
movement / erase / insert / delete / scroll, margins, SGR (16, 256 and direct
color, colon sub-parameters), wide chars, combining marks, the alternate
screen (47/1047/1049), DEC special graphics, tabs, save/restore cursor,
DSR/DA replies, OSC titles, and skipping DCS/APC/PM/SOS. Everything is
covered by `src/*_test.almd`.

## Milestones

| Milestone | Done when |
|---|---|
| **M1 core** | Parser, screen and bounded scrollback pass their tests; real programs run on a PTY headless |
| **M2 speed** | A byte-level fast path for printable runs; ≥ 200 MB/s on `bench` |
| **M3 window** | A native window through snaidhm: glyph atlas, cell rendering, keyboard and IME, resize with reflow |
| **M4 daily driver** | A week of Claude Code sessions in sprid: no crash, footprint flat, measured against Ghostty |

## Requirements

Almide with the fix for [almide#3049](https://github.com/almide/almide/issues/3049)
(branch `fix-hoist-unique-names`). Without it, calls such as
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
native/sys.rs        memory footprint and a clock, for bench
src/cell.almd        cell layout: 16 bytes per cell in one flat grid
src/width.almd       cell width of a codepoint
src/scrollback.almd  bounded scrollback
src/terminal.almd    parser + screen
src/main.almd        headless CLI
```
