// Staging view controller (PRD §4.4, §8, SPEC.md item 5) — three fixed-width columns: file
// list (196px, left) / hunk diff (centre) / commit panel (214px, right). Fixed widths, no
// drag-resize — the acceptance criteria literally specifies "(196px)"/"(214px)" with no mention
// of resizability, unlike the commit overlay. Manual entry only (toolbar button/⌘⇧S in
// graph-view.js) — never auto-shown.

import {
  getWorkingTreeStatus,
  getWorkingFileDiff,
  stageFile,
  unstageFile,
  stageHunk,
  unstageHunk,
  stageLine,
  unstageLine,
  getLastCommitMessage,
  commitChanges,
} from "./app.js";
import { createFileList } from "../components/file-list.js";
import { createCommitPanel } from "../components/commit-panel.js";
import { createDiffLine } from "../components/diff-line.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const FILES_MIN_WIDTH = 140;
const FILES_MAX_WIDTH = 400;

/**
 * @param {HTMLElement} root @param {string} repoPath
 * @param {{
 *   onExit: () => void,
 *   filesWidth?: number - persisted file-list column width (px), defaults to 196 if not
 *     supplied,
 *   onFilesResize?: (width: number) => void - fired once per completed drag (not per
 *     mousemove), same convention as the sidebar/commit-overlay resize handles (see SPEC.md's
 *     "Resizable panels" note).
 * }} opts
 */
