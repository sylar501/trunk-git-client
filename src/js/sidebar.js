// Sidebar page controller (PRD §15.3 Repository mode, §15.4.2 Workspace mode, §4.2 Branches).
// Renders the mode-dependent Repositories section (SPEC.md item 2) plus the Branches section
// (SPEC.md item 3) — Remotes/Stashes/Tags/`.sb-footer` stay out of scope for items 11-13.
// Repositories rendering is left as inline HTML (unchanged from item 2, to avoid touching
// shipped/tested behaviour); Branches rows are built via `sidebar-item.js`'s factory, the
// only place that file is used in this session.

import { switchActiveRepository, getAppState, listBranches, getWorkingTreeStatus, checkoutBranch } from "./app.js";
import { openContextMenu } from "../components/context-menu.js";
import { showToast } from "../components/toast.js";
import { openDialog } from "../components/dialog.js";
import { createSidebarItem, createSidebarSection } from "../components/sidebar-item.js";
import { laneColorVar } from "../components/commit-row.js";
import { confirmDirtyTreeStrategy } from "./branch-dialog-shared.js";
import { openSwitchBranchDialog } from "./switch-branch-dialog.js";
import { openRenameBranchDialog } from "./rename-branch-dialog.js";
import { openDeleteBranchDialog } from "./delete-branch-dialog.js";

/// Mid-conflict-resolution repo switch (PRD §15.4.4, line 540) warns instead of blocking —
/// unlike `rebase_in_progress`, which `switch_active_repository` hard-rejects server-side with
/// no override. Conflict markers on disk are left untouched either way; this only gates whether
/// Trunk's UI navigates away from them.
function confirmSwitchAwayFromConflict() {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "⚠",
      iconVariant: "amber",
      title: "Unresolved merge conflict",
      bodyHtml: `<p>This repository has an unresolved merge conflict. Switch away and return to it later?</p>`,
      footerHtml: `
        <div class="btn btn-neutral" id="csc-cancel">Cancel</div>
        <div class="btn btn-amber" id="csc-switch">Switch anyway</div>
      `,
      size: "small",
    });
    dlg.footerEl.querySelector("#csc-cancel").addEventListener("click", () => {
      dlg.close();
      resolve(false);
    });
    dlg.footerEl.querySelector("#csc-switch").addEventListener("click", () => {
      dlg.close();
      resolve(true);
    });
  });
}

function basename(path) {
  return path.replace(/\/+$/, "").split("/").pop() || path;
}

function openBackToWelcomeMenu(e) {
  e.stopPropagation();
  openContextMenu(e.clientX, e.clientY, [
    {
      label: "Open another repository or workspace…",
      onClick: () => {
        // Without this, welcome.js's fast path (PRD §15.1) sees the same most-recently-opened
        // entry still on top of the recent list and immediately reopens it — the welcome screen
        // flashes for a frame and this menu item becomes a no-op. `welcome.js`'s `init()` checks
        // and clears this flag before deciding whether to fast-path.
        sessionStorage.setItem("trunk-skip-fast-path", "1");
        window.location.href = "welcome.html";
      },
    },
  ]);
}

/// Direct sidebar-row click switch (PRD §6's mouse-profile "Switch branch" capability) — unlike
/// the full Switch dialog (⌘B, switch-branch-dialog.js), the target is already known so this
/// just runs the dirty-tree check + `checkoutBranch` inline, mirroring the Repositories section's
/// own row-click pattern just above (spinner swapped in immediately, single in-flight switch at
/// a time).
async function switchToBranchFromSidebar(repoPath, name, dotEl, onBranchChanged) {
  dotEl.outerHTML = `<div class="spinner"></div>`;
  try {
    let dirty = false;
    try {
      dirty = (await getWorkingTreeStatus(repoPath)).files.length > 0;
    } catch {
      dirty = false;
    }
    let dirtyStrategy;
    if (dirty) {
      dirtyStrategy = await confirmDirtyTreeStrategy();
      if (!dirtyStrategy) return false;
    }
    await checkoutBranch(repoPath, name, { dirtyStrategy });
    await onBranchChanged?.();
    showToast({ variant: "success", message: `Switched to ${name}.` });
    return true;
  } catch (err) {
    showToast({ variant: "danger", message: String(err) });
    return false;
  }
}

/**
 * @param {HTMLElement} container
 * @param {string} repoPath
 * @param {{ onBranchChanged?: () => Promise<void>|void }} handlers
 */
async function appendBranchesSection(container, repoPath, handlers = {}) {
  if (!repoPath) return;
  container.append(
    createSidebarSection("Branches", {
      actionLabel: "⇄",
      onAction: () => {
        openSwitchBranchDialog({ repoPath, onMutated: handlers.onBranchChanged }).then((result) => {
          if (!result?.switched) return;
          showToast({ variant: "success", message: `Switched to ${result.name}.` });
        });
      },
    })
  );

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
  const existingNames = branches.map((b) => b.name);
  let switching = false;

  for (const branch of branches) {
    const row = createSidebarItem({
      dotColor: laneColorVar(branch.color_index),
      label: branch.name,
      badgeText: branch.is_head ? "HEAD" : undefined,
      active: branch.is_head,
    });

    if (!branch.is_head) {
      row.addEventListener("click", async () => {
        if (switching) return;
        switching = true;
        const ok = await switchToBranchFromSidebar(repoPath, branch.name, row.querySelector(".sb-dot"), handlers.onBranchChanged);
        if (!ok) switching = false;
      });
    }

    row.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      openContextMenu(e.clientX, e.clientY, [
        {
          label: "Rename…",
          onClick: () =>
            openRenameBranchDialog({ repoPath, name: branch.name, existingNames, onMutated: handlers.onBranchChanged }).then(
              (result) => {
                if (!result?.renamed) return;
                showToast({ variant: "success", message: `Branch renamed to ${result.newName}.` });
              }
            ),
        },
        ...(branch.is_head
          ? []
          : [
              "separator",
              {
                label: "Delete…",
                danger: true,
                onClick: () =>
                  openDeleteBranchDialog({ repoPath, name: branch.name, onMutated: handlers.onBranchChanged }).then((result) => {
                    if (!result?.deleted) return;
                    showToast({ variant: "success", message: `Branch ${result.name} deleted.` });
                  }),
              },
            ]),
      ]);
    });

    container.append(row);
  }
}

/**
 * @param {HTMLElement} container
 * @param {object} appState - shape returned by `getAppState()` / `open_workspace`.
 * @param {object} handlers - { onAddExisting(), onCloneNew(), onSwitched(), onBranchChanged() }
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
    await appendBranchesSection(container, appState.repo_path, handlers);
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
    if (repo.conflicted) {
      const badge = row.querySelector(".sb-badge");
      badge.hidden = false;
      badge.classList.add("sb-badge-amber");
      badge.textContent = "conflict";
    }
    if (!repo.stale) {
      row.addEventListener("click", async () => {
        // Switching itself (writing `.trunk`, updating AppState) is fast — the slow part is
        // the graph walk + branch enumeration this triggers downstream (large repos can take
        // seconds). Swap the dot for a spinner immediately so a click never looks ignored,
        // and ignore further clicks until this one resolves instead of piling up redundant
        // switches.
        if (switching) return;
        if (appState.conflict_resolution_in_progress) {
          const proceed = await confirmSwitchAwayFromConflict();
          if (!proceed) return;
        }
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

  await appendBranchesSection(container, appState.active_repo, handlers);
}
