//! A space's colored letter badge in the sidebar.

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, FontWeight, IntoElement, ParentElement, Styled, div, px, rgb};

use crate::theme;

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

/// A number chip, shown while a modifier is held, for the digit key that picks its item.
pub fn chip(n: usize) -> gpui::Div {
    div()
        .flex_none()
        .min_w(px(16.))
        .h(px(16.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(rgb(theme::CHIP))
        .border_1()
        .border_color(rgb(theme::CHIP_BORDER))
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme::TEXT))
        .child(n.to_string())
}

/// The badge, with a status mark on its corner when given.
pub fn badge(color: usize, label: &str, mark: Option<AnyElement>, ring: u32) -> AnyElement {
    div()
        .relative()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .bg(rgb(theme::space_color(color)))
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(theme::BADGE_TEXT))
        // A line as tall as the badge, centered across it, so the letter sits in the middle
        // whatever the font's own line height.
        .child(
            div()
                .w_full()
                .h_full()
                .line_height(px(20.))
                .text_center()
                .child(letter(label)),
        )
        .when_some(mark, |d, mark| {
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
                    .child(mark),
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
