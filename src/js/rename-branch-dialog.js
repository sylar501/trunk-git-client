// "Rename branch" dialog (PRD §13.3, SPEC.md item 8) — sidebar context-menu only.

import { renameBranch } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { runDialogTask, validateBranchName } from "./branch-dialog-shared.js";

/**
 * Resolves with `{ renamed: true, oldName, newName }` on success; never resolves on cancel.
 * @param {{ repoPath: string, name: string, existingNames: string[], onMutated?: () => Promise<void>|void }} opts
 */
export function openRenameBranchDialog({ repoPath, name, existingNames, onMutated }) {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "✎",
      iconVariant: "amber",
      title: "Rename branch",
      size: "small",
    });

    function render() {
      dlg.setBody(`
        <div class="df">
          <div class="lbl">Current name</div>
          <input class="inp" id="rb-old" value="${name}" disabled>
        </div>
        <div class="df">
          <div class="lbl">New name</div>
          <input class="inp" id="rb-new" value="${name}">
          <div class="hint-err" id="rb-error" hidden></div>
          <div class="hint-ok" id="rb-ok" hidden>✓ valid</div>
        </div>
        <div class="info-box ib-amber">Renaming does not rename the remote-tracking branch — pushes after this will go to a new remote branch unless you also update the upstream.</div>
      `);
      dlg.setFooter(`
        <div class="btn btn-neutral" id="rb-cancel">Cancel</div>
        <div class="btn btn-amber disabled" id="rb-go">Rename</div>
      `);

      const newInput = dlg.bodyEl.querySelector("#rb-new");
      const errorEl = dlg.bodyEl.querySelector("#rb-error");
      const okEl = dlg.bodyEl.querySelector("#rb-ok");
      const goBtn = dlg.footerEl.querySelector("#rb-go");

      function revalidate() {
        const newName = newInput.value.trim();
        const others = existingNames.filter((n) => n !== name);
        const { valid, error } = validateBranchName(newName, others);
        const unchanged = newName === name;
        const ok = valid && !unchanged;
        newInput.classList.toggle("ok", ok);
        newInput.classList.toggle("err", !valid);
        errorEl.hidden = valid;
        errorEl.textContent = error ?? "";
        okEl.hidden = !ok;
        goBtn.classList.toggle("disabled", !ok);
        return ok;
      }

      newInput.addEventListener("input", revalidate);
      newInput.addEventListener("keydown", (e) => {
        if (e.key === "Enter" && revalidate()) submit();
      });
      dlg.footerEl.querySelector("#rb-cancel").addEventListener("click", () => dlg.close());
      goBtn.addEventListener("click", () => {
        if (revalidate()) submit();
      });

      newInput.focus();
      newInput.select();
      revalidate();
    }

    function submit() {
      const newInput = dlg.bodyEl.querySelector("#rb-new");
      const newName = newInput.value.trim();
      runDialogTask(dlg, {
        task: () => renameBranch(repoPath, name, newName),
        onMutated: async () => {
          await onMutated?.();
          resolve({ renamed: true, oldName: name, newName });
        },
        onError: (err) => {
          render();
          const errorEl = dlg.bodyEl.querySelector("#rb-error");
          dlg.bodyEl.querySelector("#rb-new").classList.add("err");
          errorEl.hidden = false;
          errorEl.textContent = String(err);
        },
      });
    }

    render();
  });
}
