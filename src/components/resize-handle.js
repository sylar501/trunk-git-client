// Shared drag-to-resize mechanics for the sidebar and commit-detail-overlay width handles —
// both are "drag a thin strip, resize a panel, clamp, persist on release," differing only in
// which edge the handle sits on (hence `invert`). Persisting only happens once per completed
// drag (`onResizeEnd`, fired on mouseup), not on every mousemove.

/**
 * @param {HTMLElement} handleEl
 * @param {{
 *   getWidth: () => number, setWidth: (w: number) => void,
 *   min: number, max: number,
 *   invert?: boolean - true when dragging *left* should grow the panel (a handle on its left
 *     edge, e.g. the commit overlay) instead of dragging *right* (a handle on its right edge,
 *     e.g. the sidebar).
 *   onResizeEnd?: (finalWidth: number) => void,
 *   signal?: AbortSignal
 * }} opts
 */
export function attachResizeHandle(handleEl, { getWidth, setWidth, min, max, invert = false, onResizeEnd, signal } = {}) {
  handleEl.addEventListener(
    "mousedown",
    (e) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = getWidth();
      let currentWidth = startWidth;
      handleEl.classList.add("active");

      function onMouseMove(ev) {
        const delta = invert ? startX - ev.clientX : ev.clientX - startX;
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
