//! The `?` list of every key: named groups laid out in columns, scrolling when they don't
//! fit.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use super::paint::Canvas;
use super::theme::{self, ACCENT, FG, HAIR, PANEL};

/// A named group of keys, each beside what it does.
pub struct Group {
    pub title: &'static str,
    pub keys: Vec<(&'static str, &'static str)>,
}

impl Group {
    pub fn new(title: &'static str, keys: &[(&'static str, &'static str)]) -> Self {
        Self {
            title,
            keys: keys.to_vec(),
        }
    }

    /// Its title's row and a row per key.
    fn height(&self) -> usize {
        1 + self.keys.len()
    }
}

/// Columns are this far apart.
const GAP: u16 = 4;

/// The `?` box while it's up: how far it's scrolled.
#[derive(Default)]
pub struct Help {
    top: usize,
    /// Rows that don't fit, from the last draw.
    hidden: usize,
}

impl Help {
    /// Scrolls on j k, C-d C-u and the arrows. Returns false for a key that closes it.
    pub fn key(&mut self, key: &KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let by: isize = match key.code {
            KeyCode::Char('j') | KeyCode::Down => 1,
            KeyCode::Char('k') | KeyCode::Up => -1,
            KeyCode::Char('d') if ctrl => 10,
            KeyCode::Char('u') if ctrl => -10,
            KeyCode::PageDown => 10,
            KeyCode::PageUp => -10,
            _ => return false,
        };
        self.top = self.top.saturating_add_signed(by).min(self.hidden);
        true
    }

    /// Draws `groups` in a box over the middle of `area`.
    pub fn draw(&mut self, c: &mut Canvas, area: Rect, groups: &[Group]) {
        if area.height < 3 || area.width < 6 {
            return;
        }
        let fits = area.height.saturating_sub(2) as usize;
        let cols = layout(groups, area.width.saturating_sub(4));
        let rows = cols.iter().map(|col| col.height).max().unwrap_or(0);
        self.hidden = rows.saturating_sub(fits);
        self.top = self.top.min(self.hidden);
        let inner: u16 = cols.iter().map(|col| col.width).sum::<u16>()
            + GAP * cols.len().saturating_sub(1) as u16;
        let w = (inner + 4).min(area.width);
        let h = (rows.min(fits) + 2) as u16;
        let x0 = area.x + (area.width - w) / 2;
        let y0 = area.y + (area.height - h) / 2;
        let border = theme::fg(HAIR).bg(PANEL);
        for y in y0..y0 + h {
            c.fill(x0, y, w, PANEL);
            c.put(x0, y, "│", border);
            c.put(x0 + w - 1, y, "│", border);
        }
        let rule = "─".repeat(w.saturating_sub(2) as usize);
        c.put(x0, y0, &format!("╭{rule}╮"), border);
        c.put(x0, y0 + h - 1, &format!("╰{rule}╯"), border);
        let mut x = x0 + 2;
        for col in &cols {
            let mut row = 0;
            for (i, g) in col.groups.iter().map(|&i| &groups[i]).enumerate() {
                if i > 0 {
                    row += 1;
                }
                let mut put = |row: usize, key: &str, label: &str, title: bool| {
                    let Some(y) = row.checked_sub(self.top).filter(|&r| r < fits) else {
                        return;
                    };
                    let y = y0 + 1 + y as u16;
                    if title {
                        c.put(x, y, label, theme::bold(FG).bg(PANEL));
                    } else {
                        c.put(x, y, key, theme::bold(ACCENT).bg(PANEL));
                        c.put(x + col.keys + 2, y, label, theme::fg(FG).bg(PANEL));
                    }
                };
                put(row, "", g.title, true);
                for (k, l) in &g.keys {
                    row += 1;
                    put(row, k, l, false);
                }
                row += 1;
            }
            x += col.width + GAP;
        }
        let track = h.saturating_sub(2) as usize;
        let thumb = (track * fits / rows.max(1)).clamp(1, track.max(1));
        if let Some(at) = ((track.saturating_sub(thumb)) * self.top).checked_div(self.hidden) {
            for i in at..at + thumb {
                c.put(
                    x0 + w - 1,
                    y0 + 1 + i as u16,
                    "┃",
                    theme::fg(ACCENT).bg(PANEL),
                );
            }
        }
    }
}

/// A column of whole groups.
#[derive(Debug, PartialEq)]
struct Column {
    groups: Vec<usize>,
    /// The widest key, and the whole column.
    keys: u16,
    width: u16,
    height: usize,
}

/// Packs `groups`, in order, into as many columns as `width` holds, for the shortest box.
/// More columns that leave it as tall aren't used.
fn layout(groups: &[Group], width: u16) -> Vec<Column> {
    let tall = |cols: &[Column]| cols.iter().map(|c| c.height).max().unwrap_or(0);
    let mut best = balanced(groups, 1);
    for n in 2..=groups.len() {
        let cols = balanced(groups, n);
        let inner = cols.iter().map(|c| c.width).sum::<u16>() + GAP * (cols.len() as u16 - 1);
        if inner > width {
            break;
        }
        if tall(&cols) < tall(&best) {
            best = cols;
        }
    }
    best
}

