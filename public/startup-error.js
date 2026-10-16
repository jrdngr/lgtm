// Shows an error that stops the app from starting, which would otherwise leave the window blank.
addEventListener("error", (e) => {
  const root = document.getElementById("root");
  if (!root || root.childElementCount > 0) return;
  const pre = document.createElement("pre");
  pre.style.cssText =
    "margin:16px;color:#f85149;font:12px Menlo,monospace;white-space:pre-wrap;user-select:text;-webkit-user-select:text";
  pre.textContent = `${e.message}\n${e.filename}:${e.lineno}:${e.colno}\n\n${e.error?.stack ?? ""}`;
  root.append(pre);
});
