// "Delete branch" dialog (PRD §13.4, SPEC.md item 8) — sidebar context-menu only. The mandatory
// acknowledgement checkbox on the unmerged path is non-negotiable (PRD §13's closing note: no
// undo if the commits weren't pushed) — Force Delete stays at 40% opacity until it's checked.

import { getBranchDeleteInfo, deleteBranch } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { runWithInlineSuccess } from "./branch-dialog-shared.js";

/**
 * Resolves with `{ deleted: true }` on success; never resolves on cancel.
 * @param {{ repoPath: string, name: string, onMutated?: () => Promise<void>|void }} opts
 */
export function openDeleteBranchDialog({ repoPath, name, onMutated }) {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "🗑",
      iconVariant: "red",
      title: "Delete branch",
      subtitle: name,
      size: "small",
      bodyHtml: `<div class="loading-center"><div class="spinner"></div></div>`,
      footerHtml: "",
    });

    getBranchDeleteInfo(repoPath, name)
      .then((info) => render(info))
      .catch((err) => {
        dlg.setBody(`<div class="info-box ib-red">${String(err)}</div>`);
        dlg.setFooter(`<div class="btn btn-neutral" id="db-cancel">Close</div>`);
        dlg.footerEl.querySelector("#db-cancel").addEventListener("click", () => dlg.close());
      });

    function render(info) {
      const alsoDeleteRemoteId = "db-also-remote";
      dlg.setBody(
        info.merged
          ? `
            <div class="info-box ib-green">"${name}" is fully merged into HEAD. Safe to delete.</div>
            <label class="cb-opt"><input type="checkbox" id="${alsoDeleteRemoteId}" /> Also delete remote branch</label>
          `
          : `
            <div class="info-box ib-red">"${name}" is not fully merged — ${info.commit_loss_count} commit${
              info.commit_loss_count === 1 ? "" : "s"
            } would be lost if you delete it.</div>
            <label class="cb-opt"><input type="checkbox" id="db-ack" /> I understand this work may be lost</label>
          `
      );
      dlg.setFooter(`
        <div class="btn btn-neutral" id="db-cancel">Cancel</div>
        <div class="btn btn-red ${info.merged ? "" : "disabled-40"}" id="db-go">${info.merged ? "Delete" : "Force delete"}</div>
      `);

      const goBtn = dlg.footerEl.querySelector("#db-go");
      dlg.footerEl.querySelector("#db-cancel").addEventListener("click", () => dlg.close());

      if (!info.merged) {
        const ack = dlg.bodyEl.querySelector("#db-ack");
        ack.addEventListener("change", () => goBtn.classList.toggle("disabled-40", !ack.checked));
      }

      goBtn.addEventListener("click", () => {
        if (!info.merged && !dlg.bodyEl.querySelector("#db-ack").checked) return;
        const alsoDeleteRemote = info.merged ? dlg.bodyEl.querySelector(`#${alsoDeleteRemoteId}`).checked : false;
        // `force` bypasses the backend's own merge check — only needed on the unmerged path;
        // the safe path doesn't need it since the branch is already merged anyway.
        submit(/* force */ !info.merged, alsoDeleteRemote);
      });
    }

    function submit(force, alsoDeleteRemote) {
      runWithInlineSuccess(dlg, {
        task: () => deleteBranch(repoPath, name, force, alsoDeleteRemote),
        successMessage: "Branch deleted.",
        onMutated: async () => {
          await onMutated?.();
          resolve({ deleted: true });
        },
        onError: (err) => {
          dlg.setBody(`<div class="info-box ib-red">${String(err)}</div>`);
          dlg.setFooter(`<div class="btn btn-neutral" id="db-cancel-2">Close</div>`);
          dlg.footerEl.querySelector("#db-cancel-2").addEventListener("click", () => dlg.close());
        },
      });
    }
  });
}
