// Unified diff line: line-number gutter, +/−/blank sign, code content (PRD §4.3). Built for the
// commit-detail overlay (SPEC.md item 4) as read-only; the Staging session (item 5) extends it
// with an optional interactive staging gutter (●/○, click-to-toggle) via the `staging` option —
// omitted entirely (the default), `createDiffLine`/`renderDiffLines` render exactly as before,
// so commit-overlay.js's existing read-only usage is unaffected.

const KIND_CLASS = { context: "dk", addition: "da", deletion: "dd" };
const KIND_SIGN = { context: " ", addition: "+", deletion: "−" };

/**
 * @param {{ kind: "context"|"addition"|"deletion", old_lineno: number|null,
 *   new_lineno: number|null, content: string }} line
 * @param {{ staging?: { staged: boolean, onToggle: () => void } }} [opts] - when `staging` is
 *   present and `line.kind` isn't `"context"`, renders a clickable ●/○ gutter dot reflecting
 *   `staged`, calling `onToggle()` on click. Context lines never get a dot — only +/- content is
 *   stageable (PRD §8).
 */
export function createDiffLine(line, opts) {
  const el = document.createElement("div");
  el.className = `dline ${KIND_CLASS[line.kind] || "dk"}`;
  const stageable = opts?.staging && line.kind !== "context";
  el.innerHTML = `${stageable ? '<div class="dstage"></div>' : ""}<div class="dln"></div><div class="ds"></div><div class="dc"></div>`;

  // Deleted lines only exist in the old file (no new_lineno); context/additions track the
  // resulting file's numbering, which is also what stays continuous as you read down the diff.
  const lineNo = line.kind === "deletion" ? line.old_lineno : line.new_lineno;
  el.querySelector(".dln").textContent = lineNo ?? "";
  el.querySelector(".ds").textContent = KIND_SIGN[line.kind] || " ";
  el.querySelector(".dc").textContent = line.content;

  if (stageable) {
    const dot = el.querySelector(".dstage");
    dot.textContent = opts.staging.staged ? "●" : "○";
    dot.classList.toggle("staged", opts.staging.staged);
    dot.addEventListener("click", () => opts.staging.onToggle());
  }
  return el;
}

/**
 * @param {Array<object>} lines
 * @param {object} [opts] - forwarded to every `createDiffLine` call; see its doc comment.
 */
export function renderDiffLines(lines, opts) {
  const frag = document.createDocumentFragment();
  for (const line of lines) frag.append(createDiffLine(line, opts));
  return frag;
}
