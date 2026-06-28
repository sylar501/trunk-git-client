// index.html page controller (PRD §15.3/§15.4/§15.7, SPEC.md item 2 + PRD §7, SPEC.md item 3)
// — mirrors welcome.js's role for welcome.html. Mounts the sidebar, renders the
// empty-workspace state when there are no repos yet, wires "+"/"Add existing"/"Clone new"
// and the drag-drop-onto-canvas shortcut, and mounts the real graph view for whichever repo
// is active.

import {
  getAppState,
  addExistingRepository,
  detectNestedRepos,
  pickFolder,
  onDragDrop,
  getSettings,
  saveSettings,
} from "./app.js";
import { renderSidebar } from "./sidebar.js";
import { openCloneDialog } from "./clone-dialog.js";
import { openRepoPickerDialog } from "./repo-picker-dialog.js";
import { mountGraph } from "./graph-view.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const SIDEBAR_MIN_WIDTH = 120;
const SIDEBAR_MAX_WIDTH = 360;

// Populated from `getSettings()` before the first render (see `init()`) — drag-resizing either
// panel updates this in place so a later re-render (e.g. switching repos) keeps using the
// latest known width, not whatever was loaded at startup.
let uiSettings = { sidebar_width: 156, commit_overlay_width: 264 };

function activeRepoPath(appState) {
  return appState.mode === "repository" ? appState.repo_path : appState.active_repo;
}

function applySidebarWidth(width) {
  document.getElementById("sidebar").style.width = `${width}px`;
}

function setupSidebarResize() {
  attachResizeHandle(document.getElementById("sidebar-resize"), {
    getWidth: () => uiSettings.sidebar_width,
    setWidth: (w) => {
      uiSettings.sidebar_width = w;
      applySidebarWidth(w);
    },
    min: SIDEBAR_MIN_WIDTH,
    max: SIDEBAR_MAX_WIDTH,
    onResizeEnd: (finalWidth) => saveSettings({ sidebarWidth: finalWidth }),
  });
}

async function renderGraphArea(canvas, appState) {
  if (appState.mode === "workspace" && appState.repos.length === 0) {
    canvas.innerHTML = `
      <div class="empty-state-card">
        <div class="empty-state-icon">⎇</div>
        <div class="empty-state-title">No repositories yet</div>
        <div class="empty-state-actions">
          <div class="btn btn-green" id="empty-add">Add repository</div>
          <div class="btn btn-blue" id="empty-clone">Clone repository</div>
        </div>
        <div class="divider" style="width:100%"></div>
        <div class="empty-state-hint">Or drag a folder here to add it</div>
      </div>
    `;
    canvas.querySelector("#empty-add").addEventListener("click", handleAddExisting);
    canvas.querySelector("#empty-clone").addEventListener("click", handleCloneNew);
    return;
  }
  const repoPath = activeRepoPath(appState);
  if (!repoPath) {
    canvas.innerHTML = `<div class="empty-state-hint">No active repository.</div>`;
    return;
  }
  // One shared refresh path for every commit-detail-overlay mutation (PRD §4.3, SPEC.md item
  // 4): cherry-pick/revert only need the graph re-walked, branch-from-here also needs the
  // sidebar's branch list refreshed — `refresh()` already does both together, and the cost is
  // dominated by the graph walk regardless, so there's no value in a narrower, action-specific
  // refresh here.
  await mountGraph(canvas, repoPath, {
    onMutated: () => refresh(),
    overlayWidth: uiSettings.commit_overlay_width,
    onOverlayResize: (width) => {
      uiSettings.commit_overlay_width = width;
      saveSettings({ commitOverlayWidth: width });
    },
    conflicted: appState.conflict_resolution_in_progress,
  });
}

async function refresh() {
  const appState = await getAppState();
  await renderSidebar(document.getElementById("sidebar"), appState, {
    onAddExisting: handleAddExisting,
    onCloneNew: handleCloneNew,
    onSwitched: (fresh) => renderGraphArea(document.getElementById("graph-canvas"), fresh),
  });
  await renderGraphArea(document.getElementById("graph-canvas"), appState);
  return appState;
}

async function handleAddExisting() {
  const path = await pickFolder();
  if (!path) return;
  const detection = await detectNestedRepos(path);
  if (detection.status === "not_a_repo") {
    showToast({ variant: "danger", message: "No Git repository found at this location." });
    return;
  }
  if (detection.status === "plain_repo") {
    await addExistingRepository(path);
    await refresh();
    return;
  }
  await openRepoPickerDialog(path, detection.nested);
  await refresh();
}

async function handleCloneNew() {
  const appState = await getAppState();
  const outcome = await openCloneDialog({ workspaceContext: appState.workspace_path });
  if (outcome) await refresh();
}

async function setupDragDrop() {
  await onDragDrop(async (payload) => {
    if (payload.type !== "drop" || !payload.paths?.length) return;
    const path = payload.paths[0];
    const detection = await detectNestedRepos(path);
    if (detection.status === "not_a_repo") {
      showToast({ variant: "danger", message: "No Git repository found at this location." });
      return;
    }
    // Root .git always wins on drop — no nested-repo picker in this context (PRD §15.7).
    await addExistingRepository(path);
    await refresh();
  });
}

async function init() {
  // Awaited before the first render so the sidebar/overlay never flash at their default width
  // before snapping to the persisted one — a local settings-file read is near-instant.
  uiSettings = await getSettings().catch(() => uiSettings);
  applySidebarWidth(uiSettings.sidebar_width);
  setupSidebarResize();
  await refresh();
  await setupDragDrop().catch(() => {});
}

init();
