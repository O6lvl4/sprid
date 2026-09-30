//! Colour emoji from Apple Color Emoji: the `sbix` table's PNG images,
//! decoded and fitted to a cell box. Backs `src/gui/emoji.almd`.
//!
//! Only the lookup snaidhm's font reader doesn't do lives here: `cmap` (to
//! find the glyph), `maxp` (glyph count) and `sbix` (the images). The font
//! file is mapped (`mapped.rs`), so its 180 MB cost only the pages read.

use crate::AlmideRcCow;
use std::cell::RefCell;

const PATH: &str = "/System/Library/Fonts/Apple Color Emoji.ttc";

/// Where things are in the font, found once.
struct Font {
    data: Vec<u8>,
    /// Offset of the cmap subtable (format 12 or 4) and its format.
    cmap: Option<(usize, u16)>,
    num_glyphs: usize,
    /// Offset of each strike, with its pixels per em.
    strikes: Vec<(usize, u16)>,
}

thread_local! {
    static FONT: RefCell<Option<Option<Font>>> = const { RefCell::new(None) };
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> { d.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]])) }
fn u32_at(d: &[u8], o: usize) -> Option<u32> { d.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])) }

fn open() -> Option<Font> {
    let data = crate::mapped::map_file(PATH)?;
    // A collection: the first font's table directory.
    let base = if data.get(0..4) == Some(b"ttcf") { u32_at(&data, 12)? as usize } else { 0 };
    let tables = u16_at(&data, base + 4)? as usize;
    let mut find = |tag: &[u8; 4]| {
        (0..tables).find_map(|i| {
            let rec = base + 12 + i * 16;
            (data.get(rec..rec + 4)? == tag).then(|| u32_at(&data, rec + 8).map(|o| o as usize)).flatten()
        })
    };
    let (cmap, maxp, sbix) = (find(b"cmap")?, find(b"maxp")?, find(b"sbix")?);
    let num_glyphs = u16_at(&data, maxp + 4)? as usize;
    // The best Unicode subtable: format 12 (all planes), else format 4 (BMP).
    let mut sub: Option<(usize, u16)> = None;
    for i in 0..u16_at(&data, cmap + 2)? as usize {
        let rec = cmap + 4 + i * 8;
        let (platform, encoding) = (u16_at(&data, rec)?, u16_at(&data, rec + 2)?);
        let unicode = platform == 0 || (platform == 3 && (encoding == 1 || encoding == 10));
        let at = cmap + u32_at(&data, rec + 4)? as usize;
        let format = u16_at(&data, at)?;
        if unicode && (format == 12 || (format == 4 && sub.is_none())) {
            sub = Some((at, format));
        }
    }
    let count = u32_at(&data, sbix + 4)? as usize;
    let strikes = (0..count)
        .filter_map(|i| {
            let at = sbix + u32_at(&data, sbix + 8 + i * 4)? as usize;
            Some((at, u16_at(&data, at)?))
        })
        .collect();
    Some(Font { data, cmap: sub, num_glyphs, strikes })
}

fn glyph_id(f: &Font, cp: u32) -> u32 {
    let d = &f.data;
    let Some((at, format)) = f.cmap else { return 0 };
    let found = if format == 12 {
        let groups = u32_at(d, at + 12).unwrap_or(0) as usize;
        let (mut lo, mut hi) = (0, groups);
        let mut gid = None;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let g = at + 16 + mid * 12;
            let (start, end) = (u32_at(d, g).unwrap_or(0), u32_at(d, g + 4).unwrap_or(0));
            if cp < start { hi = mid } else if cp > end { lo = mid + 1 } else {
                gid = u32_at(d, g + 8).map(|s| s + cp - start);
                break;
            }
        }
        gid
    } else {
        // Format 4: segments of the BMP.
        (|| {
            if cp > 0xFFFF { return None; }
            let segs = u16_at(d, at + 6)? as usize / 2;
            let ends = at + 14;
            let starts = ends + segs * 2 + 2;
            let deltas = starts + segs * 2;
            let ranges = deltas + segs * 2;
            let i = (0..segs).find(|&i| u16_at(d, ends + i * 2).map_or(false, |e| cp as u16 <= e))?;
            let start = u16_at(d, starts + i * 2)? as u32;
            if cp < start { return None; }
            let delta = u16_at(d, deltas + i * 2)?;
            let range = u16_at(d, ranges + i * 2)? as usize;
            let g = if range == 0 { cp as u16 } else {
                let o = ranges + i * 2 + range + (cp - start) as usize * 2;
                let g = u16_at(d, o)?;
                if g == 0 { return None; }
                g
            };
            Some(g.wrapping_add(delta) as u32)
        })()
    };
    found.filter(|&g| (g as usize) < f.num_glyphs).unwrap_or(0)
}

