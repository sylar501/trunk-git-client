// index.html page controller (PRD §15.3/§15.4/§15.7, SPEC.md item 2) — mirrors welcome.js's
// role for welcome.html. Mounts the sidebar, renders the empty-workspace state when there
// are no repos yet, and wires "+", "Add existing"/"Clone new", and the drag-drop-onto-canvas
// shortcut. The graph canvas itself stays a placeholder — SPEC.md item 3 builds it out.

import {
  getAppState,
  addExistingRepository,
  detectNestedRepos,
  pickFolder,
  onDragDrop,
} from "./app.js";
import { renderSidebar } from "./sidebar.js";
import { openCloneDialog } from "./clone-dialog.js";
import { openRepoPickerDialog } from "./repo-picker-dialog.js";
import { showToast } from "../components/toast.js";

function renderGraphPlaceholder(canvas, appState) {
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
  canvas.innerHTML = `<div class="empty-state-hint">Graph view lands in SPEC.md item 3.</div>`;
}

async function refresh() {
  const appState = await getAppState();
  renderSidebar(document.getElementById("sidebar"), appState, {
    onAddExisting: handleAddExisting,
    onCloneNew: handleCloneNew,
    onSwitched: () => renderGraphPlaceholder(document.getElementById("graph-canvas"), appState),
  });
  renderGraphPlaceholder(document.getElementById("graph-canvas"), appState);
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
  await refresh();
  await setupDragDrop().catch(() => {});
}

init();
