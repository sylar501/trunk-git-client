// Nested-repo workspace picker (PRD §15.6). Distinct from welcome.js's `showNestedRepoChoice`
// (which auto-includes everything with no picker, used only from the welcome screen, §15.2).
// This dialog is reachable from the sidebar's "+" → "Add existing" when the chosen folder
// has nested repos — at that point a workspace is always already open (Repository mode has
// no "+" button), so there is no "create a new workspace" step here, only "add to the
// already-open workspace": root pre-checked, nested repos opt-in, looping
// `addExistingRepository` per checked row on confirm.

import { addExistingRepository, repoQuickInfo } from "./app.js";
import { openDialog } from "../components/dialog.js";

function relativeTo(root, path) {
  if (path === root) return ".";
  return path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path;
}

/** Resolves once the picked repos have been added to the workspace, or never on cancel. */
export function openRepoPickerDialog(rootPath, nestedPaths) {
  return new Promise((resolve) => {
    const rows = [rootPath, ...nestedPaths].map((path) => ({
      path,
      checked: path === rootPath,
      hint: "",
    }));

    const dlg = openDialog({
      icon: "⎇",
      iconVariant: "purple",
      title: "Add repositories to workspace",
      subtitle: `${rows.length} repositor${rows.length === 1 ? "y" : "ies"} found`,
      size: "standard",
    });

    function render() {
      dlg.setBody(`
        <div class="df" id="picker-list" style="gap:6px;max-height:240px;overflow-y:auto;">
          ${rows
            .map(
              (row, i) => `
            <label style="display:flex;align-items:flex-start;gap:8px;cursor:pointer;">
              <input type="checkbox" data-i="${i}" ${row.checked ? "checked" : ""}>
              <span style="display:flex;flex-direction:column;">
                <span style="font-family:monospace;font-size:11px;color:var(--text-primary);">${relativeTo(rootPath, row.path)}</span>
                <span class="lbl" data-hint="${i}">${row.hint || "loading…"}</span>
              </span>
            </label>
          `
            )
            .join("")}
        </div>
      `);
      const checkedCount = rows.filter((r) => r.checked).length;
      dlg.setFooter(`
        <div class="btn btn-neutral" id="picker-cancel">Cancel</div>
        <div class="btn btn-green${checkedCount === 0 ? " disabled" : ""}" id="picker-add">Add to workspace</div>
      `);

      dlg.bodyEl.querySelectorAll("input[type=checkbox]").forEach((cb) => {
        cb.addEventListener("change", () => {
          rows[Number(cb.dataset.i)].checked = cb.checked;
          render();
        });
      });
      dlg.footerEl.querySelector("#picker-cancel").addEventListener("click", () => dlg.close());
      const addBtn = dlg.footerEl.querySelector("#picker-add");
      if (checkedCount > 0) {
        addBtn.addEventListener("click", async () => {
          addBtn.textContent = "Adding…";
          addBtn.classList.add("disabled");
          for (const row of rows.filter((r) => r.checked)) {
            await addExistingRepository(row.path);
          }
          dlg.close();
          resolve();
        });
      }
    }

    render();

    // Lazily fetch remote/last-commit hints per row, re-rendering each as it resolves.
    rows.forEach((row, i) => {
      repoQuickInfo(row.path)
        .then((info) => {
          row.hint = info.last_commit_summary || info.remote_url || "no commits yet";
          const el = dlg.bodyEl.querySelector(`[data-hint="${i}"]`);
          if (el) el.textContent = row.hint;
        })
        .catch(() => {});
    });
  });
}
