// resolve.html page controller (SPEC.md item 6, PRD §4.6/§9) — mirrors staging-page.js's role.
// Mounts the shared sidebar shell, then mounts the conflict resolver for the active repo.
// Switching the active repo away mid-resolution is the one case the sidebar itself guards
// (see sidebar.js's "switch anyway / cancel" confirm) rather than this page — by the time
// `onSwitched` fires here the user has already confirmed leaving the conflict markers as-is.

import { getAppState, addExistingRepository, detectNestedRepos, pickFolder, getSettings, saveSettings } from "./app.js";
import { renderSidebar } from "./sidebar.js";
import { openCloneDialog } from "./clone-dialog.js";
import { openRepoPickerDialog } from "./repo-picker-dialog.js";
import { mountConflictResolver } from "./resolve-view.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const SIDEBAR_MIN_WIDTH = 120;
const SIDEBAR_MAX_WIDTH = 360;

let uiSettings = { sidebar_width: 156, resolve_merged_height: 220 };

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
  const root = document.getElementById("resolve-root");
  if (!repoPath) {
    root.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;">No active repository.</div>`;
    return;
  }
  await mountConflictResolver(root, repoPath, {
    onDone: exitToGraph,
    mergedHeight: uiSettings.resolve_merged_height,
    onMergedResize: (height) => {
      uiSettings.resolve_merged_height = height;
      saveSettings({ resolveMergedHeight: height });
    },
  });
}

init();
