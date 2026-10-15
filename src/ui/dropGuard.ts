/** Stops files dropped where nothing takes them from opening in the webview in place of the
 * app. Drops that something takes are handled before they get here. */
export function installDropGuard() {
  const files = (e: DragEvent) => e.dataTransfer?.types.includes("Files");
  window.addEventListener("dragover", (e) => {
    if (!files(e) || e.defaultPrevented) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
  });
  window.addEventListener("drop", (e) => {
    if (files(e)) e.preventDefault();
  });
}
