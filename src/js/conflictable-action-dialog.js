// Confirm-before-acting + conflict-choice flow shared by cherry-pick and revert. Both wrap the
// same backend outcome shape (`ConflictableOutcome`: completed/applied_no_commit/conflict), so
// one parameterized module covers both instead of duplicating the dialog chrome, the
// "--no-commit" checkbox, and the post-conflict choice step twice.
//
// Not meant to be awaited by the caller — like `openCreateBranchDialog`, this only resolves
// once something actually happened (a commit was created, a no-commit apply landed, or the
// conflict-choice dialog's Resolve/Abort ran), never on Cancel/Escape — so `graph-view.js` fires
// this and forgets it, the same way it already does for "branch from here".

import { getCommitDetail, cherryPickCommit, revertCommit, abortConflictResolution } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { showToast } from "../components/toast.js";

const COPY = {
  "cherry-pick": { title: "Confirm cherry-pick", verb: "Cherry-pick", pastTense: "Commit cherry-picked." },
  revert: { title: "Confirm revert", verb: "Revert", pastTense: "Commit reverted." },
};

function formatDate(epochSeconds) {
  return new Date(epochSeconds * 1000).toLocaleString();
}

/**
 * @param {{ kind: "cherry-pick"|"revert", repoPath: string, sha: string,
 *   onMutated?: () => Promise<void>|void }} opts
 */
export async function openConflictableActionDialog({ kind, repoPath, sha, onMutated }) {
  const copy = COPY[kind];
  let detail;
  try {
    detail = await getCommitDetail(repoPath, sha);
  } catch (err) {
    showToast({ variant: "danger", message: String(err) });
    return;
  }

  const dlg = openDialog({
    icon: "⑂",
    iconVariant: "blue",
    title: copy.title,
    bodyHtml: `
      <div class="sc-row"><span class="sc-lbl">Commit</span><span class="sc-val cad-sha"></span></div>
      <div class="sc-row"><span class="sc-lbl">Message</span><span class="sc-val cad-message"></span></div>
      <div class="sc-row"><span class="sc-lbl">Author</span><span class="sc-val cad-author"></span></div>
      <div class="sc-row"><span class="sc-lbl">Date</span><span class="sc-val cad-date"></span></div>
      <label class="cb-opt" style="margin-top:10px;">
        <input type="checkbox" id="cad-no-commit" /> Don't create a commit (--no-commit)
      </label>
    `,
    footerHtml: `
      <div class="btn btn-neutral" id="cad-cancel">Cancel</div>
      <div class="btn btn-blue" id="cad-confirm">${copy.verb}</div>
    `,
  });

  // `textContent`, not templated into `bodyHtml` above — commit message/author are free-form
  // repo data, not safe to interpolate as HTML.
  dlg.bodyEl.querySelector(".cad-sha").textContent = detail.short_sha;
  dlg.bodyEl.querySelector(".cad-message").textContent = detail.summary;
  dlg.bodyEl.querySelector(".cad-author").textContent = `${detail.author_name} <${detail.author_email}>`;
  dlg.bodyEl.querySelector(".cad-date").textContent = formatDate(detail.time);

  const confirmBtn = dlg.footerEl.querySelector("#cad-confirm");
  const noCommitCheckbox = dlg.bodyEl.querySelector("#cad-no-commit");
  dlg.footerEl.querySelector("#cad-cancel").addEventListener("click", () => dlg.close());

  confirmBtn.addEventListener("click", async () => {
    confirmBtn.classList.add("disabled");
    try {
      const perform = kind === "cherry-pick" ? cherryPickCommit : revertCommit;
      const outcome = await perform(repoPath, sha, noCommitCheckbox.checked);
      dlg.close();
      if (outcome.status === "completed") {
        showToast({ variant: "success", message: copy.pastTense });
        await onMutated?.();
      } else if (outcome.status === "applied_no_commit") {
        // Deliberately no navigation here — stay on whatever screen the user was on; they can
        // go to Staging when ready to actually commit.
        showToast({ variant: "info", message: "Applied to the working tree — not committed." });
        await onMutated?.();
      } else if (outcome.status === "conflict") {
        openConflictChoiceDialog({ kind, repoPath, onMutated });
      }
    } catch (err) {
      confirmBtn.classList.remove("disabled");
      showToast({ variant: "danger", message: String(err) });
    }
  });
}

function openConflictChoiceDialog({ kind, repoPath, onMutated }) {
  const dlg = openDialog({
    icon: "⚠",
    iconVariant: "amber",
    title: "Conflicted",
    bodyHtml: `<p>Your ${kind} resulted in a conflict. Would you like to resolve the conflicts, or abort the operation?</p>`,
    footerHtml: `
      <div class="btn btn-red" id="cc-abort">Abort</div>
      <div class="btn btn-blue" id="cc-resolve">Resolve conflicts</div>
    `,
    size: "small",
  });
  dlg.footerEl.querySelector("#cc-resolve").addEventListener("click", () => {
    dlg.close();
    window.location.href = "resolve.html";
  });
  dlg.footerEl.querySelector("#cc-abort").addEventListener("click", async () => {
    dlg.close();
    try {
      await abortConflictResolution(repoPath);
      showToast({ variant: "info", message: "Operation aborted — working tree restored." });
      await onMutated?.();
    } catch (err) {
      showToast({ variant: "danger", message: String(err) });
    }
  });
}
