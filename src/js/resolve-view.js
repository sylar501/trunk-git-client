// Conflict resolver controller (PRD §4.6/§9, SPEC.md item 6) — full-screen takeover, same
// hard-page-load shape as staging-view.js. Resolution state (which hunks are accepted, manual
// edits) lives entirely in this module's per-file state, never the git index, so every choice
// is freely undoable until Continue — see the plan's "resolution model is frontend-held"
// rationale. Only two backend calls mutate anything: `finishConflictResolution` (Continue) and
// `abortConflictResolution` (Abort/Escape).

import { getConflictStatus, getConflictFile, finishConflictResolution, abortConflictResolution } from "./app.js";
import { createConflictHunk } from "../components/conflict-hunk.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const MARKER_RE = /^(<<<<<<<|\|\|\|\|\|\|\||=======|>>>>>>>)/m;
const MERGED_MIN_HEIGHT = 120;
const MERGED_MAX_HEIGHT = 480;

/**
 * One file's resolution state.
 * - `hunkChoices[i]` mirrors `segments[i]` for `kind: "conflict"` entries — `null` (unresolved),
 *   `"ours"`, `"theirs"`, or `"both"`.
 * - `fileMode` is "composed" (merged-result panel is auto-generated from `hunkChoices`) or
 *   "manual" (the user has typed a free-form override via "Edit manually" / the merged panel's
 *   edit mode). Undoing any hunk discards the manual override and drops back to "composed"
 *   (PRD §9.4's explicit callout).
 * - `panelView` ("preview"|"edit") is purely the merged-result panel's own display toggle —
 *   independent of `fileMode`; "Done editing" only flips this back to "preview", it does not
 *   discard the manual text (only hunk-undo does that).
 */
function newFileState() {
  return { segments: null, hunkChoices: [], fileMode: "composed", manualText: null, panelView: "preview" };
}

function composeText(fs) {
  return fs.segments
    .map((seg, i) => {
      if (seg.kind === "context") return seg.lines.join("\n");
      const choice = fs.hunkChoices[i];
      if (choice === "ours") return seg.ours.join("\n");
      if (choice === "theirs") return seg.theirs.join("\n");
      if (choice === "both") return [...seg.ours, ...seg.theirs].join("\n");
      return [
        "<<<<<<< ours",
        ...seg.ours,
        "||||||| base",
        ...seg.base,
        "=======",
        ...seg.theirs,
        ">>>>>>> theirs",
      ].join("\n");
    })
    .join("\n");
}

function finalText(fs) {
  return fs.fileMode === "manual" ? fs.manualText : composeText(fs);
}

function isResolved(fs) {
  if (!fs.segments) return false;
  if (fs.fileMode === "manual") return !MARKER_RE.test(fs.manualText ?? "");
  return fs.segments.every((seg, i) => seg.kind !== "conflict" || fs.hunkChoices[i]);
}

/**
 * @param {HTMLElement} root @param {string} repoPath
 * @param {{
 *   onDone: () => void - fires after a successful Continue or Abort — both return to the graph
 *     view (resolve-page.js's `exitToGraph`).
 *   mergedHeight?: number - persisted merged-result-panel height (px) from `getSettings()`,
 *     defaults to 220 if not supplied.
 *   onMergedResize?: (height: number) => void - fired once per completed drag (not per
 *     mousemove), same convention as the sidebar/commit-overlay/staging-files resize handles.
 * }} opts
 */
