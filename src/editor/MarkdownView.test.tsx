import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { MarkdownView } from "./MarkdownView";

vi.mock("../ipc", () => ({ commands: { highlightText: async () => [] } }));
vi.mock("./Mermaid", () => ({ Mermaid: () => null }));
vi.mock("../ui/cmdHeld", () => ({ trackCmd() {}, cmdClick: () => false }));

const html = (text: string, onToggle?: (line: number) => void) =>
  renderToStaticMarkup(<MarkdownView text={text} lines onToggle={onToggle} />);

describe("MarkdownView", () => {
  it("renders the extended syntax", () => {
    const out = html(
      [
        "# Title {#top}",
        "",
        "==hi== H~2~O x^2^ ~~gone~~ :tada:",
        "",
        "Term",
        ": Meaning",
        "",
        "> [!NOTE]",
        "> Careful.",
        "",
        "$x^2$",
      ].join("\n"),
    );
    expect(out).toContain('<h1 id="top"');
    expect(out).toContain(">Title</h1>");
    expect(out).toContain("<mark>hi</mark>");
    expect(out).toContain("<sub>2</sub>");
    expect(out).toContain("<sup>2</sup>");
    expect(out).toContain("<del>gone</del>");
    expect(out).toContain("🎉");
    expect(out).toMatch(/<dl>\s*<dt[^>]*>Term<\/dt>\s*<dd[^>]*>Meaning\s*<\/dd>/);
    expect(out).toContain("markdown-alert-note");
    expect(out).toContain('class="katex"');
  });

  it("enables checkboxes only with onToggle", () => {
    const text = "- [ ] a\n- [x] b";
    expect(html(text)).toContain("disabled");
    expect(html(text, () => {})).not.toContain("disabled");
    expect(html(text, () => {})).toContain('data-start="2"');
  });
});
