// Small name+directory prompt, shared by `welcome.js`'s "Create empty workspace"/"Open as
// workspace" and `command-registry.js`'s "Promote to workspace…" (PRD §15.4.1, SPEC.md items
// 1/2/9). Pulled out of `welcome.js` rather than exported from it: ES modules run a file's
// top-level code on import as a side effect, and `welcome.js`'s own top-level `init()` call
// (its fast-path redirect, PRD §15.1) would otherwise fire on every page that merely imports
// this one function — which is exactly what broke `index.html`/`staging.html`/`resolve.html`
// into a reload loop the first time `command-registry.js` imported it from there.

import { openDialog } from "../components/dialog.js";
import { createWorkspace, pickFolder } from "./app.js";

/**
 * `submit` defaults to `createWorkspace` with `initialRepos` baked in — the command
 * palette's "Promote to workspace…" flow passes `promoteToWorkspace` instead, which takes no
 * `initialRepos` (the backend reads the currently-open repo straight off `AppState`).
 */
export function promptWorkspaceDetails({ title, initialRepos, submit = (name, directory) => createWorkspace(name, directory, initialRepos) }) {
  return new Promise((resolve) => {
    const dlg = openDialog({
      icon: "+",
      iconVariant: "purple",
      title,
      bodyHtml: `
        <div class="df">
          <div class="lbl">Workspace name</div>
          <input class="inp" id="ws-name" placeholder="my-workspace">
        </div>
        <div class="df">
          <div class="lbl">Directory</div>
          <div class="field-row">
            <input class="inp" id="ws-dir">
            <div class="btn btn-neutral" id="ws-dir-browse">Browse…</div>
          </div>
        </div>
      `,
      footerHtml: `
        <div class="btn btn-neutral" id="ws-cancel">Cancel</div>
        <div class="btn btn-green" id="ws-create">Create workspace</div>
      `,
      size: "small",
    });
    const nameInput = dlg.bodyEl.querySelector("#ws-name");
    const dirInput = dlg.bodyEl.querySelector("#ws-dir");
    dlg.bodyEl.querySelector("#ws-dir-browse").addEventListener("click", async () => {
      const folder = await pickFolder();
      if (folder) dirInput.value = folder;
    });
    dlg.footerEl.querySelector("#ws-cancel").addEventListener("click", () => dlg.close());
    dlg.footerEl.querySelector("#ws-create").addEventListener("click", async () => {
      const name = nameInput.value.trim();
      const directory = dirInput.value.trim();
      if (!name || !directory) return;
      const result = await submit(name, directory);
      dlg.close();
      resolve(result);
    });
  });
}
