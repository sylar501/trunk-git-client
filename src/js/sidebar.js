// Sidebar page controller (PRD §15.3 Repository mode, §15.4.2 Workspace mode). Renders the
// mode-dependent Repositories section only — Branches/Remotes/Stashes/Tags/`.sb-footer`
// (SPEC.md item 3+) are not built here. Left as a page-level controller rather than folded
// into the still-stub `src/components/sidebar-item.js`: that file is earmarked for the
// generalized multi-section row templates item 3 will need once Branches/Remotes/etc.
// exist — writing that generalization now would be guessing at item 3's shapes.

import { switchActiveRepository, getAppState } from "./app.js";
import { openContextMenu } from "../components/context-menu.js";
import { showToast } from "../components/toast.js";

function basename(path) {
  return path.replace(/\/+$/, "").split("/").pop() || path;
}

/**
 * @param {HTMLElement} container
 * @param {object} appState - shape returned by `getAppState()` / `open_workspace`.
 * @param {object} handlers - { onAddExisting(), onCloneNew(), onSwitched() }
 */
export function renderSidebar(container, appState, handlers = {}) {
  container.innerHTML = "";

  if (appState.mode === "repository") {
    const row = document.createElement("div");
    row.className = "ws-row";
    row.innerHTML = `<span class="ws-name"></span>`;
    row.querySelector(".ws-name").textContent = basename(appState.repo_path || "");
    container.append(row);
    return;
  }

  if (appState.mode !== "workspace") return;

  const wsRow = document.createElement("div");
  wsRow.className = "ws-row";
  wsRow.innerHTML = `<span class="ws-name"></span><span style="color:var(--text-tertiary);font-size:11px;">⌄</span>`;
  wsRow.querySelector(".ws-name").textContent = appState.workspace?.name || "workspace";
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

  for (const repo of repos) {
    const row = document.createElement("div");
    row.className = repo.stale ? "sb-item stale" : repo.active ? "sb-item active" : "sb-item";
    row.innerHTML = `<span class="sb-dot"></span><span class="sb-name"></span><span class="sb-badge" hidden></span>`;
    row.querySelector(".sb-name").textContent = repo.name;
    if (!repo.stale) {
      row.addEventListener("click", async () => {
        try {
          await switchActiveRepository(repo.path);
          const fresh = await getAppState();
          renderSidebar(container, fresh, handlers);
          handlers.onSwitched?.(fresh);
        } catch (err) {
          showToast({ variant: "danger", message: String(err) });
        }
      });
    }
    container.append(row);
  }
}
