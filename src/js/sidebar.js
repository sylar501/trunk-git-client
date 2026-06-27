// Sidebar page controller (PRD §15.3 Repository mode, §15.4.2 Workspace mode, §4.2 Branches).
// Renders the mode-dependent Repositories section (SPEC.md item 2) plus the Branches section
// (SPEC.md item 3) — Remotes/Stashes/Tags/`.sb-footer` stay out of scope for items 11-13.
// Repositories rendering is left as inline HTML (unchanged from item 2, to avoid touching
// shipped/tested behaviour); Branches rows are built via `sidebar-item.js`'s factory, the
// only place that file is used in this session.

import { switchActiveRepository, getAppState, listBranches } from "./app.js";
import { openContextMenu } from "../components/context-menu.js";
import { showToast } from "../components/toast.js";
import { createSidebarItem, createSidebarSection } from "../components/sidebar-item.js";
import { laneColorVar } from "../components/commit-row.js";

function basename(path) {
  return path.replace(/\/+$/, "").split("/").pop() || path;
}

function openBackToWelcomeMenu(e) {
  e.stopPropagation();
  openContextMenu(e.clientX, e.clientY, [
    {
      label: "Open another repository or workspace…",
      onClick: () => {
        window.location.href = "welcome.html";
      },
    },
  ]);
}

async function appendBranchesSection(container, repoPath) {
  if (!repoPath) return;
  container.append(createSidebarSection("Branches"));

  // Branch enumeration is cheap on a small repo but can still lag on a large one (many refs,
  // slow disk) — show immediate feedback rather than leaving the section looking frozen
  // while `listBranches` is in flight.
  const loadingRow = document.createElement("div");
  loadingRow.className = "loading-row";
  loadingRow.innerHTML = `<div class="spinner"></div><span>Loading branches…</span>`;
  container.append(loadingRow);

  let branches;
  try {
    branches = await listBranches(repoPath);
  } catch {
    loadingRow.remove();
    return; // repo unreadable (e.g. stale) — section header alone is harmless
  }
  loadingRow.remove();
  for (const branch of branches) {
    container.append(
      createSidebarItem({
        dotColor: laneColorVar(branch.color_index),
        label: branch.name,
        badgeText: branch.is_head ? "HEAD" : undefined,
        active: branch.is_head,
      })
    );
  }
}

/**
 * @param {HTMLElement} container
 * @param {object} appState - shape returned by `getAppState()` / `open_workspace`.
 * @param {object} handlers - { onAddExisting(), onCloneNew(), onSwitched() }
 */
export async function renderSidebar(container, appState, handlers = {}) {
  container.innerHTML = "";

  if (appState.mode === "repository") {
    const row = document.createElement("div");
    row.className = "ws-row";
    row.innerHTML = `<span class="ws-name"></span>`;
    row.querySelector(".ws-name").textContent = basename(appState.repo_path || "");
    row.addEventListener("click", openBackToWelcomeMenu);
    container.append(row);
    await appendBranchesSection(container, appState.repo_path);
    return;
  }

  if (appState.mode !== "workspace") return;

  const wsRow = document.createElement("div");
  wsRow.className = "ws-row";
  wsRow.innerHTML = `<span class="ws-name"></span><span style="color:var(--text-tertiary);font-size:11px;">⌄</span>`;
  wsRow.querySelector(".ws-name").textContent = appState.workspace?.name || "workspace";
  wsRow.addEventListener("click", openBackToWelcomeMenu);
  container.append(wsRow);

  const sec = document.createElement("div");
  sec.className = "sb-sec";
  sec.innerHTML = `<span>Repositories</span><span class="sb-add">+</span>`;
  sec.querySelector(".sb-add").addEventListener("click", (e) => {
    e.stopPropagation();
    openContextMenu(e.clientX, e.clientY, [
      { label: "Add existing repository", onClick: () => handlers.onAddExisting?.() },
      { label: "Clone new repository", onClick: () => handlers.onCloneNew?.() },
    ]);
  });
  container.append(sec);

  const repos = appState.repos || [];
  let switching = false;

  for (const repo of repos) {
    const row = document.createElement("div");
    row.className = repo.stale ? "sb-item stale" : repo.active ? "sb-item active" : "sb-item";
    row.innerHTML = `<span class="sb-dot"></span><span class="sb-name"></span><span class="sb-badge" hidden></span>`;
    row.querySelector(".sb-name").textContent = repo.name;
    if (!repo.stale) {
      row.addEventListener("click", async () => {
        // Switching itself (writing `.trunk`, updating AppState) is fast — the slow part is
        // the graph walk + branch enumeration this triggers downstream (large repos can take
        // seconds). Swap the dot for a spinner immediately so a click never looks ignored,
        // and ignore further clicks until this one resolves instead of piling up redundant
        // switches.
        if (switching) return;
        switching = true;
        row.querySelector(".sb-dot").outerHTML = `<div class="spinner"></div>`;
        try {
          await switchActiveRepository(repo.path);
          const fresh = await getAppState();
          await renderSidebar(container, fresh, handlers);
          handlers.onSwitched?.(fresh);
        } catch (err) {
          switching = false;
          showToast({ variant: "danger", message: String(err) });
        }
      });
    }
    container.append(row);
  }

  await appendBranchesSection(container, appState.active_repo);
}
