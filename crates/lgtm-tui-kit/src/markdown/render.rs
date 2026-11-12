//! Markdown laid out as styled screen lines.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};

use super::table;
use crate::highlight;
use crate::theme::{self, ACCENT, BG, BLANK, FAINT, FG, GREEN, HAIR, PANEL};

/// Inline code's color: the highlighter's for strings.
const CODE: Color = theme::rgb(0xce9178);

/// Text in one style, and the link it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub style: Style,
    pub link: Option<String>,
}

impl Run {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            link: None,
        }
    }

    pub fn width(&self) -> usize {
        self.text.chars().count()
    }
}

/// A screen line: its runs, the source line it starts at, and the color the rest of it is
/// filled with.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub runs: Vec<Run>,
    pub src: usize,
    pub fill: Option<Color>,
}

#[cfg(test)]
impl Line {
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// A block that holds others.
enum Container {
    Quote,
    /// A list item's marker, until its first line takes it.
    Item {
        marker: String,
        used: bool,
    },
}

/// A list: the next item's number, when it's numbered.
struct List {
    next: Option<u64>,
}

struct Code {
    lang: String,
    text: String,
    src: usize,
}

/// Inline styles that are on.
#[derive(Default)]
struct Inline {
    bold: usize,
    italic: usize,
    strike: usize,
    heading: Option<HeadingLevel>,
    link: Option<String>,
}

struct Renderer {
    width: usize,
    starts: Vec<usize>,
    out: Vec<Line>,
    /// Inline text of the block being read, each run with its source line.
    runs: Vec<(Run, usize)>,
    inline: Inline,
    containers: Vec<Container>,
    lists: Vec<List>,
    code: Option<Code>,
    table: Option<table::Table>,
    /// Whether the next block goes right under the line before, as a list item's first does.
    tight: bool,
}

/// `text` laid out `width` columns wide.
pub fn render(text: &str, width: usize) -> Vec<Line> {
    let width = width.max(8);
    let mut r = Renderer {
        width,
        starts: std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
        out: vec![],
        runs: vec![],
        inline: Inline::default(),
        containers: vec![],
        lists: vec![],
        code: None,
        table: None,
        tight: false,
    };
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    for (ev, range) in Parser::new_ext(text, opts).into_offset_iter() {
        let src = r.line_at(range.start);
        r.event(ev, src);
    }
    r.flush();
    r.out
}

impl Renderer {
    fn line_at(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|&s| s <= offset)
            .saturating_sub(1)
    }

    fn style(&self) -> Style {
        let mut s = theme::fg(FG).bg(BG);
        if let Some(level) = self.inline.heading {
            s = heading_style(level);
        }
        if self.inline.bold > 0 {
            s = s.add_modifier(Modifier::BOLD);
        }
        if self.inline.italic > 0 {
            s = s.add_modifier(Modifier::ITALIC);
        }
        if self.inline.strike > 0 {
            s = s.add_modifier(Modifier::CROSSED_OUT);
        }
        if self.inline.link.is_some() {
            s = s.fg(ACCENT).add_modifier(Modifier::UNDERLINED);
        }
        s
    }

    fn push(&mut self, text: &str, style: Style, src: usize) {
        if let Some(t) = self.table.as_mut() {
            t.push(Run::new(text, style));
            return;
        }
        let mut run = Run::new(text, style);
        run.link = self.inline.link.clone();
        self.runs.push((run, src));
    }

    fn event(&mut self, ev: Event, src: usize) {
        if let Some(code) = self.code.as_mut() {
            match ev {
                Event::Text(t) => code.text.push_str(&t),
                Event::End(TagEnd::CodeBlock | TagEnd::MetadataBlock(_)) => self.end_code(),
                _ => {}
            }
            return;
        }
        match ev {
            Event::Start(tag) => self.start(tag, src),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.push(&t, self.style(), src),
            Event::Code(t) | Event::InlineMath(t) | Event::DisplayMath(t) => {
                let style = self.style().fg(CODE).bg(BLANK);
                self.push(&t, style, src);
            }
            Event::Html(t) | Event::InlineHtml(t) => {
                self.push(t.trim_end_matches('\n'), self.style(), src)
            }
            Event::FootnoteReference(label) => {
                let style = theme::fg(ACCENT).bg(BG);
                self.push(&format!("[{label}]"), style, src);
            }
            Event::SoftBreak => self.push(" ", self.style(), src),
            Event::HardBreak => {
                self.flush();
                self.tight = true;
            }
            Event::Rule => {
                self.flush();
                self.gap();
                let w = self.width.saturating_sub(self.prefix_width());
                self.emit(
                    vec![Run::new("─".repeat(w), theme::fg(HAIR).bg(BG))],
                    src,
                    None,
                );
            }
            Event::TaskListMarker(done) => {
                if let Some(Container::Item { marker, .. }) = self.containers.last_mut() {
                    *marker = if done { "☑ " } else { "☐ " }.to_string();
                }
            }
        }
    }