export async function mountConflictResolver(root, repoPath, { onDone, mergedHeight: initialMergedHeight, onMergedResize } = {}) {
  root._rsvAbortController?.abort();
  const abortController = new AbortController();
  root._rsvAbortController = abortController;
  const { signal } = abortController;

  root.innerHTML = `
    <div class="rsv-layout">
      <div class="rsv-banner">
        <span class="rsv-banner-text"></span>
        <span class="rsv-banner-progress"></span>
        <div class="tb-spacer"></div>
        <div class="btn btn-red rsv-abort">Abort</div>
      </div>
      <div class="rsv-tabs"></div>
      <div class="rsv-editor">
        <div class="rsv-panel rsv-ours">
          <div class="rsv-panel-label">ours</div>
          <div class="rsv-panel-body"></div>
        </div>
        <div class="rsv-panel rsv-base">
          <div class="rsv-panel-label">base</div>
          <div class="rsv-panel-body"></div>
        </div>
        <div class="rsv-panel rsv-theirs">
          <div class="rsv-panel-label">theirs</div>
          <div class="rsv-panel-body"></div>
        </div>
      </div>
      <div class="resize-handle resize-handle-h" id="rsv-merged-resize"></div>
      <div class="rsv-merged">
        <div class="rsv-merged-hdr">
          <span class="rsv-merged-title">Merged result</span>
          <div class="tb-spacer"></div>
          <div class="btn btn-neutral rsv-edit-toggle">Edit manually</div>
          <div class="btn btn-amber rsv-done-editing" hidden>Done editing</div>
        </div>
        <div class="rsv-edit-banner" hidden>edit mode active</div>
        <div class="rsv-merged-preview"></div>
        <textarea class="rsv-merged-textarea" hidden></textarea>
      </div>
      <div class="rsv-footer">
        <div class="btn btn-blue disabled rsv-continue">Continue</div>
      </div>
    </div>
  `;

  const mergedEl = root.querySelector(".rsv-merged");

  // Drag-to-resize the merged-result panel (SPEC.md's "Resizable panels" note) — a flex-sibling
  // `.resize-handle` strip on the panel's top edge, height set via JS inline style (not a CSS
  // literal), persisted only once per completed drag. `invert: true` because the handle sits on
  // the panel's *top* edge — dragging up (toward the start of the Y axis) should grow it.
  let mergedHeight = initialMergedHeight || 220;
  mergedEl.style.height = `${mergedHeight}px`;
  attachResizeHandle(root.querySelector("#rsv-merged-resize"), {
    getWidth: () => mergedHeight,
    setWidth: (h) => {
      mergedHeight = h;
      mergedEl.style.height = `${h}px`;
    },
    min: MERGED_MIN_HEIGHT,
    max: MERGED_MAX_HEIGHT,
    axis: "y",
    invert: true,
    onResizeEnd: (finalHeight) => onMergedResize?.(finalHeight),
    signal,
  });

  const bannerTextEl = root.querySelector(".rsv-banner-text");
  const bannerProgressEl = root.querySelector(".rsv-banner-progress");
  const tabsEl = root.querySelector(".rsv-tabs");
  const oursBodyEl = root.querySelector(".rsv-ours .rsv-panel-body");
  const baseBodyEl = root.querySelector(".rsv-base .rsv-panel-body");
  const theirsBodyEl = root.querySelector(".rsv-theirs .rsv-panel-body");
  const mergedPreviewEl = root.querySelector(".rsv-merged-preview");
  const mergedTextareaEl = root.querySelector(".rsv-merged-textarea");
  const editBannerEl = root.querySelector(".rsv-edit-banner");
  const editToggleBtn = root.querySelector(".rsv-edit-toggle");
  const doneEditingBtn = root.querySelector(".rsv-done-editing");
  const continueBtn = root.querySelector(".rsv-continue");

  let session = null;
  const fileStates = new Map();
  let selectedPath = null;

  async function loadSession() {
    session = await getConflictStatus(repoPath);
    if (!session) {
      showToast({ variant: "info", message: "No conflict in progress." });
      onDone?.();
      return false;
    }
    bannerTextEl.textContent = `${session.operation} in progress`;
    for (const path of session.files) {
      if (!fileStates.has(path)) fileStates.set(path, newFileState());
    }
    return true;
  }

  function renderProgress() {
    const total = session.files.length;
    const resolved = session.files.filter((p) => isResolved(fileStates.get(p))).length;
    bannerProgressEl.textContent = `${resolved} of ${total} resolved`;
    continueBtn.classList.toggle("disabled", resolved !== total);
  }

  function renderTabs() {
    tabsEl.innerHTML = "";
    for (const path of session.files) {
      const tab = document.createElement("div");
      tab.className = `rsv-tab${path === selectedPath ? " active" : ""}`;
      const resolved = isResolved(fileStates.get(path));
      tab.innerHTML = `<span class="rsv-tab-dot ${resolved ? "resolved" : "unresolved"}"></span><span class="rsv-tab-name"></span>`;
      tab.querySelector(".rsv-tab-name").textContent = path;
      tab.addEventListener("click", () => selectFile(path));
      tabsEl.append(tab);
    }
  }

  function renderMergedPanel(fs) {
    const isManual = fs.fileMode === "manual";
    doneEditingBtn.hidden = fs.panelView !== "edit";
    editToggleBtn.hidden = fs.panelView === "edit";
    editBannerEl.hidden = fs.panelView !== "edit";
    mergedTextareaEl.hidden = fs.panelView !== "edit";
    mergedPreviewEl.hidden = fs.panelView === "edit";

    if (fs.panelView === "edit") {
      mergedTextareaEl.value = isManual ? fs.manualText : composeText(fs);
      return;
    }
    const text = finalText(fs);
    mergedPreviewEl.textContent = text;
  }

  function enterManualMode(fs) {
    if (fs.fileMode !== "manual") {
      fs.fileMode = "manual";
      fs.manualText = composeText(fs);
    }
    fs.panelView = "edit";
    renderMergedPanel(fs);
  }

  // Any per-hunk action (accept ours/theirs/both, or undo) hands control back to the composed
  // view, discarding a manual override if one was active — both are "the user is using the
  // structured per-hunk UI now," not just undo. Without this, accepting a hunk while manual
  // mode was active silently updated `hunkChoices` but left the merged-result panel showing the
  // stale `manualText` snapshot until an undo happened to reset `fileMode` first.
  function exitManualMode(fs) {
    if (fs.fileMode !== "manual") return;
    fs.fileMode = "composed";
    fs.manualText = null;
    fs.panelView = "preview";
  }

  function undoHunk(fs, index) {
    fs.hunkChoices[index] = null;
    exitManualMode(fs);
    renderFileEditor(selectedPath);
  }

  function renderFileEditor(path) {
    const fs = fileStates.get(path);
    oursBodyEl.innerHTML = "";
    baseBodyEl.innerHTML = "";
    theirsBodyEl.innerHTML = "";

    fs.segments.forEach((seg, i) => {
      if (seg.kind === "context") {
        const text = seg.lines.join("\n");
        for (const el of [oursBodyEl, baseBodyEl, theirsBodyEl]) {
          const block = document.createElement("div");
          block.className = "cfh-context";
          block.textContent = text;
          el.append(block);
        }
        return;
      }
      createConflictHunk(
        seg,
        { oursEl: oursBodyEl, baseEl: baseBodyEl, theirsEl: theirsBodyEl },
        {
          choice: fs.hunkChoices[i],
          onChoose: (choice) => {
            fs.hunkChoices[i] = choice;
            exitManualMode(fs);
            renderTabs();
            renderProgress();
            renderFileEditor(path);
            renderMergedPanel(fs);
          },
          onEditManually: () => {
            enterManualMode(fs);
            renderTabs();
            renderProgress();
          },
          onUndo: () => {
            undoHunk(fs, i);
            renderTabs();
            renderProgress();
            renderMergedPanel(fs);
          },
        }
      );
    });

    renderMergedPanel(fs);
  }

  async function selectFile(path) {
    selectedPath = path;
    renderTabs();
    const fs = fileStates.get(path);
    if (!fs.segments) {
      oursBodyEl.innerHTML = baseBodyEl.innerHTML = theirsBodyEl.innerHTML =
        `<div class="loading-row"><div class="spinner"></div><span>Loading…</span></div>`;
      try {
        fs.segments = await getConflictFile(repoPath, path);
        fs.hunkChoices = fs.segments.map(() => null);
      } catch (err) {
        showToast({ variant: "danger", message: String(err) });
        return;
      }
    }
    renderFileEditor(path);
  }

  // Synced scrolling across the three panels — `.rsv-panel-body` is the actual scroll container
  // (`.rsv-panel` itself clips via `overflow: hidden`). A small re-entrancy guard since setting
  // one panel's scrollTop fires its own "scroll" event.
  let syncingScroll = false;
  const scrollPanels = [oursBodyEl, baseBodyEl, theirsBodyEl];
  for (const el of scrollPanels) {
    el.addEventListener(
      "scroll",
      () => {
        if (syncingScroll) return;
        syncingScroll = true;
        for (const other of scrollPanels) {
          if (other !== el) other.scrollTop = el.scrollTop;
        }
        syncingScroll = false;
      },
      { signal }
    );
  }

  editToggleBtn.addEventListener("click", () => {
    const fs = fileStates.get(selectedPath);
    enterManualMode(fs);
  });
  doneEditingBtn.addEventListener("click", () => {
    const fs = fileStates.get(selectedPath);
    fs.manualText = mergedTextareaEl.value;
    fs.panelView = "preview";
    renderMergedPanel(fs);
    renderTabs();
    renderProgress();
  });

  async function doAbort() {
    try {
      await abortConflictResolution(repoPath);
      showToast({ variant: "info", message: "Conflict resolution aborted — working tree restored." });
      onDone?.();
    } catch (err) {
      showToast({ variant: "danger", message: String(err) });
    }
  }

  root.querySelector(".rsv-abort").addEventListener("click", doAbort);

  continueBtn.addEventListener("click", async () => {
    if (continueBtn.classList.contains("disabled")) return;
    const files = session.files.map((path) => ({ path, content: finalText(fileStates.get(path)) }));
    try {
      await finishConflictResolution(repoPath, files);
      showToast({ variant: "success", message: "Conflict resolved." });
      onDone?.();
    } catch (err) {
      showToast({ variant: "danger", message: String(err) });
    }
  });

  // Two-stage Escape (same convention as staging-view.js): first defocuses an active input,
  // second aborts — Abort is the only "exit" this view has, there's no plain "back to graph"
  // that would leave conflict markers in a half-resolved-in-memory state.
  document.addEventListener(
    "keydown",
    (e) => {
      if (e.key !== "Escape") return;
      const active = document.activeElement;
      if (active && (active.tagName === "TEXTAREA" || active.tagName === "INPUT")) {
        active.blur();
        return;
      }
      doAbort();
    },
    { signal }
  );

  const ok = await loadSession();
  if (!ok) return;
  renderTabs();
  renderProgress();
  if (session.files.length > 0) await selectFile(session.files[0]);
}
