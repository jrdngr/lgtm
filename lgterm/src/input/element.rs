//! Paints a text box's line, selection and cursor, scrolled to keep the cursor in view.

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, ShapedLine, Style, TextAlign,
    TextRun, UnderlineStyle, Window, fill, point, px, relative, rgb, size,
};

use super::TextInput;
use crate::theme;

pub struct TextElement {
    pub input: Entity<TextInput>,
}

pub struct Prepaint {
    line: ShapedLine,
    selection: Option<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let input = self.input.read(cx);
        let style = window.text_style();
        let text = input.text.clone();
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match &input.marked {
            Some(m) => [
                (0..m.start, None),
                (m.clone(), Some(style.color)),
                (m.end..text.len(), None),
            ]
            .into_iter()
            .filter(|(r, _)| !r.is_empty())
            .map(|(r, line)| TextRun {
                len: r.len(),
                underline: line.map(|color| UnderlineStyle {
                    color: Some(color),
                    thickness: px(1.),
                    wavy: false,
                }),
                ..run.clone()
            })
            .collect(),
            None => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(text.into(), font_size, &runs, None);

        let x = line.x_for_index(input.cursor());
        let width = bounds.size.width - px(2.);
        let scroll = input
            .scroll
            .min(x)
            .max(x - width)
            .min((line.width - width).max(px(0.)))
            .max(px(0.));
        let left = bounds.left() - scroll;
        let selected = input.selected.clone();
        let focused = input.focus.is_focused(window);
        let selection = (focused && !selected.is_empty()).then(|| {
            fill(
                Bounds::from_corners(
                    point(left + line.x_for_index(selected.start), bounds.top()),
                    point(left + line.x_for_index(selected.end), bounds.bottom()),
                ),
                rgb(theme::SELECTION),
            )
        });
        let cursor = (focused && selected.is_empty()).then(|| {
            fill(
                Bounds::new(
                    point(left + x, bounds.top()),
                    size(px(1.5), bounds.size.height),
                ),
                rgb(theme::TEXT),
            )
        });
        self.input.update(cx, |i, _| i.scroll = scroll);
        Prepaint {
            line,
            selection,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus, scroll) = {
            let input = self.input.read(cx);
            (input.focus.clone(), input.scroll)
        };
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line = prepaint.line.clone();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection);
            }
            let origin = point(bounds.left() - scroll, bounds.top());
            let _ = line.paint(
                origin,
                window.line_height(),
                TextAlign::Left,
                None,
                window,
                cx,
            );
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
        self.input.update(cx, |i, _| {
            i.line = Some(line);
            i.bounds = Some(bounds);
        });
    }
}
