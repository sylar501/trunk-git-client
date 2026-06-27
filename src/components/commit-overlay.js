// Commit detail overlay (PRD §4.3, SPEC.md item 4): 264px panel, slides in from the right over
// the (unshrunk) graph canvas. Renders commit metadata, a changed-files list, and a read-only
// diff for the selected file. Deliberately owns no global dismiss listeners (no document-level
// click/keydown) — graph-view.js (which already owns the `.crow` click handler) owns Escape and
// outside-click centrally, since a self-attached outside-click listener here would race with
// clicking a *different* commit row and close the overlay before that row's click handler gets
// a chance to just update it in place. This component only wires its own ✕ button and file-row
// clicks, and reports cherry-pick/revert/branch-from-here intent via callbacks — it never shows
// toasts or triggers a refresh itself, graph-view.js orchestrates that centrally.

import { getCommitDetail, getCommitFileDiff } from "../js/app.js";
import { renderDiffLines } from "./diff-line.js";

function formatDate(epochSeconds) {
  const d = new Date(epochSeconds * 1000);
  const now = new Date();
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === now.toDateString()) return `today, ${time}`;
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) return `yesterday, ${time}`;
  return `${d.toLocaleDateString([], { month: "short", day: "numeric" })}, ${time}`;
}

/** @param {{ onCherryPick, onRevert, onBranchFromHere }} handlers - each called with
 *   `{ sha, shortSha, summary, repoPath }`; resolving (or rejecting, it's ignored either way)
 *   just re-enables the action buttons — toasts/refresh are the caller's responsibility. */
export function createCommitOverlay({ onCherryPick, onRevert, onBranchFromHere } = {}) {
  const el = document.createElement("div");
  el.className = "cdo";
  el.innerHTML = `
    <div class="cdo-resize-handle"></div>
    <div class="cdo-hdr">
      <div class="cdo-msg"></div>
      <div class="cdo-x" title="Close">✕</div>
    </div>
    <div class="cdo-meta">
      <div class="cdo-row"><span class="cdo-lbl">Author</span><span class="cdo-val p"></span></div>
      <div class="cdo-row"><span class="cdo-lbl">Date</span><span class="cdo-val p"></span></div>
      <div class="cdo-row"><span class="cdo-lbl">SHA</span><span class="cdo-val"></span></div>
    </div>
    <div class="cdo-sec">Changed files</div>
    <div class="cdo-files"></div>
    <div class="cdo-diff"></div>
    <div class="cdo-actions">
      <div class="cdo-btn" data-action="cherry-pick">cherry-pick</div>
      <div class="cdo-btn" data-action="revert">revert</div>
      <div class="cdo-btn" data-action="branch-from-here">branch from here</div>
      <div class="cdo-btn" data-action="copy-sha">copy SHA</div>
    </div>
  `;

  const msgEl = el.querySelector(".cdo-msg");
  const [authorVal, dateVal, shaVal] = el.querySelectorAll(".cdo-val");
  const filesEl = el.querySelector(".cdo-files");
  const diffEl = el.querySelector(".cdo-diff");
  const actionsEl = el.querySelector(".cdo-actions");

  let currentSha = null;
  let currentRepoPath = null;
  let currentDetail = null;
  let loadGeneration = 0;

  function setButtonsDisabled(disabled) {
    actionsEl.querySelectorAll(".cdo-btn").forEach((b) => b.classList.toggle("disabled", disabled));
  }

  async function showFileDiff(filePath) {
    filesEl.querySelectorAll(".cdo-file").forEach((row) => {
      row.classList.toggle("sel", row.dataset.path === filePath);
    });
    const generation = loadGeneration;
    diffEl.innerHTML = `<div class="loading-row"><div class="spinner"></div><span>Loading diff…</span></div>`;
    try {
      const lines = await getCommitFileDiff(currentRepoPath, currentSha, filePath);
      if (generation !== loadGeneration) return; // a newer open()/file click superseded this
      diffEl.innerHTML = "";
      if (lines.length === 0) {
        diffEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">Binary or empty diff — nothing to show.</div>`;
        return;
      }
      diffEl.append(renderDiffLines(lines));
    } catch (err) {
      if (generation !== loadGeneration) return;
      diffEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">Couldn't load diff: ${String(err)}</div>`;
    }
  }

  function renderFiles(files) {
    filesEl.innerHTML = "";
    for (const file of files) {
      const row = document.createElement("div");
      row.className = "cdo-file";
      row.dataset.path = file.path;
      row.innerHTML = `<span class="cdo-add"></span><span class="cdo-del"></span><span class="cdo-fn"></span>`;
      row.querySelector(".cdo-add").textContent = file.additions ? `+${file.additions}` : "";
      row.querySelector(".cdo-del").textContent = file.deletions ? `−${file.deletions}` : "";
      row.querySelector(".cdo-fn").textContent = file.path;
      row.addEventListener("click", () => showFileDiff(file.path));
      filesEl.append(row);
    }
  }

  /** Opens the overlay for `sha` — or, if it's already open, updates it in place. */
  async function open(sha, repoPath) {
    currentSha = sha;
    currentRepoPath = repoPath;
    el.classList.add("open");

    const generation = ++loadGeneration;
    msgEl.textContent = "";
    authorVal.textContent = "";
    dateVal.textContent = "";
    shaVal.textContent = "";
    filesEl.innerHTML = `<div class="loading-row"><div class="spinner"></div><span>Loading…</span></div>`;
    diffEl.innerHTML = "";

    try {
      const detail = await getCommitDetail(repoPath, sha);
      if (generation !== loadGeneration) return; // a newer open() call superseded this fetch
      currentDetail = detail;
      msgEl.textContent = detail.summary;
      authorVal.textContent = `${detail.author_name} · ${detail.author_email}`;
      dateVal.textContent = formatDate(detail.time);
      shaVal.textContent = detail.short_sha;
      renderFiles(detail.files);
      if (detail.files.length > 0) {
        await showFileDiff(detail.files[0].path);
      } else {
        diffEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">No file changes.</div>`;
      }
    } catch (err) {
      if (generation !== loadGeneration) return;
      filesEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">Couldn't load commit: ${String(err)}</div>`;
    }
  }

  function close() {
    el.classList.remove("open");
    currentSha = null;
    currentRepoPath = null;
    currentDetail = null;
  }

  function isOpen() {
    return currentSha !== null;
  }

  el.querySelector(".cdo-x").addEventListener("click", close);

  actionsEl.addEventListener("click", async (e) => {
    const btn = e.target.closest(".cdo-btn");
    if (!btn || btn.classList.contains("disabled") || !currentSha) return;
    const action = btn.dataset.action;

    if (action === "copy-sha") {
      try {
        await navigator.clipboard.writeText(currentSha);
        const original = btn.textContent;
        btn.textContent = "copied!";
        setTimeout(() => {
          btn.textContent = original;
        }, 1200);
      } catch {
        // Clipboard access denied/unavailable — nothing destructive at stake, fail silently.
      }
      return;
    }

    const handler = { "cherry-pick": onCherryPick, revert: onRevert, "branch-from-here": onBranchFromHere }[action];
    if (!handler) return;
    setButtonsDisabled(true);
    try {
      await handler({ sha: currentSha, shortSha: currentDetail?.short_sha, summary: currentDetail?.summary, repoPath: currentRepoPath });
    } finally {
      setButtonsDisabled(false);
    }
  });

  return { el, open, close, isOpen };
}
