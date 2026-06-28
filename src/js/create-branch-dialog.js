// "Create branch" dialog (PRD §13.1, SPEC.md item 8) — upgrades the minimal prompt opened from
// the commit-detail overlay's "branch from here" action (SPEC.md item 4) into the full dialog:
// starting-point dropdown, real-time name validation, checkout/push-after-creating checkboxes.
//
// Two call modes:
//  - Fixed-sha ("branch from here", graph-view.js): `sha`/`shortSha`/`summary` are supplied —
//    starting point is locked to that commit, shown as the original blue info box.
//  - Generic (⌘⇧B, no commit in context): only `repoPath` is supplied — a `<select>` lets the
//    user pick any local branch as the starting point, defaulting to HEAD's branch.

import { createBranchAt, listBranches, listRemotes, pushBranch } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { runWithInlineSuccess, validateBranchName } from "./branch-dialog-shared.js";

/**
 * Resolves with `{ created: true }` on success; never resolves on cancel.
 * @param {{ sha?: string, shortSha?: string, summary?: string, repoPath: string }} opts
 */
export function openCreateBranchDialog({ sha, shortSha, summary, repoPath }) {
  return new Promise((resolve) => {
    const fixedStartingPoint = Boolean(sha);

    const dlg = openDialog({
      icon: "+",
      iconVariant: "green",
      title: "Create branch",
      subtitle: fixedStartingPoint ? "New branch starting at this commit" : "New branch",
      size: "small",
      bodyHtml: `<div class="loading-center"><div class="spinner"></div></div>`,
      footerHtml: "",
    });

    Promise.all([listBranches(repoPath), listRemotes(repoPath).catch(() => [])]).then(([branches, remotes]) => {
      const existingNames = branches.map((b) => b.name);
      const headBranch = branches.find((b) => b.is_head) ?? branches[0];

      const state = {
        name: "",
        checkout: true,
        push: false,
        sha: fixedStartingPoint ? sha : headBranch?.sha ?? "",
        startingBranch: fixedStartingPoint ? null : headBranch?.name,
      };

      function render() {
        dlg.setBody(`
          <div class="df">
            <div class="lbl">Branch name</div>
            <input class="inp" id="branch-name" placeholder="feature/my-branch" value="${state.name}">
            <div class="hint-err" id="branch-error" hidden></div>
            <div class="hint-ok" id="branch-ok" hidden>✓ valid</div>
          </div>
          ${
            fixedStartingPoint
              ? `<div class="info-box ib-blue">Starting at <strong>${shortSha}</strong> — ${summary}</div>`
              : `
                <div class="df">
                  <div class="lbl">Starting point</div>
                  <select class="inp" id="branch-start">
                    ${branches
                      .map(
                        (b) =>
                          `<option value="${b.name}" ${b.name === state.startingBranch ? "selected" : ""}>${
                            b.is_head ? `HEAD (${b.name})` : b.name
                          }</option>`
                      )
                      .join("")}
                  </select>
                </div>
              `
          }
          <label class="cb-opt"><input type="checkbox" id="branch-checkout" ${state.checkout ? "checked" : ""} /> Checkout after creating</label>
          <label class="cb-opt"><input type="checkbox" id="branch-push" ${state.push ? "checked" : ""} ${
            remotes.length === 0 ? "disabled" : ""
          } /> Push to remote after creating</label>
        `);
        dlg.setFooter(`
          <span style="color:var(--text-tertiary);font-family:monospace;font-size:10px;">from ${state.sha.slice(0, 7)}</span>
          <div class="tb-spacer"></div>
          <div class="btn btn-neutral" id="branch-cancel">Cancel</div>
          <div class="btn btn-green disabled" id="branch-create">Create branch</div>
        `);

        const nameInput = dlg.bodyEl.querySelector("#branch-name");
        const errorEl = dlg.bodyEl.querySelector("#branch-error");
        const okEl = dlg.bodyEl.querySelector("#branch-ok");
        const createBtn = dlg.footerEl.querySelector("#branch-create");
        const startSelect = dlg.bodyEl.querySelector("#branch-start");

        function revalidate() {
          const { valid, error } = validateBranchName(nameInput.value.trim(), existingNames);
          nameInput.classList.toggle("ok", valid);
          nameInput.classList.toggle("err", !valid && nameInput.value.length > 0);
          errorEl.hidden = valid || nameInput.value.length === 0;
          errorEl.textContent = error ?? "";
          okEl.hidden = !valid;
          createBtn.classList.toggle("disabled", !valid);
          return valid;
        }

        nameInput.addEventListener("input", () => {
          state.name = nameInput.value;
          revalidate();
        });
        nameInput.addEventListener("keydown", (e) => {
          if (e.key === "Enter" && revalidate()) submit();
        });

        if (startSelect) {
          startSelect.addEventListener("change", () => {
            state.startingBranch = startSelect.value;
            state.sha = branches.find((b) => b.name === startSelect.value)?.sha ?? state.sha;
            render();
          });
        }

        dlg.bodyEl.querySelector("#branch-checkout").addEventListener("change", (e) => {
          state.checkout = e.target.checked;
        });
        dlg.bodyEl.querySelector("#branch-push").addEventListener("change", (e) => {
          state.push = e.target.checked;
        });

        dlg.footerEl.querySelector("#branch-cancel").addEventListener("click", () => dlg.close());
        createBtn.addEventListener("click", () => {
          if (revalidate()) submit();
        });

        nameInput.focus();
        revalidate();
      }

      function submit() {
        const name = state.name.trim();
        runWithInlineSuccess(dlg, {
          task: async () => {
            await createBranchAt(repoPath, state.sha, name, state.checkout);
            if (state.push && remotes.length > 0) {
              await pushBranch(repoPath, name, remotes[0], name, true, false, false);
            }
          },
          successMessage: "Branch created.",
          onMutated: () => resolve({ created: true }),
          onError: (err) => {
            render();
            const errorEl = dlg.bodyEl.querySelector("#branch-error");
            const nameInput = dlg.bodyEl.querySelector("#branch-name");
            nameInput.classList.add("err");
            errorEl.hidden = false;
            errorEl.textContent = String(err);
            nameInput.focus();
          },
        });
      }

      render();
    });
  });
}