/// The shortest packing into at most `n` columns.
fn balanced(groups: &[Group], n: usize) -> Vec<Column> {
    let tallest = groups.iter().map(Group::height).max().unwrap_or(0);
    let total = groups.iter().map(Group::height).sum::<usize>() + groups.len();
    (tallest..=total)
        .map(|h| pack(groups, h))
        .find(|cols| cols.len() <= n)
        .unwrap_or_else(|| pack(groups, total))
}

/// Fills columns up to `height` rows each, a blank row between groups.
fn pack(groups: &[Group], height: usize) -> Vec<Column> {
    let mut cols: Vec<Vec<usize>> = vec![];
    let mut used = 0;
    for (i, g) in groups.iter().enumerate() {
        match cols.last_mut() {
            Some(col) if used + 1 + g.height() <= height => {
                col.push(i);
                used += 1 + g.height();
            }
            _ => {
                cols.push(vec![i]);
                used = g.height();
            }
        }
    }
    cols.into_iter()
        .map(|ids| {
            let width = |f: fn(&(&'static str, &'static str)) -> &'static str| {
                ids.iter()
                    .flat_map(|&i| groups[i].keys.iter().map(f))
                    .map(|s| s.chars().count())
                    .max()
                    .unwrap_or(0) as u16
            };
            let keys = width(|(k, _)| k);
            let labels = width(|(_, l)| l);
            let titles = ids
                .iter()
                .map(|&i| groups[i].title.chars().count())
                .max()
                .unwrap_or(0) as u16;
            let height = ids.iter().map(|&i| groups[i].height() + 1).sum::<usize>() - 1;
            Column {
                width: (keys + 2 + labels).max(titles),
                keys,
                height,
                groups: ids,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups(sizes: &[usize]) -> Vec<Group> {
        sizes
            .iter()
            .map(|&n| Group {
                title: "Group",
                keys: vec![("k", "label"); n],
            })
            .collect()
    }

    fn ids(cols: &[Column]) -> Vec<Vec<usize>> {
        cols.iter().map(|c| c.groups.clone()).collect()
    }

    #[test]
    fn groups_spread_over_as_many_balanced_columns_as_the_width_holds() {
        // Each column is 1 + 2 + 5 = 8 wide, and 12 with the gap.
        let g = groups(&[4, 3, 4, 3]);
        assert_eq!(ids(&layout(&g, 8)), [vec![0, 1, 2, 3]]);
        assert_eq!(ids(&layout(&g, 20)), [vec![0, 1], vec![2, 3]]);
        assert_eq!(ids(&layout(&g, 44)), [vec![0], vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn a_column_that_leaves_the_box_as_tall_is_left_out() {
        // The tallest group sets the height with two columns already.
        assert_eq!(ids(&layout(&groups(&[9, 1, 1]), 80)), [vec![0], vec![1, 2]]);
    }

    #[test]
    fn scrolling_stops_at_the_last_row_and_other_keys_close_it() {
        let mut help = Help { top: 0, hidden: 3 };
        for _ in 0..5 {
            assert!(help.key(&KeyEvent::from(KeyCode::Char('j'))));
        }
        assert_eq!(help.top, 3);
        assert!(!help.key(&KeyEvent::from(KeyCode::Esc)));
    }

    #[test]
    fn a_box_too_short_for_its_rows_scrolls_them_and_draws_a_thumb() {
        use ratatui::buffer::Buffer;
        let area = Rect::new(0, 0, 40, 6);
        let draw = |help: &mut Help| {
            let mut screen = Buffer::empty(area);
            help.draw(&mut Canvas::new(&mut screen, area), area, &groups(&[6]));
            screen
        };
        let mut help = Help::default();
        let screen = draw(&mut help);
        assert_eq!(help.hidden, 3);
        // 12 wide, centered: the right border is at 25.
        assert_eq!(screen[(25, 1)].symbol(), "┃");
        help.key(&KeyEvent::from(KeyCode::Char('j')));
        let screen = draw(&mut help);
        let row: String = (0..40).map(|x| screen[(x, 1)].symbol()).collect();
        assert!(row.contains("k  label"), "{row}");
    }

    #[test]
    fn a_box_with_room_to_spare_draws_no_thumb() {
        use ratatui::buffer::Buffer;
        let area = Rect::new(0, 0, 40, 20);
        let mut screen = Buffer::empty(area);
        Help::default().draw(&mut Canvas::new(&mut screen, area), area, &groups(&[3]));
        let all: String = screen.content().iter().map(|c| c.symbol()).collect();
        assert!(!all.contains('┃'));
    }
}
