# Design notes

## Scrollback that can't leak

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

## Rendering

The window is [snaidhm](https://github.com/almide-graphics/snaidhm)'s:
winit for the window and input, wgpu for the GPU, snaidhm's glyph cache for
text. A frame is one draw call of quads — background runs, glyphs, lines,
cursor — built straight into a vertex buffer.

A program part way through drawing a screen is not shown half drawn: inside
a synchronized update (mode 2026), or with the cursor hidden by a program
that hides it while it draws, the frame waits for the rest, and the PTY is
read through to it rather than 1 KB per turn of the loop.

## What moved memory and start

- **No GPU transfer commands.** On Metal, the first blit of a process makes
  the driver hold ~46 MB for about a second, again whenever drawing resumes
  after idle. snaidhm writes buffers through mappings (per-frame vertices in
  a ring of mapped copies, the glyph atlas as a mapped storage buffer instead
  of a texture) and zeroes them at creation on the CPU. wgpu itself clears
  its internal zero buffer with a blit on the first submit; the measured peak
  is with a wgpu-core patch that zeroes it through a mapping on integrated
  GPUs (`almide-graphics/wgpu`, branch `v24-zero-buffer-on-cpu`). Without
  it, the peak is ~150 MB.
- **Fonts are mapped, not read.** Their pages are the file's, clean, and not
  counted against sprid — as with Core Text. `native/mapped.rs` is a global
  allocator that owns those mappings, so they can be ordinary `Vec<u8>`s.
- **The shell starts before the window**, and the window shows with its
  first frame, not empty while the GPU gets ready.
- **The parser is 5x what it was**: rows scrolled off are encoded straight
  into scrollback, a width table, ASCII runs, one store per field pair, CSI
  parameters in fixed buffers. A PTY read no longer allocates and zeroes
  64 KB per 1 KB it returns, and the window sleeps on its PTYs and events at
  once instead of polling (snaidhm's `wait_fds`).

## How it is measured

`scripts/bench.py` runs sprid as an app (`open -n -a`) with UDEV Gothic
35NFLG 14.5, a 100 x 30 grid, 10 MB of scrollback and a fixed script. A run
ends when the script has stamped its marker file AND the window is on screen
(`scripts/window_shown.c`), so starting the program first does not count as
ready before the window shows. The peak is the kernel's own record
(`phys_footprint_peak`), not a sample. Two sessions on an M-series Mac, each
a median of 5 runs. Idle CPU is at the resolution of the measurement (one
10 ms tick in 20 s): sampled, every sprid thread is asleep.

For debugging: `SPRID_DEBUG=1` reports wakes and input-to-frame latency every
2 s, `SPRID_TRACE=1` logs every read and frame, and
`SPRID_AUTOSCROLL="dy,start ms,end ms,every ms"` turns the wheel by itself.

## Compiler workarounds

sprid needs Almide with the fix for
[almide#3049](https://github.com/almide/almide/issues/3049), on `develop`
since [almide#3087](https://github.com/almide/almide/pull/3087) (0.65.1 and
earlier lack it): without it, calls such as `erase_cells(t, t.y, t.x, t.cols)`
silently pass the wrong arguments.

Open issues it works around:
[#3045](https://github.com/almide/almide/issues/3045) (a `Bytes` parameter on
`@extern(rust)`: `pty.write` takes a `String` for now),
[#3050](https://github.com/almide/almide/issues/3050) (arguments bound with
`let` before an in-place stdlib call),
[#3051](https://github.com/almide/almide/issues/3051) (`let empty: List[T] = []`
before `t.field = empty`).