export async function mountStaging(root, repoPath, { onExit, filesWidth: initialFilesWidth, onFilesResize } = {}) {
  // `mountStaging` can be remounted on the same root — abort the previous call's document-level
  // Escape listener before attaching a new one (same convention as graph-view.js's `mountGraph`).
  root._stgAbortController?.abort();
  const abortController = new AbortController();
  root._stgAbortController = abortController;
  const { signal } = abortController;

  root.innerHTML = `
    <div class="stg-layout">
      <div class="stg-topbar">
        <div class="btn btn-neutral stg-exit">← history <span class="kbd-badge">Esc</span></div>
      </div>
      <div class="stg-columns">
        <div class="stg-files-col"></div>
        <div class="resize-handle" id="stg-files-resize"></div>
        <div class="stg-diff-col">
          <div class="stg-diff-hdr"><span class="stg-diff-fn"></span></div>
          <div class="stg-diff-body"></div>
        </div>
        <div class="stg-panel-col"></div>
      </div>
    </div>
  `;

  const diffFnEl = root.querySelector(".stg-diff-fn");
  const diffBodyEl = root.querySelector(".stg-diff-body");
  const filesColEl = root.querySelector(".stg-files-col");
  const panelColEl = root.querySelector(".stg-panel-col");

  // Drag-to-resize file-list column (SPEC.md's "Resizable panels" note) — same mechanics as the
  // sidebar: a flex-sibling `.resize-handle` strip on the panel's right edge, width set via JS
  // inline style (not a CSS literal), persisted only once per completed drag.
  let filesWidth = initialFilesWidth || 196;
  filesColEl.style.width = `${filesWidth}px`;
  attachResizeHandle(root.querySelector("#stg-files-resize"), {
    getWidth: () => filesWidth,
    setWidth: (w) => {
      filesWidth = w;
      filesColEl.style.width = `${w}px`;
    },
    min: FILES_MIN_WIDTH,
    max: FILES_MAX_WIDTH,
    onResizeEnd: (finalWidth) => onFilesResize?.(finalWidth),
    signal,
  });

  let status = null;
  let selectedPath = null;
  let loadGeneration = 0;

  async function refreshStatus() {
    status = await getWorkingTreeStatus(repoPath);
    fileList.render(status.files);
    const stagedCount = status.files.filter((f) => f.staged).length;
    const additions = status.files.reduce((sum, f) => sum + f.additions, 0);
    const deletions = status.files.reduce((sum, f) => sum + f.deletions, 0);
    commitPanel.setStats({ stagedCount, additions, deletions });
    commitPanel.setBranch(status.branch_name);
    commitPanel.setAuthor(status.author_name, status.author_email);
    commitPanel.setCanAmend(status.can_amend);
    commitPanel.setHasSigningKey(status.has_signing_key);
  }

  // Hunk/line-level stage controls are hidden for whole-file adds/deletes — the diff still
  // renders read-only for context, but only the file-list checkbox can (un)stage these. Two
  // reasons: conceptually a brand-new/fully-removed file isn't "a diff with hunks", it's one
  // indivisible event; and concretely, `stage_hunk`/`stage_line`'s hand-built patch buffer
  // (`build_partial_hunk_patch` in git/mod.rs) doesn't emit the `/dev/null` + file-mode headers
  // a real add/delete patch needs, so applying a partial hunk to a deleted file's diff left a
  // zero-byte blob staged at that path instead of actually removing it from the index (visible
  // as a stuck indeterminate checkbox) — whole-file `stage_file`/`unstage_file` already handles
  // both cases correctly via `index.add_path`/`reset_default`, so this sidesteps the bug rather
  // than fixing the patch builder for a rarely-used capability.
  function renderFileDiff(fileDiff, fileStatus) {
    diffBodyEl.innerHTML = "";
    const wholeFileOnly = fileStatus === "added" || fileStatus === "deleted";
    if (fileDiff.is_binary) {
      diffBodyEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">Binary file — no inline diff.</div>`;
      return;
    }
    if (fileDiff.hunks.length === 0) {
      diffBodyEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">No changes.</div>`;
      return;
    }
    for (const hunk of fileDiff.hunks) {
      const hunkEl = document.createElement("div");
      hunkEl.className = "shunk";
      hunkEl.innerHTML = `
        <div class="shunk-hdr">
          <span class="shunk-text"></span>
          ${wholeFileOnly ? "" : '<div class="btn shunk-btn"></div>'}
        </div>
        <div class="shunk-lines"></div>
      `;
      hunkEl.querySelector(".shunk-text").textContent = hunk.header;

      if (!wholeFileOnly) {
        const btn = hunkEl.querySelector(".shunk-btn");
        btn.classList.add(hunk.fully_staged ? "btn-amber" : "btn-green");
        btn.textContent = hunk.fully_staged ? "unstage hunk" : "stage hunk";
        btn.addEventListener("click", async () => {
          try {
            // `unstage_hunk` locates the hunk via `old_start` (HEAD-relative — shared with the
            // HEAD-vs-index diff it actually operates on); `stage_hunk` via `new_start`
            // (workdir-relative — shared with the index-vs-workdir diff it operates on). Using
            // the wrong one is exactly what caused the "hunk did not apply" bug once an earlier
            // hunk in the same file had already been staged.
            if (hunk.fully_staged) {
              await unstageHunk(repoPath, fileDiff.path, hunk.old_start);
            } else {
              await stageHunk(repoPath, fileDiff.path, hunk.new_start);
            }
            await refreshStatus();
            await loadDiff(fileDiff.path);
          } catch (err) {
            showToast({ variant: "danger", message: String(err) });
          }
        });
      }

      const linesEl = hunkEl.querySelector(".shunk-lines");
      for (const line of hunk.lines) {
        const lineEl = wholeFileOnly
          ? createDiffLine(line)
          : createDiffLine(line, {
              staging: {
                staged: line.staged === "staged",
                onToggle: async () => {
                  try {
                    if (line.staged === "staged") {
                      await unstageLine(repoPath, fileDiff.path, hunk.old_start, line.line_index_in_hunk);
                    } else {
                      await stageLine(repoPath, fileDiff.path, hunk.new_start, line.line_index_in_hunk);
                    }
                    await refreshStatus();
                    await loadDiff(fileDiff.path);
                  } catch (err) {
                    showToast({ variant: "danger", message: String(err) });
                  }
                },
              },
            });
        linesEl.append(lineEl);
      }
      diffBodyEl.append(hunkEl);
    }
  }

  async function loadDiff(path) {
    const generation = ++loadGeneration;
    diffFnEl.textContent = path;
    diffBodyEl.innerHTML = `<div class="loading-row"><div class="spinner"></div><span>Loading diff…</span></div>`;
    try {
      const fileDiff = await getWorkingFileDiff(repoPath, path);
      if (generation !== loadGeneration) return; // a newer selection superseded this fetch
      const fileStatus = status?.files.find((f) => f.path === path)?.status;
      renderFileDiff(fileDiff, fileStatus);
    } catch (err) {
      if (generation !== loadGeneration) return;
      diffBodyEl.innerHTML = `<div class="empty-state-hint" style="margin:12px;">Couldn't load diff: ${String(err)}</div>`;
    }
  }

  const fileList = createFileList({
    onSelect: async (path) => {
      selectedPath = path;
      await loadDiff(path);
    },
    onToggleFile: async (path, shouldStage) => {
      try {
        if (shouldStage) await stageFile(repoPath, path);
        else await unstageFile(repoPath, path);
        await refreshStatus();
        if (selectedPath === path) await loadDiff(path);
      } catch (err) {
        showToast({ variant: "danger", message: String(err) });
      }
    },
    onStageAll: async () => {
      try {
        for (const file of status?.files.filter((f) => f.unstaged) ?? []) {
          await stageFile(repoPath, file.path);
        }
        await refreshStatus();
        if (selectedPath) await loadDiff(selectedPath);
      } catch (err) {
        showToast({ variant: "danger", message: String(err) });
      }
    },
    onUnstageAll: async () => {
      try {
        for (const file of status?.files.filter((f) => f.staged) ?? []) {
          await unstageFile(repoPath, file.path);
        }
        await refreshStatus();
        if (selectedPath) await loadDiff(selectedPath);
      } catch (err) {
        showToast({ variant: "danger", message: String(err) });
      }
    },
  });
  filesColEl.append(fileList.el);

  const commitPanel = createCommitPanel({
    onAmendToggle: async (turningOn) => (turningOn ? getLastCommitMessage(repoPath) : null),
    onCommit: async ({ message, amend, sshSign }) => {
      if (!message.trim()) {
        showToast({ variant: "danger", message: "Enter a commit message." });
        return;
      }
      commitPanel.setBusy(true);
      try {
        await commitChanges(repoPath, message, amend, sshSign);
        showToast({ variant: "success", message: amend ? "Commit amended." : "Committed." });
        commitPanel.setMessage("");
        await refreshStatus();
        if (selectedPath) await loadDiff(selectedPath);
      } catch (err) {
        showToast({ variant: "danger", message: String(err) });
      } finally {
        commitPanel.setBusy(false);
      }
    },
  });
  panelColEl.append(commitPanel.el);

  function exit() {
    showToast({ variant: "info", message: "Returned to history." });
    onExit?.();
  }

  root.querySelector(".stg-exit").addEventListener("click", exit);

  // Two-stage Escape (PRD §8): first Escape just defocuses an active input/textarea; second
  // (or first, when nothing's focused) exits to the graph. Local listener, not a central
  // registry — consistent with how every other view in this codebase wires its own Escape.
  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "Escape") return;
      const active = document.activeElement;
      if (active && (active.tagName === "TEXTAREA" || active.tagName === "INPUT")) {
        active.blur();
        return;
      }
      exit();
    },
    { signal }
  );

  await refreshStatus();
  if (status.files.length > 0) {
    selectedPath = status.files[0].path;
    fileList.selectPath(selectedPath);
    await loadDiff(selectedPath);
  } else {
    diffBodyEl.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;text-align:center;">No changes.</div>`;
  }
}
