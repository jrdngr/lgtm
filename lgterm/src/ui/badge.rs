//! A layout's colored letter badge in the sidebar.

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, Styled, div, px, rgb};

use super::{marks, theme};
use crate::status::Status;

const COLORS: [u32; 8] = [
    0x3f9e98, 0x5a9a52, 0xd27a45, 0x5b86e5, 0xa066d3, 0xc95a7a, 0xb59a3c, 0x4f9ac9,
];

/// The badge's letter: the label's first letter or digit, uppercased, or `~` for home.
pub fn letter(label: &str) -> String {
    if label.starts_with('~') {
        return "~".to_owned();
    }
    label
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "·".to_owned())
}

/// The color for the layout at `index` in the sidebar, so neighbors always differ.
fn color(index: usize) -> u32 {
    COLORS[index % COLORS.len()]
}

/// The badge, with `status` on its corner when given.
pub fn badge(index: usize, label: &str, status: Option<Status>, ring: u32) -> AnyElement {
    div()
        .relative()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .bg(rgb(color(index)))
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::BADGE_TEXT))
        .child(letter(label))
        .when_some(status, |d, s| {
            d.child(
                div()
                    .absolute()
                    .right(px(-4.))
                    .bottom(px(-4.))
                    .size(px(11.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(ring))
                    .child(marks::mark(s)),
            )
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::letter;

    #[test]
    fn letters_skip_punctuation_and_keep_home() {
        assert_eq!(letter("chess-trainer"), "C");
        assert_eq!(letter(".dotfiles"), "D");
        assert_eq!(letter("~"), "~");
        assert_eq!(letter("~/notes"), "~");
        assert_eq!(letter("élan"), "É");
        assert_eq!(letter("--"), "·");
    }
}