/// The PNG of glyph `gid` in the smallest strike at least `px` per em (else
/// the largest), with the strike's pixels per em.
fn png_of(f: &Font, gid: u32, px: u32) -> Option<(&[u8], u16)> {
    let d = &f.data;
    let mut strikes = f.strikes.clone();
    strikes.sort_by_key(|&(_, ppem)| ppem);
    let order = strikes.iter().filter(|s| s.1 as u32 >= px).chain(strikes.iter().rev());
    for &(at, ppem) in order {
        let o = at + 4 + gid as usize * 4;
        let (start, end) = (u32_at(d, o)? as usize, u32_at(d, o + 4)? as usize);
        if end > start + 8 && d.get(at + start + 4..at + start + 8) == Some(b"png ") {
            return Some((&d[at + start + 8..at + end], ppem));
        }
    }
    None
}

fn decode(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(png);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..(w * h * 4) as usize].to_vec(),
        png::ColorType::Rgb => buf[..(w * h * 3) as usize].chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..(w * h * 2) as usize].chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf[..(w * h) as usize].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

/// Emoji `cp` fitted into a `w` x `h` box, centred, as bytes: width and height
/// (u32, little-endian) then RGBA rows, straight alpha. Empty when the font
/// has no image for it.
pub fn bitmap(cp: i64, w: i64, h: i64) -> AlmideRcCow<Vec<u8>> {
    let out = FONT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let font = slot.get_or_insert_with(open).as_ref()?;
        let (bw, bh) = (w.max(1) as u32, h.max(1) as u32);
        let gid = glyph_id(font, cp as u32);
        if gid == 0 { return None; }
        let (png, _) = png_of(font, gid, bw.max(bh))?;
        let (iw, ih, rgba) = decode(png)?;
        Some(fit(&rgba, iw, ih, bw, bh))
    });
    AlmideRcCow::new(out.unwrap_or_default())
}

/// `rgba` (`iw` x `ih`) scaled to fit `bw` x `bh`, centred: each output pixel
/// the average of the source pixels it covers, in premultiplied alpha so
/// transparent edges don't darken.
fn fit(rgba: &[u8], iw: u32, ih: u32, bw: u32, bh: u32) -> Vec<u8> {
    let scale = (bw as f64 / iw as f64).min(bh as f64 / ih as f64);
    let (tw, th) = (((iw as f64 * scale).round() as u32).max(1), ((ih as f64 * scale).round() as u32).max(1));
    let (ox, oy) = ((bw - tw.min(bw)) / 2, (bh - th.min(bh)) / 2);
    let mut out = Vec::with_capacity(8 + (bw * bh * 4) as usize);
    out.extend_from_slice(&bw.to_le_bytes());
    out.extend_from_slice(&bh.to_le_bytes());
    out.resize(8 + (bw * bh * 4) as usize, 0);
    for y in 0..th.min(bh) {
        let (sy0, sy1) = (y * ih / th, ((y + 1) * ih / th).max(y * ih / th + 1));
        for x in 0..tw.min(bw) {
            let (sx0, sx1) = (x * iw / tw, ((x + 1) * iw / tw).max(x * iw / tw + 1));
            let mut acc = [0u64; 4];
            for sy in sy0..sy1.min(ih) {
                for sx in sx0..sx1.min(iw) {
                    let p = ((sy * iw + sx) * 4) as usize;
                    let a = rgba[p + 3] as u64;
                    acc[0] += rgba[p] as u64 * a;
                    acc[1] += rgba[p + 1] as u64 * a;
                    acc[2] += rgba[p + 2] as u64 * a;
                    acc[3] += a;
                }
            }
            let n = ((sy1.min(ih) - sy0) * (sx1.min(iw) - sx0)).max(1) as u64;
            let o = 8 + (((y + oy) * bw + x + ox) * 4) as usize;
            if acc[3] > 0 {
                for c in 0..3 {
                    out[o + c] = (acc[c] / acc[3]).min(255) as u8;
                }
                out[o + 3] = (acc[3] / n).min(255) as u8;
            }
        }
    }
    out
}