    fn start(&mut self, tag: Tag, src: usize) {
        match tag {
            Tag::Paragraph => self.block(),
            Tag::Heading { level, .. } => {
                self.block();
                self.inline.heading = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.block();
                self.containers.push(Container::Quote);
                self.tight = true;
            }
            Tag::CodeBlock(kind) => {
                self.block();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some(Code {
                    lang,
                    text: String::new(),
                    src: src + 1,
                });
            }
            Tag::MetadataBlock(_) => {
                self.block();
                self.code = Some(Code {
                    lang: "yaml".into(),
                    text: String::new(),
                    src: src + 1,
                });
            }
            Tag::List(first) => {
                self.flush();
                if self.lists.is_empty() {
                    self.gap();
                }
                self.lists.push(List { next: first });
            }
            Tag::Item => {
                self.flush();
                let depth = self.lists.len().saturating_sub(1);
                let marker = match self.lists.last_mut().and_then(|l| l.next.as_mut()) {
                    Some(n) => {
                        *n += 1;
                        format!("{}. ", *n - 1)
                    }
                    None => format!("{} ", ["•", "◦", "▪"][depth % 3]),
                };
                self.containers.push(Container::Item {
                    marker,
                    used: false,
                });
                self.tight = true;
            }
            Tag::FootnoteDefinition(label) => {
                self.block();
                self.containers.push(Container::Item {
                    marker: format!("[{label}] "),
                    used: false,
                });
                self.tight = true;
            }
            Tag::Table(aligns) => {
                self.block();
                self.table = Some(table::Table::new(
                    aligns
                        .into_iter()
                        .map(|a| match a {
                            Alignment::Right => table::Align::Right,
                            Alignment::Center => table::Align::Center,
                            _ => table::Align::Left,
                        })
                        .collect(),
                    src,
                ));
            }
            Tag::TableRow | Tag::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.row();
                }
            }
            Tag::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    t.cell();
                }
            }
            Tag::Emphasis => self.inline.italic += 1,
            Tag::Strong => self.inline.bold += 1,
            Tag::Strikethrough => self.inline.strike += 1,
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.inline.link = Some(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(level) => {
                self.flush();
                self.inline.heading = None;
                if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    let w = self.width.saturating_sub(self.prefix_width());
                    let (ch, color) = match level {
                        HeadingLevel::H1 => ("━", ACCENT),
                        _ => ("─", HAIR),
                    };
                    let src = self.out.last().map_or(0, |l| l.src);
                    self.emit(
                        vec![Run::new(ch.repeat(w), theme::fg(color).bg(BG))],
                        src,
                        None,
                    );
                }
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.containers.pop();
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::Item | TagEnd::FootnoteDefinition => {
                self.flush();
                self.containers.pop();
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    let w = self.width.saturating_sub(self.prefix_width());
                    for (runs, src) in t.lines(w) {
                        self.emit(runs, src, None);
                    }
                }
            }
            TagEnd::Emphasis => self.inline.italic = self.inline.italic.saturating_sub(1),
            TagEnd::Strong => self.inline.bold = self.inline.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.inline.strike = self.inline.strike.saturating_sub(1),
            TagEnd::Link | TagEnd::Image => self.inline.link = None,
            _ => {}
        }
    }

    /// Starts a block: what's pending goes out, and a blank line comes before it unless it
    /// opens a list item or a quote.
    fn block(&mut self) {
        self.flush();
        self.gap();
    }

    fn gap(&mut self) {
        if self.tight {
            self.tight = false;
            return;
        }
        if self.out.is_empty() {
            return;
        }
        let src = self.out.last().map_or(0, |l| l.src);
        // A blank line inside a quote keeps the quote's bar.
        let runs = self.prefix(false);
        self.out.push(Line {
            runs,
            src,
            fill: None,
        });
    }

    /// The pending inline text, wrapped under the containers' prefixes.
    fn flush(&mut self) {
        if self.runs.is_empty() {
            return;
        }
        let runs = std::mem::take(&mut self.runs);
        let src = runs[0].1;
        let words = words(runs.into_iter().map(|(r, _)| r));
        let mut line: Vec<Run> = vec![];
        let mut used = 0;
        let avail = self.width.saturating_sub(self.prefix_width()).max(4);
        for word in words {
            let w: usize = word.iter().map(Run::width).sum();
            let space = word.first().is_some_and(|r| r.text.starts_with(' '));
            if used + w > avail && used > 0 {
                self.emit(std::mem::take(&mut line), src, None);
                used = 0;
            }
            for mut run in word {
                if used == 0 && space {
                    run.text = run.text.trim_start().to_string();
                }
                // A word wider than the line is cut across lines.
                while used + run.width() > avail {
                    let take = avail - used;
                    let head: String = run.text.chars().take(take).collect();
                    run.text = run.text.chars().skip(take).collect();
                    line.push(Run {
                        text: head,
                        ..run.clone()
                    });
                    self.emit(std::mem::take(&mut line), src, None);
                    used = 0;
                }
                used += run.width();
                if !run.text.is_empty() {
                    line.push(run);
                }
            }
        }
        if !line.is_empty() {
            self.emit(line, src, None);
        }
    }

    fn end_code(&mut self) {
        let Some(code) = self.code.take() else {
            return;
        };
        let text = code.text.trim_end_matches('\n');
        let colors = highlight_lang(&code.lang, text);
        let avail = self.width.saturating_sub(self.prefix_width() + 2);
        for (i, line) in text.split('\n').enumerate() {
            let segs = colors.get(i).map(Vec::as_slice).unwrap_or(&[]);
            let mut runs = vec![Run::new(" ", theme::fg(FG).bg(PANEL))];
            let mut used = 0;
            for (piece, color) in highlight::runs(line, segs) {
                let piece: String = piece.chars().take(avail - used).collect();
                used += piece.chars().count();
                runs.push(Run::new(piece, theme::fg(color).bg(PANEL)));
            }
            self.emit(runs, code.src + i, Some(PANEL));
        }
    }

    /// Puts `runs` out after the containers' prefixes.
    fn emit(&mut self, runs: Vec<Run>, src: usize, fill: Option<Color>) {
        self.tight = false;
        let mut all = self.prefix(true);
        all.extend(runs);
        self.out.push(Line {
            runs: all,
            src,
            fill,
        });
    }

    fn prefix_width(&self) -> usize {
        self.containers
            .iter()
            .map(|c| match c {
                Container::Quote => 2,
                Container::Item { marker, .. } => marker.chars().count(),
            })
            .sum()
    }

    /// The containers' prefixes; `take` uses up the innermost item's marker.
    fn prefix(&mut self, take: bool) -> Vec<Run> {
        let mut out = vec![];
        let last = self.containers.len().saturating_sub(1);
        for (i, c) in self.containers.iter_mut().enumerate() {
            match c {
                Container::Quote => out.push(Run::new("▌ ", theme::fg(FAINT).bg(BG))),
                Container::Item { marker, used } => {
                    let shown = !*used && (take || i < last);
                    let text = if shown {
                        marker.clone()
                    } else {
                        " ".repeat(marker.chars().count())
                    };
                    let color = if marker.starts_with('☑') {
                        GREEN
                    } else {
                        ACCENT
                    };
                    out.push(Run::new(text, theme::fg(color).bg(BG)));
                    if take {
                        *used = true;
                    }
                }
            }
        }
        out
    }
}

