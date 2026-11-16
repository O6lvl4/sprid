//! Scrollback records of grid rows, written natively: src/record.almd's
//! `used_cols_at` and `write`, byte for byte, for a row without combining
//! marks (every line that scrolls off, as good as). Per cell a few loads and
//! stores instead of a dozen checked byte operations; what `cat` of a long
//! file spends most of its time on otherwise.
const SIZE: usize = 16;
/// record.STYLE_MASK: background, attributes, link and prompt mark of a
/// cell's second 8 bytes — all but width and the grapheme flag.
const STYLE_MASK: u64 = 0xFE00_FFFF_FFFF_FFFF;
/// record.BLANK_MASK / BLANK_TAIL: default background, no attributes,
/// width 1.
const BLANK_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;
const BLANK_TAIL: u64 = 0x0001_0000_0000_0000;
const RUN_HEADER: usize = 14;

#[inline(always)]
fn u32_at(g: &[u8], p: usize) -> u32 { u32::from_le_bytes([g[p], g[p + 1], g[p + 2], g[p + 3]]) }
#[inline(always)]
fn u64_at(g: &[u8], p: usize) -> u64 { u64::from_le_bytes(g[p..p + 8].try_into().unwrap()) }
#[inline(always)]
fn put_u16(o: &mut [u8], p: usize, v: u16) { o[p..p + 2].copy_from_slice(&v.to_le_bytes()) }
#[inline(always)]
fn put_u32(o: &mut [u8], p: usize, v: u32) { o[p..p + 4].copy_from_slice(&v.to_le_bytes()) }
#[inline(always)]
fn put_u64(o: &mut [u8], p: usize, v: u64) { o[p..p + 8].copy_from_slice(&v.to_le_bytes()) }

/// Columns of the row at byte `off` of `grid` up to and including the last
/// cell that shows something, the cells from `within` on known blank.
pub fn used_cols(g: &[u8], off: i64, within: i64) -> i64 {
    let off = off as usize;
    let mut n = within.max(0) as usize;
    while n > 0 {
        let c = off + (n - 1) * SIZE;
        if c + SIZE > g.len() { n -= 1; continue; }
        if u32_at(g, c) == 0 && u64_at(g, c + 8) & BLANK_MASK == BLANK_TAIL { n -= 1 } else { break }
    }
    n as i64
}

/// The record of `used` columns of the row at byte `off` of `grid` into
/// `out` at `start`, which has room for record.bound bytes; where it ends.
pub fn write(o: &mut Vec<u8>, start: i64, g: &[u8], off: i64, used: i64, wrapped: bool) -> i64 {
    let start = start as usize;
    let off = off as usize;
    let used = used.max(0) as usize;
    if start + 11 + used * (RUN_HEADER + 4) > o.len() || off + used * SIZE > g.len() { return start as i64; }
    put_u16(o, start + 4, used as u16);
    o[start + 6] = wrapped as u8;
    let mut p = start + 7;
    let mut x = 0usize;
    while x < used {
        let c = off + x * SIZE;
        let fg = u32_at(g, c + 4);
        let style = u64_at(g, c + 8) & STYLE_MASK;
        put_u32(o, p, fg);
        put_u64(o, p + 4, style);
        let text_start = p + RUN_HEADER;
        let mut q = text_start;
        while x < used {
            let c = off + x * SIZE;
            let head = u64_at(g, c);
            let tail = u64_at(g, c + 8);
            if tail & STYLE_MASK != style || (head >> 32) as u32 != fg { break; }
            // Nothing for a wide char's second half, a space for a cell
            // never written.
            if (tail >> 48) & 0xFF != 0 {
                let cp = head as u32;
                let cp = if cp == 0 { 32 } else { cp };
                if cp < 0x80 {
                    o[q] = cp as u8;
                    q += 1;
                } else if cp < 0x800 {
                    o[q] = 0xC0 | (cp >> 6) as u8;
                    o[q + 1] = 0x80 | (cp & 0x3F) as u8;
                    q += 2;
                } else if cp < 0x10000 {
                    o[q] = 0xE0 | (cp >> 12) as u8;
                    o[q + 1] = 0x80 | ((cp >> 6) & 0x3F) as u8;
                    o[q + 2] = 0x80 | (cp & 0x3F) as u8;
                    q += 3;
                } else {
                    o[q] = 0xF0 | (cp >> 18) as u8;
                    o[q + 1] = 0x80 | ((cp >> 12) & 0x3F) as u8;
                    o[q + 2] = 0x80 | ((cp >> 6) & 0x3F) as u8;
                    o[q + 3] = 0x80 | (cp & 0x3F) as u8;
                    q += 4;
                }
            }
            x += 1;
        }
        // The text length over the 4 bytes past the style, with the flags'
        // top 7 bits.
        put_u32(o, p + 10, (q - text_start) as u32 | (((style >> 57) & 0x7F) as u32) << 24);
        p = q;
    }
    put_u32(o, start, (p - start - 4) as u32);
    p as i64
}
