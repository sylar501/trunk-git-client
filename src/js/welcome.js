// Welcome screen page controller (PRD §15.1, §15.2). Runs the fast-path check before
// painting any UI, then wires the two primary buttons, the recent list, the nested-repo
// choice dialog, and the create-empty-workspace prompt.

import {
  openRepository,
  openWorkspace,
  listRecent,
  removeRecent,
  detectNestedRepos,
  createWorkspace,
  pickFolder,
} from "./app.js";
import { openDialog } from "../components/dialog.js";
import { showToast } from "../components/toast.js";
import { openContextMenu } from "../components/context-menu.js";
import { openCloneDialog } from "./clone-dialog.js";

function goToGraph() {
  window.location.href = "index.html";
}

function relativeTime(epochSeconds) {
  const deltaSeconds = Math.max(0, Math.floor(Date.now() / 1000) - epochSeconds);
  if (deltaSeconds < 60) return "just now";
  const minutes = Math.floor(deltaSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function basename(path) {
  return path.replace(/\/+$/, "").split("/").pop() || path;
}

async function openRecentEntry(entry) {
  if (entry.kind === "repository") {
    await openRepository(entry.path);
  } else {
    await openWorkspace(entry.path);
  }
  goToGraph();
}

/** Small name+directory prompt, shared by "Create empty workspace" and "Open as workspace". */
function promptWorkspaceDetails({ title, initialRepos }) {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "+",
      iconVariant: "purple",
      title,
      bodyHtml: `
        <div class="df">
          <div class="lbl">Workspace name</div>
          <input class="inp" id="ws-name" placeholder="my-workspace">
        </div>
        <div class="df">
          <div class="lbl">Directory</div>
          <div class="field-row">
            <input class="inp" id="ws-dir">
            <div class="btn btn-neutral" id="ws-dir-browse">Browse…</div>
          </div>
        </div>
      `,
      footerHtml: `
        <div class="btn btn-neutral" id="ws-cancel">Cancel</div>
        <div class="btn btn-green" id="ws-create">Create workspace</div>
      `,
      size: "small",
    });
    const nameInput = dlg.bodyEl.querySelector("#ws-name");
    const dirInput = dlg.bodyEl.querySelector("#ws-dir");
    dlg.bodyEl.querySelector("#ws-dir-browse").addEventListener("click", async () => {
      const folder = await pickFolder();
      if (folder) dirInput.value = folder;
    });
    dlg.footerEl.querySelector("#ws-cancel").addEventListener("click", () => dlg.close());
    dlg.footerEl.querySelector("#ws-create").addEventListener("click", async () => {
      const name = nameInput.value.trim();
      const directory = dirInput.value.trim();
      if (!name || !directory) return;
      const result = await createWorkspace(name, directory, initialRepos);
      dlg.close();
      resolve(result);
    });
  });
}

function showNestedRepoChoice(rootPath, nested) {
  const dlg = openDialog({
    icon: "⎇",
    iconVariant: "amber",
    title: "Nested repositories detected",
    subtitle: `Found ${nested.length} repositor${nested.length === 1 ? "y" : "ies"} inside this folder`,
    bodyHtml: `<div class="lbl">Open the root folder as a single repository, or create a workspace containing it and the nested repositories below it.</div>`,
    footerHtml: `
      <div class="btn btn-neutral" id="choice-repo">Open as repository</div>
      <div class="btn btn-green" id="choice-ws">Open as workspace</div>
    `,
    size: "small",
  });
  dlg.footerEl.querySelector("#choice-repo").addEventListener("click", async () => {
    dlg.close();
    await openRepository(rootPath);
    goToGraph();
  });
  dlg.footerEl.querySelector("#choice-ws").addEventListener("click", async () => {
    dlg.close();
    const result = await promptWorkspaceDetails({
      title: "Open as workspace",
      initialRepos: [rootPath, ...nested],
    });
    await openWorkspace(result.path);
    goToGraph();
  });
}

async function handleOpenRepository() {
  const path = await pickFolder();
  if (!path) return;
  const detection = await detectNestedRepos(path);
  if (detection.status === "not_a_repo") {
    showToast({ variant: "danger", message: "No Git repository found at this location." });
    return;
  }
  if (detection.status === "plain_repo") {
    await openRepository(path);
    goToGraph();
    return;
  }
  showNestedRepoChoice(path, detection.nested);
}

async function handleCloneRepository() {
  const outcome = await openCloneDialog();
  if (outcome) goToGraph();
}

async function handleCreateEmptyWorkspace() {
  const result = await promptWorkspaceDetails({
    title: "Create empty workspace",
    initialRepos: [],
  });
  await openWorkspace(result.path);
  goToGraph();
}

function renderRecentRow(entry) {
  const row = document.createElement("div");
  row.className = entry.stale ? "recent-row stale" : "recent-row";

  const iconClass = entry.stale ? "stale" : entry.kind;
  row.innerHTML = `
    <div class="recent-icon ${iconClass}">${entry.kind === "workspace" ? "▤" : "⌂"}</div>
    <div class="recent-meta">
      <div class="recent-name"></div>
      <div class="recent-path"></div>
    </div>
    <div class="recent-time"></div>
  `;
  row.querySelector(".recent-name").textContent = basename(entry.path);
  row.querySelector(".recent-path").textContent = entry.path;
  row.querySelector(".recent-time").textContent = entry.stale ? "not found" : relativeTime(entry.last_opened);

  if (!entry.stale) {
    row.addEventListener("click", () => openRecentEntry(entry));
  }
  row.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    openContextMenu(e.clientX, e.clientY, [
      {
        label: "Remove from recent",
        danger: true,
        onClick: async () => {
          await removeRecent(entry.path);
          renderRecentList();
        },
      },
    ]);
  });
  return row;
}

async function renderRecentList() {
  const list = document.getElementById("recent-list");
  const empty = document.getElementById("recent-empty");
  const entries = await listRecent();
  list.innerHTML = "";
  if (entries.length === 0) {
    empty.hidden = false;
    return;
  }
  empty.hidden = true;
  for (const entry of entries) {
    list.append(renderRecentRow(entry));
  }
}

// PRD §15.1 fast path is implemented (see openRecentEntry below) but disabled for now:
// index.html is still an empty placeholder (PRD §7, Main graph view — SPEC.md item 3),
// so skipping straight to it leaves no way back to Welcome short of deleting recent.json.
// Re-enable once the graph view exists and can navigate back to Welcome on its own.
const FAST_PATH_ENABLED = false;

async function init() {
  const entries = await listRecent();
  const newest = entries[0];
  if (FAST_PATH_ENABLED && newest && !newest.stale) {
    await openRecentEntry(newest);
    return;
  }

  document.getElementById("welcome-root").hidden = false;
  await renderRecentList();

  document.getElementById("btn-open-repo").addEventListener("click", handleOpenRepository);
  document.getElementById("btn-clone-repo").addEventListener("click", handleCloneRepository);
  document.getElementById("link-create-workspace").addEventListener("click", (e) => {
    e.preventDefault();
    handleCreateEmptyWorkspace();
  });
}

init();