fn heading_style(level: HeadingLevel) -> Style {
    let color = match level {
        HeadingLevel::H1 | HeadingLevel::H2 => ACCENT,
        _ => FG,
    };
    theme::bold(color).bg(BG)
}

/// Runs split into words, each a list of runs with the space before it.
fn words(runs: impl Iterator<Item = Run>) -> Vec<Vec<Run>> {
    let mut out: Vec<Vec<Run>> = vec![vec![]];
    for run in runs {
        let mut piece = String::new();
        for ch in run.text.chars() {
            if ch == ' ' && !piece.trim().is_empty() {
                out.last_mut().unwrap().push(Run {
                    text: std::mem::take(&mut piece),
                    ..run.clone()
                });
                out.push(vec![]);
            }
            piece.push(ch);
        }
        if !piece.is_empty() {
            out.last_mut().unwrap().push(Run { text: piece, ..run });
        }
    }
    out.retain(|w| !w.is_empty());
    out
}

/// Highlights `text` as code in `lang`, by the file extension it goes by.
fn highlight_lang(lang: &str, text: &str) -> Vec<Vec<lgtm_core::syntax::Segment>> {
    let ext = match lang.to_ascii_lowercase().as_str() {
        "rust" => "rs".to_string(),
        "python" => "py".into(),
        "javascript" => "js".into(),
        "typescript" => "ts".into(),
        "bash" | "shell" | "zsh" | "console" => "sh".into(),
        "yml" => "yaml".into(),
        "" => return vec![],
        other => other.to_string(),
    };
    lgtm_core::syntax::highlight_file(&format!("x.{ext}"), text)
}
