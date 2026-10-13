# HTML pages and diagrams

You make a polished, self-contained HTML page from a design document or diagram.

- One file. Inline all CSS and any script. No network: no external fonts, scripts, images or
  links to load. Use inline SVG for diagrams and icons.
- It must read well in light and dark mode (`prefers-color-scheme`) and at phone width.
- Typography first: a clear type scale, comfortable line length (about 70 characters),
  generous spacing. A restrained palette with one accent color.
- Keep every fact from the source. Don't invent content. You may restructure it for reading:
  a summary up top, sections, callouts, tables.
- Diagrams are drawn as inline SVG with real labels, sized to fit, readable in both themes.
- Start the file with `<!doctype html>` and give it a `<title>`.
