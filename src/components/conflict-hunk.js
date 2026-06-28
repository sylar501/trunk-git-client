// One conflict segment rendered across the three-panel editor (PRD §9.2/§9.3, SPEC.md item 6).
// A "hunk" here is backend-supplied `{ kind: "conflict", ours, base, theirs }` — three arrays of
// plain text lines, no line numbers (unlike diff-line.js's unified-diff lines, which always
// have one side's line number; a three-way conflict has no single shared numbering to show).
// Renders the same hunk's ours/base/theirs blocks into three different parent panels (passed in
// as `{ oursEl, baseEl, theirsEl }`) so the caller controls layout and synced scrolling — this
// component only owns one hunk's worth of DOM across those three columns plus its action row.

function linesHtml(lines) {
  return lines.map((l) => `<div class="cfh-line">${l.length ? "" : "&nbsp;"}</div>`).join("");
}

function setLinesText(container, lines) {
  const rows = container.querySelectorAll(".cfh-line");
  rows.forEach((row, i) => {
    row.textContent = lines[i] ?? "";
  });
}

/**
 * @param {{ ours: string[], base: string[], theirs: string[] }} segment
 * @param {{ oursEl: HTMLElement, baseEl: HTMLElement, theirsEl: HTMLElement }} panels - parent
 *   elements (one per editor column) this hunk's three blocks are appended into.
 * @param {{
 *   choice: "ours"|"theirs"|"both"|null,
 *   onChoose: (choice: "ours"|"theirs"|"both") => void,
 *   onEditManually: () => void,
 *   onUndo: () => void,
 * }} opts
 */
export function createConflictHunk(segment, panels, opts) {
  const oursBlock = document.createElement("div");
  oursBlock.className = "cfh-block cfh-ours";
  const baseBlock = document.createElement("div");
  baseBlock.className = "cfh-block cfh-base";
  const theirsBlock = document.createElement("div");
  theirsBlock.className = "cfh-block cfh-theirs";

  oursBlock.innerHTML = linesHtml(segment.ours);
  baseBlock.innerHTML = linesHtml(segment.base);
  theirsBlock.innerHTML = linesHtml(segment.theirs);
  setLinesText(oursBlock, segment.ours);
  setLinesText(baseBlock, segment.base);
  setLinesText(theirsBlock, segment.theirs);

  const actions = document.createElement("div");
  actions.className = "cfh-actions";
  actions.innerHTML = `
    <div class="btn btn-green cfh-act" data-choice="ours">accept ours</div>
    <div class="btn btn-blue cfh-act" data-choice="theirs">accept theirs</div>
    <div class="btn btn-amber cfh-act" data-choice="both">accept both</div>
    <div class="btn btn-neutral cfh-act" data-choice="manual">edit manually</div>
  `;
  actions.querySelectorAll(".cfh-act").forEach((btn) => {
    btn.addEventListener("click", () => {
      const choice = btn.dataset.choice;
      if (choice === "manual") opts.onEditManually();
      else opts.onChoose(choice);
    });
  });
  theirsBlock.append(actions);

  // One bar per panel — `ours`/`theirs` get an undo button, `base` doesn't (it's reference-only,
  // no action buttons per PRD §9.3, so no undo either).
  function makeResolvedBar(withUndo) {
    const bar = document.createElement("div");
    bar.className = "cfh-resolved-bar";
    bar.innerHTML = `<span class="cfh-resolved-text"></span>${withUndo ? '<div class="btn btn-neutral cfh-undo">undo</div>' : ""}`;
    if (withUndo) bar.querySelector(".cfh-undo").addEventListener("click", () => opts.onUndo());
    return bar;
  }
  const oursBar = makeResolvedBar(true);
  const baseBar = makeResolvedBar(false);
  const theirsBar = makeResolvedBar(true);
  oursBlock.append(oursBar);
  baseBlock.append(baseBar);
  theirsBlock.append(theirsBar);

  function applyChoiceState(choice) {
    oursBlock.classList.toggle("cfh-resolved", !!choice);
    baseBlock.classList.toggle("cfh-resolved", !!choice);
    theirsBlock.classList.toggle("cfh-resolved", !!choice);
    actions.hidden = !!choice;
    const label = { ours: "ours", theirs: "theirs", both: "both" }[choice] || "";
    for (const bar of [oursBar, baseBar, theirsBar]) {
      bar.querySelector(".cfh-resolved-text").textContent = `✓ accepted ${label}`;
    }
  }
  applyChoiceState(opts.choice);

  panels.oursEl.append(oursBlock);
  panels.baseEl.append(baseBlock);
  panels.theirsEl.append(theirsBlock);

  return {
    setChoice: applyChoiceState,
  };
}
