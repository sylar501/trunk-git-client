// Minimal "Create branch" prompt opened from the commit-detail overlay's "branch from here"
// action (PRD §4.3, SPEC.md item 4). Deliberately stripped down — no real-time name validation,
// no starting-point picker (the commit is fixed, passed in by the caller). SPEC.md item 8
// (Branch dialogs, a future session) owns the full validated dialog with a starting-point
// dropdown; this is structurally compatible with that — an additive upgrade later, not a
// rewrite. Icon/title match the Component Library's eventual "Create branch" dialog (green +)
// so item 8 only needs to add fields, not change the dialog's identity.

import { createBranchAt } from "./app.js";
import { openDialog } from "../components/dialog.js";

/**
 * Resolves with `{ created: true }` on success; never resolves on cancel.
 * @param {{ sha: string, shortSha: string, summary: string, repoPath: string }} opts
 */
export function openCreateBranchDialog({ sha, shortSha, summary, repoPath }) {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "+",
      iconVariant: "green",
      title: "Create branch",
      subtitle: "New branch starting at this commit",
      size: "small",
      bodyHtml: `
        <div class="df">
          <div class="lbl">Branch name</div>
          <input class="inp" id="branch-name" placeholder="feature/my-branch">
          <div class="hint-err" id="branch-error" hidden></div>
        </div>
        <div class="info-box ib-blue">Starting at <strong>${shortSha}</strong> — ${summary}</div>
      `,
      footerHtml: `
        <div class="btn btn-neutral" id="branch-cancel">Cancel</div>
        <div class="btn btn-green" id="branch-create">Create branch</div>
      `,
    });

    const nameInput = dlg.bodyEl.querySelector("#branch-name");
    const errorEl = dlg.bodyEl.querySelector("#branch-error");
    const createBtn = dlg.footerEl.querySelector("#branch-create");

    dlg.footerEl.querySelector("#branch-cancel").addEventListener("click", () => dlg.close());

    function submit() {
      const name = nameInput.value.trim();
      if (!name) return;
      nameInput.classList.remove("err");
      errorEl.hidden = true;
      createBtn.textContent = "Creating…";
      createBranchAt(repoPath, sha, name)
        .then(() => {
          dlg.close();
          resolve({ created: true });
        })
        .catch((err) => {
          createBtn.textContent = "Create branch";
          nameInput.classList.add("err");
          errorEl.textContent = String(err);
          errorEl.hidden = false;
          nameInput.focus();
        });
    }

    createBtn.addEventListener("click", submit);
    nameInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") submit();
    });
  });
}
