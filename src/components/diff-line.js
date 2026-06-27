// Read-only unified diff line: line-number gutter, +/−/blank sign, code content (PRD §4.3).
// Built for the commit-detail overlay (SPEC.md item 4) — the Staging session (item 5) extends
// this with an interactive staging gutter (●/○, click-to-toggle), not built here.

const KIND_CLASS = { context: "dk", addition: "da", deletion: "dd" };
const KIND_SIGN = { context: " ", addition: "+", deletion: "−" };

/**
 * @param {{ kind: "context"|"addition"|"deletion", old_lineno: number|null,
 *   new_lineno: number|null, content: string }} line
 */
export function createDiffLine(line) {
  const el = document.createElement("div");
  el.className = `dline ${KIND_CLASS[line.kind] || "dk"}`;
  el.innerHTML = `<div class="dln"></div><div class="ds"></div><div class="dc"></div>`;

  // Deleted lines only exist in the old file (no new_lineno); context/additions track the
  // resulting file's numbering, which is also what stays continuous as you read down the diff.
  const lineNo = line.kind === "deletion" ? line.old_lineno : line.new_lineno;
  el.querySelector(".dln").textContent = lineNo ?? "";
  el.querySelector(".ds").textContent = KIND_SIGN[line.kind] || " ";
  el.querySelector(".dc").textContent = line.content;
  return el;
}

/** @param {Array<object>} lines */
export function renderDiffLines(lines) {
  const frag = document.createDocumentFragment();
  for (const line of lines) frag.append(createDiffLine(line));
  return frag;
}
