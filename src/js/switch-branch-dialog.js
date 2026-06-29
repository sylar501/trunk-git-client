// "Switch branch" dialog (PRD §13.2, SPEC.md item 8): search-focused list of local branches
// (newest-first) then remote-only branches below a separator. Opened via ⌘B (graph-view.js) or
// the sidebar Branches section's "Switch…" affordance — distinct from a direct sidebar-row
// click, which switches immediately without this picker (see sidebar.js).

import { listBranchesForSwitch, getWorkingTreeStatus, checkoutBranch } from "./app.js";
import { laneColorVar } from "../components/commit-row.js";
import { openDialog } from "../components/dialog.js";
import { runDialogTask, confirmDirtyTreeStrategy } from "./branch-dialog-shared.js";
import { openCreateBranchDialog } from "./create-branch-dialog.js";

/**
 * Resolves with `{ switched: true, name }` on success; never resolves on cancel.
 * @param {{ repoPath: string, onMutated?: () => Promise<void>|void }} opts
 */
export function openSwitchBranchDialog({ repoPath, onMutated }) {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "⌥",
      iconVariant: "blue",
      title: "Switch branch",
      size: "small",
      bodyHtml: `<div class="loading-center"><div class="spinner"></div></div>`,
      footerHtml: "",
    });

    listBranchesForSwitch(repoPath).then((entries) => {
      const state = { query: "", selected: 0 };

      function filtered() {
        const q = state.query.toLowerCase();
        const rows = entries.filter((e) => e.name.toLowerCase().includes(q));
        // Already sorted local-newest-first-then-remote-only by the backend; just keep that
        // order stable under filtering.
        return rows;
      }

      function selectedEntry() {
        return filtered()[state.selected];
      }

      function render() {
        const rows = filtered();
        const local = rows.filter((r) => !r.is_remote_only);
        const remoteOnly = rows.filter((r) => r.is_remote_only);
        const target = selectedEntry();

        dlg.setBody(`
          <input class="inp" id="sb-search" placeholder="filter branches…" value="${state.query}">
          <div class="sb-branch-list" id="sb-list">
            ${local.map((r) => branchRowHtml(r, rows.indexOf(r))).join("")}
            ${remoteOnly.length > 0 ? `<div class="divider"></div>` : ""}
            ${remoteOnly.map((r) => branchRowHtml(r, rows.indexOf(r))).join("")}
          </div>
          ${
            target?.is_remote_only
              ? `<div class="info-box ib-blue">Switching here creates a local branch tracking <strong>${target.remote_label}</strong>.</div>`
              : ""
          }
          <div class="divider" style="border-top:1px dashed var(--border-default);margin-top:4px;"></div>
          <div class="text-link" id="sb-create-new">Create new branch… ⌘⇧B</div>
        `);
        dlg.setFooter(`
          <div class="btn btn-neutral" id="sb-cancel">Cancel</div>
          <div class="btn btn-blue ${!target || target.is_head ? "disabled" : ""}" id="sb-go">${
            target?.is_remote_only ? "Checkout & track" : "Switch"
          }</div>
        `);

        function branchRowHtml(r, index) {
          return `
            <div class="sb-branch-row ${index === state.selected ? "selected" : ""}" data-index="${index}">
              <span class="sb-dot" style="background:${laneColorVar(r.color_index)}"></span>
              <span class="sb-branch-name">${r.name}</span>
              ${r.is_head ? `<span class="sb-badge">current</span>` : r.is_remote_only ? `<span class="sb-branch-remote">${r.remote_label}</span>` : ""}
            </div>
          `;
        }

        const listEl = dlg.bodyEl.querySelector("#sb-list");
        listEl.querySelectorAll(".sb-branch-row").forEach((row) => {
          row.addEventListener("click", () => {
            state.selected = Number(row.dataset.index);
            render();
          });
          row.addEventListener("dblclick", () => attemptSwitch());
        });

        const searchInput = dlg.bodyEl.querySelector("#sb-search");
        searchInput.addEventListener("input", () => {
          state.query = searchInput.value;
          state.selected = 0;
          render();
        });
        searchInput.addEventListener("keydown", (e) => {
          const rowCount = filtered().length;
          if (e.key === "ArrowDown") {
            e.preventDefault();
            state.selected = Math.min(state.selected + 1, rowCount - 1);
            render();
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            state.selected = Math.max(state.selected - 1, 0);
            render();
          } else if (e.key === "Enter") {
            attemptSwitch();
          }
        });

        dlg.bodyEl.querySelector("#sb-create-new").addEventListener("click", () => {
          dlg.close();
          // The dialog itself already shows the create/checkout-failure toast.
          openCreateBranchDialog({ repoPath, onMutated }).then(async (result) => {
            if (!result?.created) return;
            await onMutated?.();
            resolve({ switched: true, name: result.name });
          });
        });
        dlg.footerEl.querySelector("#sb-cancel").addEventListener("click", () => dlg.close());
        dlg.footerEl.querySelector("#sb-go").addEventListener("click", () => attemptSwitch());

        searchInput.focus();
        searchInput.setSelectionRange(searchInput.value.length, searchInput.value.length);
      }

      async function attemptSwitch() {
        const target = selectedEntry();
        if (!target || target.is_head) return;
        const remoteName = target.is_remote_only ? target.remote_label.split("/")[0] : undefined;
        const remoteBranch = target.is_remote_only ? target.name : undefined;

        let dirty = false;
        try {
          dirty = (await getWorkingTreeStatus(repoPath)).files.length > 0;
        } catch {
          dirty = false;
        }

        if (dirty) {
          const strategy = await confirmDirtyTreeStrategy();
          if (!strategy) return;
          runSwitch(target.name, remoteName, remoteBranch, strategy);
        } else {
          runSwitch(target.name, remoteName, remoteBranch);
        }
      }

      function runSwitch(name, remoteName, remoteBranch, dirtyStrategy) {
        runDialogTask(dlg, {
          task: () => checkoutBranch(repoPath, name, { remoteName, remoteBranch, dirtyStrategy }),
          onMutated: async () => {
            await onMutated?.();
            resolve({ switched: true, name });
          },
          onError: (err) => {
            render();
            dlg.bodyEl.insertAdjacentHTML("beforeend", `<div class="info-box ib-red">${String(err)}</div>`);
          },
        });
      }

      render();
    });
  });
}
