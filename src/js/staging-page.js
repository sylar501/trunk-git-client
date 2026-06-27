// staging.html page controller (SPEC.md item 5, PRD §4.4/§8) — mirrors index-page.js's role:
// mounts the shared sidebar shell, then mounts the staging view for whichever repo is active.
// Repo/workspace add+clone handlers are reused verbatim from index-page.js's pattern so the
// sidebar's "+" button keeps working here; switching the *active* repo away from the one being
// staged doesn't make sense mid-staging, so `onSwitched` just returns to the graph view instead
// of trying to keep this page's stale staging state in sync with a different repo.

import {
  getAppState,
  addExistingRepository,
  detectNestedRepos,
  pickFolder,
  getSettings,
  saveSettings,
} from "./app.js";
import { renderSidebar } from "./sidebar.js";
import { openCloneDialog } from "./clone-dialog.js";
import { openRepoPickerDialog } from "./repo-picker-dialog.js";
import { mountStaging } from "./staging-view.js";
import { showToast } from "../components/toast.js";
import { attachResizeHandle } from "../components/resize-handle.js";

const SIDEBAR_MIN_WIDTH = 120;
const SIDEBAR_MAX_WIDTH = 360;

let uiSettings = { sidebar_width: 156, staging_files_width: 196 };

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
  const root = document.getElementById("staging-root");
  if (!repoPath) {
    root.innerHTML = `<div class="empty-state-hint" style="margin:24px auto;">No active repository.</div>`;
    return;
  }
  await mountStaging(root, repoPath, {
    onExit: exitToGraph,
    filesWidth: uiSettings.staging_files_width,
    onFilesResize: (width) => {
      uiSettings.staging_files_width = width;
      saveSettings({ stagingFilesWidth: width });
    },
  });
}

init();
