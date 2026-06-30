// rebase.html page controller (SPEC.md item 10, PRD §16) — mirrors resolve-page.js's role.
// Mounts the shared sidebar shell, reads the chosen onto-ref from sessionStorage (fresh start)
// or detects an already-in-progress session (restart-resume), then delegates to mountRebase.
// PRD §16.4: Cancel/Escape in the edit mode navigates back here (to index.html); this page
// never mounts the old graph state directly.

import { getAppState, addExistingRepository, detectNestedRepos, pickFolder, getSettings, saveSettings } from "./app.js";
import { renderSidebar } from "./sidebar.js";
import { openCloneDialog } from "./clone-dialog.js";
import { openRepoPickerDialog } from "./repo-picker-dialog.js";
import { mountRebase } from "./rebase-view.js";
import { mountCommandPalette } from "./command-registry.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const SIDEBAR_MIN_WIDTH = 120;
const SIDEBAR_MAX_WIDTH = 360;

let uiSettings = { sidebar_width: 156 };
let latestAppState = null;

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

function exitToGraph() {
  window.location.href = "index.html";
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
    return refreshSidebar();
  }
  await openRepoPickerDialog(path, detection.nested);
  return refreshSidebar();
}

async function handleCloneNew() {
  const appState = await getAppState();
  const outcome = await openCloneDialog({ workspaceContext: appState.workspace_path });
  if (outcome) await refreshSidebar();
}

async function refreshSidebar() {
  const appState = await getAppState();
  latestAppState = appState;
  await renderSidebar(document.getElementById("sidebar"), appState, {
    onAddExisting: handleAddExisting,
    onCloneNew: handleCloneNew,
    onSwitched: exitToGraph,
  });
  return appState;
}

async function init() {
  uiSettings = await getSettings().catch(() => uiSettings);
  applySidebarWidth(uiSettings.sidebar_width);
  setupSidebarResize();

  const appState = await refreshSidebar();
  const repoPath = activeRepoPath(appState);
  const root = document.getElementById("rebase-root");

  if (!repoPath) {
    root.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;">No active repository.</div>`;
    return;
  }

  // Determine whether this is a fresh start (ontoRef in sessionStorage) or a restart-resume
  // (appState.rebase_in_progress already true from a previous crash / force-quit).
  const ontoRef = sessionStorage.getItem("trunk-rebase-onto");
  sessionStorage.removeItem("trunk-rebase-onto");
  const isResume = !ontoRef && appState.rebase_in_progress;

  await mountRebase(root, repoPath, {
    ontoRef: ontoRef || null,
    resume: isResume,
    onDone: exitToGraph,
  });

  mountCommandPalette(() => ({
    repoPath: activeRepoPath(latestAppState || appState),
    appState: latestAppState || appState,
    onMutated: refreshSidebar,
    onAddExisting: handleAddExisting,
    onCloneNew: handleCloneNew,
    goToStaging: () => (window.location.href = "staging.html"),
    goToHistory: exitToGraph,
  }));
}

init();
