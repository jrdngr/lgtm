//! Box-drawing and block characters drawn as rectangles that fill their cell, so lines and
//! blocks join across rows and columns instead of leaving the font's gaps.

/// A rectangle in cell pixels, `[left, top, right, bottom]`, with its opacity.
pub type Piece = ([f32; 4], f32);

/// Line weights of the four arms, up, right, down, left: 0 none, 1 light, 2 heavy.
type Arms = [u8; 4];

/// U+2500 to U+254B; dashed lines (U+2504 to U+250B) are left to the font.
#[rustfmt::skip]
const LINES: [Option<Arms>; 76] = {
    const fn a(s: &[u8; 4]) -> Option<Arms> {
        Some([s[0] - b'0', s[1] - b'0', s[2] - b'0', s[3] - b'0'])
    }
    [
        a(b"0101"), a(b"0202"), a(b"1010"), a(b"2020"), None, None, None, None, None, None, None,
        None, a(b"0110"), a(b"0210"), a(b"0120"), a(b"0220"), a(b"0011"), a(b"0012"), a(b"0021"),
        a(b"0022"), a(b"1100"), a(b"1200"), a(b"2100"), a(b"2200"), a(b"1001"), a(b"1002"),
        a(b"2001"), a(b"2002"), a(b"1110"), a(b"1210"), a(b"2110"), a(b"1120"), a(b"2120"),
        a(b"2210"), a(b"1220"), a(b"2220"), a(b"1011"), a(b"1012"), a(b"2011"), a(b"1021"),
        a(b"2021"), a(b"2012"), a(b"1022"), a(b"2022"), a(b"0111"), a(b"0112"), a(b"0211"),
        a(b"0212"), a(b"0121"), a(b"0122"), a(b"0221"), a(b"0222"), a(b"1101"), a(b"1102"),
        a(b"1201"), a(b"1202"), a(b"2101"), a(b"2102"), a(b"2201"), a(b"2202"), a(b"1111"),
        a(b"1112"), a(b"1211"), a(b"1212"), a(b"2111"), a(b"1121"), a(b"2121"), a(b"2112"),
        a(b"2211"), a(b"1122"), a(b"1221"), a(b"2212"), a(b"1222"), a(b"2122"), a(b"2221"),
        a(b"2222"),
    ]
};

fn arms(c: char) -> Option<Arms> {
    let a = |s: &[u8; 4]| Some([s[0] - b'0', s[1] - b'0', s[2] - b'0', s[3] - b'0']);
    match c as u32 {
        n @ 0x2500..=0x254b => LINES[(n - 0x2500) as usize],
        // Rounded corners, drawn square.
        0x256d => a(b"0110"),
        0x256e => a(b"0011"),
        0x256f => a(b"1001"),
        0x2570 => a(b"1100"),
        0x2574 => a(b"0001"),
        0x2575 => a(b"1000"),
        0x2576 => a(b"0100"),
        0x2577 => a(b"0010"),
        0x2578 => a(b"0002"),
        0x2579 => a(b"2000"),
        0x257a => a(b"0200"),
        0x257b => a(b"0020"),
        0x257c => a(b"0201"),
        0x257d => a(b"1020"),
        0x257e => a(b"0102"),
        0x257f => a(b"2010"),
        _ => None,
    }
}

