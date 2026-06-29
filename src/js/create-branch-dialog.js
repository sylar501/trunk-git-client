// "Create branch" dialog (PRD §13.1, SPEC.md item 8) — upgrades the minimal prompt opened from
// the commit-detail overlay's "branch from here" action (SPEC.md item 4) into the full dialog:
// starting-point dropdown, real-time name validation, checkout/push-after-creating checkboxes.
//
// Two call modes:
//  - Fixed-sha ("branch from here", graph-view.js): `sha`/`shortSha`/`summary` are supplied —
//    starting point is locked to that commit, shown as the original blue info box.
//  - Generic (⌘⇧B, no commit in context): only `repoPath` is supplied — a `<select>` lets the
//    user pick any local branch as the starting point, defaulting to HEAD's branch.

import { createBranchAt, checkoutBranch, listBranches, listRemotes, getWorkingTreeStatus } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { showToast } from "../components/toast.js";
import { validateBranchName, confirmDirtyTreeStrategy } from "./branch-dialog-shared.js";
import { openPushDialog } from "./push-dialog.js";

/**
 * Resolves with `{ created: true, name }` on success; never resolves on cancel. The dialog
 * itself owns the create/checkout/push-handoff toasts (see `submit()` below) — callers should
 * only use the resolved value to refresh (e.g. `onMutated`), not to show their own "created"
 * toast, since that would duplicate or contradict whichever one this dialog already showed.
 * @param {{ sha?: string, shortSha?: string, summary?: string, repoPath: string, onMutated?: () => Promise<void>|void }} opts
 *   `onMutated` is only used for the push-handoff step (§13.1's "Push to remote after creating")
 *   — the create/checkout refresh is the caller's own job after the returned promise resolves.
 */
export function openCreateBranchDialog({ sha, shortSha, summary, repoPath, onMutated }) {
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

      // "Checkout after creating" moves HEAD to a new starting point exactly like the Switch
      // dialog does — same dirty-tree exposure, so it gets the same upfront dirty check +
      // stash/carry confirm as Switch, rather than silently checking out with no warning.
      //
      // Create, checkout, and push are three independent failure boundaries, deliberately not
      // run as one bundled task (see this session's bug history): the branch can exist even if
      // checkout fails, and checkout can succeed even if a later push fails (invalid credentials,
      // no network, etc) — collapsing them into one try/catch made every later failure look like
      // "nothing happened" and skipped refreshing the sidebar/graph even though the branch was
      // real. Push itself is no longer attempted here at all: a network push needs progress/retry
      // UI the Push dialog already has, so success just hands off to it instead of pushing
      // silently inside this dialog's own spinner.
      async function submit() {
        const name = state.name.trim();

        let dirtyStrategy;
        if (state.checkout) {
          let dirty = false;
          try {
            dirty = (await getWorkingTreeStatus(repoPath)).files.length > 0;
          } catch {
            dirty = false;
          }
          if (dirty) {
            dirtyStrategy = await confirmDirtyTreeStrategy("Create branch");
            if (!dirtyStrategy) return; // cancelled — leave the form as-is
          }
        }

        dlg.setBody(`<div class="loading-center"><div class="spinner lg"></div></div>`);
        dlg.setFooter("");

        // Step 1: create the ref. Failure here means nothing happened — restore the form with an
        // inline error, same convention as every other dialog's failure path.
        try {
          await createBranchAt(repoPath, state.sha, name);
        } catch (err) {
          render();
          const errorEl = dlg.bodyEl.querySelector("#branch-error");
          const nameInput = dlg.bodyEl.querySelector("#branch-name");
          nameInput.classList.add("err");
          errorEl.hidden = false;
          errorEl.textContent = String(err);
          nameInput.focus();
          return;
        }

        // Step 2: optionally check it out. The branch exists regardless of what happens here, so
        // a failure must not look like creation failed — close the dialog, refresh (via the
        // resolved promise below), and surface a distinct warning toast instead of reopening the
        // form for an operation that already partially succeeded.
        let checkoutFailed = false;
        let checkoutErrorMessage = "";
        if (state.checkout) {
          try {
            await checkoutBranch(repoPath, name, { dirtyStrategy });
          } catch (err) {
            checkoutFailed = true;
            checkoutErrorMessage = String(err);
          }
        }

        dlg.close();
        if (checkoutFailed) {
          showToast({ variant: "warning", message: `Branch ${name} created, but checkout failed: ${checkoutErrorMessage}` });
        } else {
          showToast({ variant: "success", message: `Branch ${name} created.` });
        }
        resolve({ created: true, name });

        // Step 3: push handoff. Only offered once create+checkout both succeeded — pushing
        // doesn't strictly require a checkout to have worked, but a checkout failure means
        // something unexpected happened that's worth pausing on rather than barreling into a
        // second dialog.
        if (!checkoutFailed && state.push && remotes.length > 0) {
          openPushDialog({ repoPath, initialLocalBranch: name, onMutated });
        }
      }

      render();
    });
  });
}
