// Draggable commit-list row for the interactive rebase editing UI (PRD §16.2).
// Each row has an action selector + message area; reword action replaces the message with
// an inline textarea. Drag-and-drop (HTML5 native) reorders the plan array.

const ACTIONS = ["pick", "reword", "squash", "fixup", "edit", "drop"];

const ACTION_COLOR = {
  pick:   "var(--blue)",
  reword: "var(--green)",
  squash: "var(--amber)",
  fixup:  "var(--amber)",
  edit:   "var(--purple)",
  drop:   "var(--red)",
};

const ACTION_BG = {
  pick:   "transparent",
  reword: "rgba(87,171,90,0.06)",
  squash: "rgba(198,144,38,0.07)",
  fixup:  "rgba(198,144,38,0.07)",
  edit:   "rgba(152,110,226,0.07)",
  drop:   "rgba(229,83,75,0.07)",
};

/**
 * Creates a draggable rebase-row element.
 * @param {object} step — the RebaseStep from the plan.
 * @param {number} index — position in plan.steps (oldest-first; 0 = first commit to replay).
 * @param {{ onActionChange, onMessageChange, onReorder }} callbacks
 * @returns {HTMLElement}
 */
export function renderRebaseRow(step, index, { onActionChange, onMessageChange, onReorder }) {
  const row = document.createElement("div");
  row.className = "rb-row";
  row.draggable = true;
  row.dataset.index = index;
  row.style.cssText = `
    display:flex;align-items:center;gap:8px;padding:5px 10px;
    border-bottom:1px solid var(--border-subtle);cursor:grab;
    background:${ACTION_BG[step.action]};
    transition:background 0.1s;
  `;

  const actionColor = ACTION_COLOR[step.action] || "var(--text-secondary)";
  const isReword = step.action === "reword";
  const isDrop = step.action === "drop";

  row.innerHTML = `
    <div class="rb-row-drag" title="Drag to reorder" style="color:var(--text-tertiary);cursor:grab;user-select:none;font-size:14px;">⠿</div>
    <select class="rb-action-select inp" style="
      color:${actionColor};padding:2px 4px;font-size:11px;cursor:pointer;min-width:72px;
    ">
      ${ACTIONS.map((a) => {
        const invalid = index === 0 && (a === "squash" || a === "fixup");
        return `<option value="${a}" ${a === step.action ? "selected" : ""} ${invalid ? "disabled" : ""}>${a}</option>`;
      }).join("")}
    </select>
    <span class="rb-row-sha" style="font-family:monospace;font-size:11px;color:var(--text-secondary);flex-shrink:0;">${step.short_sha}</span>
    ${
      isReword
        ? `<textarea class="rb-row-msg-input inp" rows="1" style="flex:1;font-size:12px;padding:2px 4px;resize:none;overflow:hidden;">${step.new_message ?? step.summary}</textarea>`
        : `<span class="rb-row-msg" style="flex:1;font-size:12px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;${isDrop ? "opacity:0.4;text-decoration:line-through;" : ""}">${step.summary}</span>`
    }
    <span class="rb-row-author" style="font-size:11px;color:var(--text-secondary);flex-shrink:0;max-width:80px;overflow:hidden;text-overflow:ellipsis;">${step.author_name}</span>
  `;

  // Action change.
  row.querySelector(".rb-action-select").addEventListener("change", (e) => {
    onActionChange(e.target.value);
  });

  // Reword textarea: auto-grow and notify on change.
  const textarea = row.querySelector(".rb-row-msg-input");
  if (textarea) {
    // Disable row dragging while the textarea has focus so the browser can
    // handle mousedown → drag as native text selection rather than a DnD move.
    textarea.addEventListener("mousedown", () => { row.draggable = false; });
    textarea.addEventListener("mouseup",   () => { row.draggable = true; });
    textarea.addEventListener("blur",      () => { row.draggable = true; });

    textarea.addEventListener("input", () => {
      textarea.style.height = "auto";
      textarea.style.height = textarea.scrollHeight + "px";
      onMessageChange(textarea.value);
    });
    requestAnimationFrame(() => {
      textarea.style.height = "auto";
      textarea.style.height = textarea.scrollHeight + "px";
    });
  }

  // Native HTML5 drag-and-drop reordering.
  let dragOverIndex = null;

  row.addEventListener("dragstart", (e) => {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(index));
    row.style.opacity = "0.4";
  });
  row.addEventListener("dragend", () => {
    row.style.opacity = "";
    clearDropIndicators(row.parentElement);
  });
  row.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    dragOverIndex = index;
    clearDropIndicators(row.parentElement);
    const half = row.getBoundingClientRect().height / 2;
    const above = e.clientY - row.getBoundingClientRect().top < half;
    row.style.borderTop = above ? "2px solid var(--blue)" : "";
    row.style.borderBottom = above ? "" : "2px solid var(--blue)";
  });
  row.addEventListener("dragleave", () => {
    row.style.borderTop = "";
    row.style.borderBottom = "";
  });
  row.addEventListener("drop", (e) => {
    e.preventDefault();
    const fromIndex = Number(e.dataTransfer.getData("text/plain"));
    const half = row.getBoundingClientRect().height / 2;
    const above = e.clientY - row.getBoundingClientRect().top < half;
    let toIndex = above ? index : index + 1;
    if (fromIndex < toIndex) toIndex--;
    onReorder(fromIndex, toIndex);
  });

  return row;
}

function clearDropIndicators(parent) {
  if (!parent) return;
  parent.querySelectorAll(".rb-row").forEach((r) => {
    r.style.borderTop = "";
    r.style.borderBottom = "";
  });
}
