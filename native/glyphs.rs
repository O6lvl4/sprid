//! Printing done natively: the run of plain ASCII that most output is, and
//! the run of UTF-8 text — CJK, say — written straight into the grid
//! (src/vt/glyphs.almd `print_ascii_run`, `print_utf8_run`).

const SIZE: usize = 16;
const OFF_WIDTH: usize = 14;

/// The widths of the Basic Multilingual Plane, one byte per code point, as
/// src/width.almd builds them: given once (`set_widths`) rather than passed
/// on each call, which would copy the 64 KB each time.
static WIDTHS: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Keep `bmp`, the width table, for `print_utf8_run`. The first call counts.
pub fn set_widths(bmp: &[u8]) {
    if bmp.len() >= 0x10000 {
        let _ = WIDTHS.set(bmp.to_vec());
    }
}

/// Printable ASCII bytes of `data` from `i` (before `n`) into the cells from
/// byte `o` of `grid`, at most `max` of them, each over a cell one column
/// wide; `colors` and `tail` are the cell's second and last words as
/// glyphs.write_cell lays them out. How many cells were written: the run
/// stops at any other byte and at a wide char's half.
pub fn print_ascii_run(grid: &mut Vec<u8>, o: i64, data: &[u8], i: i64, n: i64, max: i64, colors: i64, tail: i64) -> i64 {
    let mut o = o.max(0) as usize;
    let mut j = i.max(0) as usize;
    let n = (n.max(0) as usize).min(data.len());
    let mut left = max.max(0) as usize;
    if o + left * SIZE > grid.len() { return 0; }
    let colors = (colors as u64).to_le_bytes();
    let tail = (tail as u32).to_le_bytes();
    let mut k = 0i64;
    while left > 0 && j < n {
        let b = data[j];
        if !(0x20..0x7F).contains(&b) || grid[o + OFF_WIDTH] != 1 { break; }
        grid[o..o + 4].copy_from_slice(&[b, 0, 0, 0]);
        grid[o + 4..o + 12].copy_from_slice(&colors);
        grid[o + 12..o + 16].copy_from_slice(&tail);
        o += SIZE;
        j += 1;
        left -= 1;
        k += 1;
    }
    k
}

/// Text of `data` from `i` (before `n`) into the cells from byte `o` of
/// `grid`, at most `max` columns of it: printable ASCII and UTF-8 sequences
/// of the Basic Multilingual Plane, each one column wide or — by the width
/// table (`set_widths`) — two, over plain cells, or a wide char over a wide
/// char. `colors` as `write_cell` lays it out; `attrs` the pen's, the width
/// added here. The run stops at anything else: a control byte, an invalid
/// or cut-short sequence, a combining mark, a char outside the plane, a
/// half of a wide char that would be left over, and at `max`.
///
/// 0 when nothing was taken; else the bytes taken, the columns written at
/// bit 24, the last character at bit 40 and, at bit 60, whether it was
/// wide.
pub fn print_utf8_run(grid: &mut Vec<u8>, o: i64, data: &[u8], i: i64, n: i64, max: i64, colors: i64, attrs: i64) -> i64 {
    let Some(bmp) = WIDTHS.get() else { return 0 };
    let mut o = o.max(0) as usize;
    let start = i.max(0) as usize;
    let mut j = start;
    let n = (n.max(0) as usize).min(data.len());
    let mut left = max.max(0) as usize;
    if o + left * SIZE > grid.len() { return 0; }
    let colors = (colors as u64).to_le_bytes();
    let attrs = attrs as u32;
    let mut cols = 0usize;
    let mut last = (0u32, 1usize);
    while left > 0 && j < n {
        let b = data[j];
        let (cp, len) = if (0x20..0x7F).contains(&b) {
            (b as u32, 1)
        } else if (0xC2..=0xDF).contains(&b) {
            if j + 1 >= n { break; }
            let b1 = data[j + 1];
            if !(0x80..=0xBF).contains(&b1) { break; }
            (((b as u32 & 0x1F) << 6) | (b1 as u32 & 0x3F), 2)
        } else if (0xE0..=0xEF).contains(&b) {
            if j + 2 >= n { break; }
            let (b1, b2) = (data[j + 1], data[j + 2]);
            if !(0x80..=0xBF).contains(&b1) || !(0x80..=0xBF).contains(&b2) { break; }
            // Overlong (E0 80..9F) and surrogates (ED A0..BF) are not text.
            if (b == 0xE0 && b1 < 0xA0) || (b == 0xED && b1 >= 0xA0) { break; }
            (((b as u32 & 0x0F) << 12) | ((b1 as u32 & 0x3F) << 6) | (b2 as u32 & 0x3F), 3)
        } else {
            break;
        };
        let w = if cp < 0x300 { 1 } else { bmp[cp as usize] as usize };
        if w == 0 || w > left { break; }
        let w0 = grid[o + OFF_WIDTH];
        if w == 1 {
            if w0 != 1 { break; }
        } else {
            let w1 = grid[o + SIZE + OFF_WIDTH];
            if !((w0 == 1 && w1 == 1) || (w0 == 2 && w1 == 0)) { break; }
        }
        grid[o..o + 4].copy_from_slice(&cp.to_le_bytes());
        grid[o + 4..o + 12].copy_from_slice(&colors);
        grid[o + 12..o + 16].copy_from_slice(&(attrs | ((w as u32) << 16)).to_le_bytes());
        if w == 2 {
            grid[o + SIZE..o + SIZE + 4].copy_from_slice(&[0, 0, 0, 0]);
            grid[o + SIZE + 4..o + SIZE + 12].copy_from_slice(&colors);
            grid[o + SIZE + 12..o + SIZE + 16].copy_from_slice(&attrs.to_le_bytes());
        }
        o += SIZE * w;
        j += len;
        left -= w;
        cols += w;
        last = (cp, w);
    }
    if j == start { return 0; }
    (j - start) as i64 | (cols as i64) << 24 | (last.0 as i64) << 40 | ((last.1 == 2) as i64) << 60
}