/// Block elements, U+2580 to U+259F, as fractions of the cell.
fn blocks(c: char) -> Option<Vec<Piece>> {
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| ([x0, y0, x1, y1], 1.);
    let shade = |a: f32| vec![([0., 0., 1., 1.], a)];
    let (ul, ur, ll, lr) = (
        r(0., 0., 0.5, 0.5),
        r(0.5, 0., 1., 0.5),
        r(0., 0.5, 0.5, 1.),
        r(0.5, 0.5, 1., 1.),
    );
    Some(match c as u32 {
        0x2580 => vec![r(0., 0., 1., 0.5)],
        n @ 0x2581..=0x2588 => {
            let h = (n - 0x2580) as f32 / 8.;
            vec![r(0., 1. - h, 1., 1.)]
        }
        n @ 0x2589..=0x258f => vec![r(0., 0., (0x2590 - n) as f32 / 8., 1.)],
        0x2590 => vec![r(0.5, 0., 1., 1.)],
        0x2591 => shade(0.25),
        0x2592 => shade(0.5),
        0x2593 => shade(0.75),
        0x2594 => vec![r(0., 0., 1., 0.125)],
        0x2595 => vec![r(0.875, 0., 1., 1.)],
        0x2596 => vec![ll],
        0x2597 => vec![lr],
        0x2598 => vec![ul],
        0x2599 => vec![ul, ll, lr],
        0x259a => vec![ul, lr],
        0x259b => vec![ul, ur, ll],
        0x259c => vec![ul, ur, lr],
        0x259d => vec![ur],
        0x259e => vec![ur, ll],
        0x259f => vec![ur, ll, lr],
        _ => return None,
    })
}

/// Whether `c` is drawn here rather than by the font.
pub fn drawn(c: char) -> bool {
    arms(c).is_some() || matches!(c as u32, 0x2580..=0x259f)
}

/// The rectangles for `c` in a `w` by `h` cell, with light lines `light` thick. Edges land on
/// the cell's edges so neighbors join.
pub fn pieces(c: char, w: f32, h: f32, light: f32) -> Option<Vec<Piece>> {
    if let Some(b) = blocks(c) {
        return Some(
            b.into_iter()
                .map(|([x0, y0, x1, y1], a)| ([x0 * w, y0 * h, x1 * w, y1 * h], a))
                .collect(),
        );
    }
    let [up, right, down, left] = arms(c)?;
    let thick = |weight: u8| light * weight as f32;
    let (cx, cy) = ((w / 2.).floor(), (h / 2.).floor());
    // Arms reach past the center by half the widest crossing line, so corners close.
    let reach_v = thick(up.max(down).max(left.max(right))) / 2.;
    let mut out = Vec::new();
    let mut vertical = |weight: u8, y0: f32, y1: f32| {
        if weight > 0 {
            let t = thick(weight);
            let x0 = cx - (t / 2.).floor();
            out.push(([x0, y0, x0 + t, y1], 1.));
        }
    };
    vertical(up, 0., cy + reach_v);
    vertical(down, cy - reach_v, h);
    let mut horizontal = |weight: u8, x0: f32, x1: f32| {
        if weight > 0 {
            let t = thick(weight);
            let y0 = cy - (t / 2.).floor();
            out.push(([x0, y0, x1, y0 + t], 1.));
        }
    };
    horizontal(left, 0., cx + reach_v);
    horizontal(right, cx - reach_v, w);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{drawn, pieces};

    #[test]
    fn lines_and_blocks_reach_the_cell_edges_so_rows_join() {
        let (w, h) = (8., 18.);
        let vertical = pieces('│', w, h, 1.).unwrap();
        assert_eq!(
            vertical,
            [([4., 0., 5., 9.5], 1.), ([4., 8.5, 5., 18.], 1.)]
        );
        let horizontal = pieces('─', w, h, 1.).unwrap();
        assert_eq!(
            horizontal,
            [([0., 9., 4.5, 10.], 1.), ([3.5, 9., 8., 10.], 1.)]
        );
        assert_eq!(pieces('█', w, h, 1.).unwrap(), [([0., 0., 8., 18.], 1.)]);
        assert_eq!(pieces('▄', w, h, 1.).unwrap(), [([0., 9., 8., 18.], 1.)]);
    }

    #[test]
    fn a_heavy_corner_closes_and_dashes_and_letters_stay_with_the_font() {
        let corner = pieces('┏', 8., 18., 1.).unwrap();
        // Down arm from above the center, right arm from left of the center.
        assert_eq!(corner, [([3., 8., 5., 18.], 1.), ([3., 8., 8., 10.], 1.)]);
        assert!(!drawn('┄'));
        assert!(!drawn('M'));
        assert!(drawn('╭'));
    }
}
