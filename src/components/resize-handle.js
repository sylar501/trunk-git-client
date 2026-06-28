// Shared drag-to-resize mechanics for the sidebar, commit-detail-overlay, staging-files, and
// conflict-resolver merged-result-panel handles — all are "drag a thin strip, resize a panel,
// clamp, persist on release," differing only in which axis the drag reads from (`axis`) and
// which edge the handle sits on (`invert`). Persisting only happens once per completed drag
// (`onResizeEnd`, fired on mouseup), not on every mousemove.

/**
 * @param {HTMLElement} handleEl
 * @param {{
 *   getWidth: () => number, setWidth: (w: number) => void - named for the common (horizontal)
 *     case; with `axis: "y"` these get/set a height instead, the mechanics are identical.
 *   min: number, max: number,
 *   axis?: "x"|"y" - "x" (default) reads `clientX` for a vertical strip between two
 *     side-by-side panels; "y" reads `clientY` for a horizontal strip between two stacked
 *     panels (e.g. the conflict resolver's merged-result panel, anchored above its handle).
 *   invert?: boolean - true when dragging toward the *start* of the axis (left for x, up for y)
 *     should grow the panel (a handle on that edge, e.g. the commit overlay's left edge or the
 *     merged-result panel's top edge) instead of growing when dragging toward the end.
 *   onResizeEnd?: (finalWidth: number) => void,
 *   signal?: AbortSignal
 * }} opts
 */
export function attachResizeHandle(handleEl, { getWidth, setWidth, min, max, axis = "x", invert = false, onResizeEnd, signal } = {}) {
  handleEl.addEventListener(
    "mousedown",
    (e) => {
      e.preventDefault();
      const startPos = axis === "y" ? e.clientY : e.clientX;
      const startWidth = getWidth();
      let currentWidth = startWidth;
      handleEl.classList.add("active");

      function onMouseMove(ev) {
        const pos = axis === "y" ? ev.clientY : ev.clientX;
        const delta = invert ? startPos - pos : pos - startPos;
        currentWidth = Math.max(min, Math.min(max, startWidth + delta));
        setWidth(currentWidth);
      }

      function onMouseUp() {
        document.removeEventListener("mousemove", onMouseMove);
        document.removeEventListener("mouseup", onMouseUp);
        handleEl.classList.remove("active");
        onResizeEnd?.(currentWidth);
      }

      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
    },
    { signal }
  );
}
