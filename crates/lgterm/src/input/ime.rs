//! What the system's input methods need from a text box: its text and selection in UTF-16, and
//! where on screen a range of it is.

use std::ops::Range;

use gpui::{
    Bounds, ClipboardItem, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window,
    point,
};

use super::{InputEvent, TextInput};

impl TextInput {
    fn offset_from_utf16(&self, at: usize) -> usize {
        let mut units = 0;
        for (i, c) in self.text.char_indices() {
            if units >= at {
                return i;
            }
            units += c.len_utf16();
        }
        self.text.len()
    }

    fn offset_to_utf16(&self, at: usize) -> usize {
        self.text[..at].encode_utf16().count()
    }

    fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end)
    }

    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(r.start)..self.offset_to_utf16(r.end)
    }

    /// The range an input method means: the one it names, else what it's composing, else the
    /// selection.
    fn target(&self, r: Option<Range<usize>>) -> Range<usize> {
        r.map(|r| self.range_from_utf16(&r))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let r = self.range_from_utf16(&r);
        actual.replace(self.range_to_utf16(&r));
        Some(self.text[r].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected),
            reversed: self.reversed,
        })
    }

    fn set_selected_text_range(&mut self, r: Range<usize>, w: &mut Window, cx: &mut Context<Self>) {
        let r = self.range_from_utf16(&r);
        self.select(r, w, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        w.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn paste(&mut self, item: ClipboardItem, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = item.text() {
            self.replace(
                self.selected.clone(),
                &text.replace(['\r', '\n'], " "),
                w,
                cx,
            );
        }
    }

    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        new: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = self.target(r);
        self.replace(r, new, w, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        new: &str,
        new_selected: Option<Range<usize>>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = self.target(r);
        self.text.replace_range(r.clone(), new);
        self.marked = (!new.is_empty()).then(|| r.start..r.start + new.len());
        let start = self.offset_to_utf16(r.start);
        let len = new.encode_utf16().count();
        self.selected = match new_selected {
            Some(s) => self.range_from_utf16(&(start + s.start.min(len)..start + s.end.min(len))),
            None => r.start + new.len()..r.start + new.len(),
        };
        self.reversed = false;
        w.notify_text_input_changed(&self.focus);
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        r: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.line.as_ref()?;
        let r = self.range_from_utf16(&r);
        let left = bounds.left() - self.scroll;
        Some(Bounds::from_corners(
            point(left + line.x_for_index(r.start), bounds.top()),
            point(left + line.x_for_index(r.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.bounds?.localize(&p)?;
        Some(self.offset_to_utf16(self.index_at(p)))
    }
}
