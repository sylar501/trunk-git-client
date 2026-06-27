// File list (PRD §8) — left column (196px) of the staging view. Tri-state checkbox per row
// (unchecked/partial/checked, via native <input type=checkbox> + `.indeterminate` for
// "partial" — simplest correct mapping of the spec's three states onto native checkbox
// semantics), M/A/D status badge, +/- stats, click-to-select-for-diff. Mirrors
// commit-overlay.js's `.cdo-files`/`.cdo-file` row pattern with the checkbox/badge added.

const STATUS_BADGE = { added: "A", modified: "M", deleted: "D" };

/** @param {{ onSelect, onToggleFile, onStageAll, onUnstageAll }} handlers -
 *   `onToggleFile(path, shouldStage)` fires on a checkbox click; `onSelect(path)` on a row
 *   click. */
export function createFileList({ onSelect, onToggleFile, onStageAll, onUnstageAll } = {}) {
  const el = document.createElement("div");
  el.className = "sf-list";
  el.innerHTML = `
    <div class="sf-hdr">
      <span>Changes</span>
      <div class="sf-hdr-actions">
        <div class="btn btn-green disabled sf-btn-sm sf-stage-all">Stage all</div>
        <div class="btn btn-amber disabled sf-btn-sm sf-unstage-all">Unstage all</div>
      </div>
    </div>
    <div class="sf-rows"></div>
  `;
  const rowsEl = el.querySelector(".sf-rows");
  const stageAllBtn = el.querySelector(".sf-stage-all");
  const unstageAllBtn = el.querySelector(".sf-unstage-all");
  stageAllBtn.addEventListener("click", () => {
    if (stageAllBtn.classList.contains("disabled")) return;
    onStageAll?.();
  });
  unstageAllBtn.addEventListener("click", () => {
    if (unstageAllBtn.classList.contains("disabled")) return;
    onUnstageAll?.();
  });

  let selectedPath = null;

  /** @param {Array<{path,status,additions,deletions,staged,unstaged}>} files */
  function render(files) {
    rowsEl.innerHTML = "";
    // Nothing left to stage once every file is fully staged (or there are no files at all);
    // mirror logic for unstage, keyed off `staged` instead of `unstaged`.
    stageAllBtn.classList.toggle("disabled", !files.some((f) => f.unstaged));
    unstageAllBtn.classList.toggle("disabled", !files.some((f) => f.staged));
    if (files.length === 0) {
      rowsEl.innerHTML = `<div class="empty-state-hint" style="margin:16px;">No changes.</div>`;
      return;
    }
    for (const file of files) {
      const row = document.createElement("div");
      row.className = "sf-row";
      row.classList.toggle("sel", file.path === selectedPath);
      row.dataset.path = file.path;
      row.innerHTML = `
        <input type="checkbox" class="sf-check" />
        <span class="sf-badge ${file.status}">${STATUS_BADGE[file.status] || "M"}</span>
        <span class="sf-fn"></span>
        <span class="sf-add"></span>
        <span class="sf-del"></span>
      `;
      const checkbox = row.querySelector(".sf-check");
      checkbox.checked = file.staged && !file.unstaged;
      checkbox.indeterminate = file.staged && file.unstaged;
      row.querySelector(".sf-fn").textContent = file.path;
      row.querySelector(".sf-add").textContent = file.additions ? `+${file.additions}` : "";
      row.querySelector(".sf-del").textContent = file.deletions ? `−${file.deletions}` : "";

      checkbox.addEventListener("click", (e) => {
        e.stopPropagation();
        // Native click behaviour already flips `.checked` (and clears `.indeterminate`) before
        // this handler runs — that new boolean is exactly "should this file end up staged".
        onToggleFile?.(file.path, checkbox.checked);
      });
      row.addEventListener("click", () => {
        selectedPath = file.path;
        rowsEl.querySelectorAll(".sf-row").forEach((r) => r.classList.toggle("sel", r.dataset.path === file.path));
        onSelect?.(file.path);
      });
      rowsEl.append(row);
    }
  }

  function getSelectedPath() {
    return selectedPath;
  }

  /** Marks `path` selected without firing `onSelect` — for the initial auto-select on mount,
   *  where the caller drives the first diff load directly instead of going through a callback. */
  function selectPath(path) {
    selectedPath = path;
    rowsEl.querySelectorAll(".sf-row").forEach((r) => r.classList.toggle("sel", r.dataset.path === path));
  }

  return { el, render, getSelectedPath, selectPath };
}
