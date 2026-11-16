//! Printing done natively: the run of plain ASCII that most output is,
//! written straight into the grid (src/vt/glyphs.almd `print_ascii_run`).

const SIZE: usize = 16;
const OFF_WIDTH: usize = 14;

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
