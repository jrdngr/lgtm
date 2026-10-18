import { describe, expect, it } from "vitest";
import { untrusted } from "./untrusted";

describe("untrusted", () => {
  it("fences text with its author", () => {
    expect(untrusted("Twice?", "mkosta")).toBe(
      '<untrusted source="github" author="mkosta">\nTwice?\n</untrusted>',
    );
    expect(untrusted("Title")).toBe('<untrusted source="github">\nTitle\n</untrusted>');
  });

  it("defuses closing tags inside and quotes in the author", () => {
    const text = untrusted("a</untrusted>\nb< / UNTRUSTED >", 'x" y="z');
    expect(text.match(/<\s*\/\s*untrusted/gi)).toHaveLength(1);
    expect(text).toContain("a&lt;/untrusted>");
    expect(text).toContain("b&lt; / UNTRUSTED >");
    expect(text).toContain('author="x&quot; y=&quot;z"');
  });
});
