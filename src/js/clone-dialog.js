// Clone dialog, welcome-screen context (PRD §15.5.1): 3-step wizard (Source / Destination /
// Progress) with real streamed git2 clone progress. Kept separate from welcome.js because it's
// the most stateful flow — Step 1/2 field values must survive a Step 3 failure + Retry.

import { cloneRepository, onCloneProgress, pickFolder, defaultDirectory } from "./app.js";
import { openDialog } from "../components/dialog.js";

function suggestNameFromUrl(url) {
  const trimmed = url.trim().replace(/\/+$/, "");
  const last = trimmed.split(/[/:]/).pop() || "repository";
  return last.replace(/\.git$/i, "") || "repository";
}

function joinPath(dir, name) {
  return `${dir.replace(/[/\\]+$/, "")}/${name}`;
}

/** Resolves with `{ repoPath, workspacePath }` on a successful clone; never resolves on cancel. */
export function openCloneDialog() {
  return new Promise((resolve) => {
    const state = { url: "", destination: "", createWorkspace: false, workspacePath: "" };
    // Default "Clone into" should always be a full absolute path, never a bare relative
    // name — fetched once up front so it's ready by the time Step 1's "Next" is clicked.
    const homeDirPromise = defaultDirectory().catch(() => "");
    let dlg;

    function renderStep1() {
      dlg.setBody(`
        <div class="df">
          <div class="lbl">Repository URL</div>
          <input class="inp" id="clone-url" placeholder="https://github.com/owner/repo.git" value="${state.url}">
        </div>
      `);
      dlg.setFooter(`
        <div class="btn btn-neutral" id="clone-cancel">Cancel</div>
        <div class="btn btn-blue" id="clone-next">Next</div>
      `);
      const urlInput = dlg.bodyEl.querySelector("#clone-url");
      urlInput.focus();
      dlg.footerEl.querySelector("#clone-cancel").addEventListener("click", () => dlg.close());
      dlg.footerEl.querySelector("#clone-next").addEventListener("click", async () => {
        const url = urlInput.value.trim();
        if (!url) return;
        state.url = url;
        if (!state.destination) {
          const homeDir = await homeDirPromise;
          state.destination = homeDir ? joinPath(homeDir, suggestNameFromUrl(url)) : suggestNameFromUrl(url);
        }
        renderStep2();
      });
    }

    function renderStep2() {
      dlg.setBody(`
        <div class="df">
          <div class="lbl">Clone into</div>
          <div class="field-row">
            <input class="inp" id="clone-dest" value="${state.destination}">
            <div class="btn btn-neutral" id="clone-dest-browse">Browse…</div>
          </div>
        </div>
        <label class="cb-opt" style="display:flex;align-items:center;gap:8px;">
          <input type="checkbox" id="clone-mk-ws" ${state.createWorkspace ? "checked" : ""}>
          <span>Also create a workspace</span>
        </label>
        <div class="df" id="clone-ws-field" style="${state.createWorkspace ? "" : "display:none"}">
          <div class="lbl">Workspace file</div>
          <div class="field-row">
            <input class="inp" id="clone-ws-path" value="${state.workspacePath}">
            <div class="btn btn-neutral" id="clone-ws-browse">Browse…</div>
          </div>
        </div>
      `);
      dlg.setFooter(`
        <div class="btn btn-neutral" id="clone-back">Back</div>
        <div class="btn btn-green" id="clone-go">Clone Repository</div>
      `);
      const destInput = dlg.bodyEl.querySelector("#clone-dest");
      const wsCheckbox = dlg.bodyEl.querySelector("#clone-mk-ws");
      const wsField = dlg.bodyEl.querySelector("#clone-ws-field");
      const wsPathInput = dlg.bodyEl.querySelector("#clone-ws-path");

      dlg.bodyEl.querySelector("#clone-dest-browse").addEventListener("click", async () => {
        const folder = await pickFolder();
        if (folder) destInput.value = `${folder}/${state.destination.split("/").pop()}`;
      });

      dlg.bodyEl.querySelector("#clone-ws-browse").addEventListener("click", async () => {
        const folder = await pickFolder();
        if (!folder) return;
        const currentName = wsPathInput.value.split("/").pop() || `${state.destination.split("/").pop()}.trunk`;
        wsPathInput.value = `${folder}/${currentName}`;
      });

      wsCheckbox.addEventListener("change", () => {
        wsField.style.display = wsCheckbox.checked ? "" : "none";
        if (wsCheckbox.checked && !wsPathInput.value) {
          wsPathInput.value = `${state.destination}.trunk`;
        }
      });

      dlg.footerEl.querySelector("#clone-back").addEventListener("click", () => {
        state.destination = destInput.value.trim();
        state.createWorkspace = wsCheckbox.checked;
        state.workspacePath = wsPathInput.value.trim();
        renderStep1();
      });
      dlg.footerEl.querySelector("#clone-go").addEventListener("click", () => {
        state.destination = destInput.value.trim();
        state.createWorkspace = wsCheckbox.checked;
        state.workspacePath = wsPathInput.value.trim();
        if (!state.destination) return;
        renderStep3();
      });
    }

    function renderStep3() {
      dlg.setBody(`
        <div class="df">
          <div class="lbl" id="clone-status">Cloning…</div>
          <div id="clone-log" style="font-family:monospace;font-size:10px;color:var(--text-secondary);max-height:140px;overflow-y:auto;"></div>
        </div>
      `);
      dlg.setFooter(`<div class="btn btn-neutral" id="clone-close">Cancel</div>`);
      dlg.footerEl.querySelector("#clone-close").addEventListener("click", () => dlg.close());

      const statusEl = dlg.bodyEl.querySelector("#clone-status");
      const logEl = dlg.bodyEl.querySelector("#clone-log");

      let unlisten;
      onCloneProgress((payload) => {
        const line = `received ${payload.received_objects}/${payload.total_objects} objects (${payload.received_bytes} bytes)`;
        logEl.textContent = line;
      }).then((fn) => {
        unlisten = fn;
      });

      const workspacePath = state.createWorkspace ? state.workspacePath : null;
      cloneRepository(state.url, state.destination, workspacePath)
        .then((outcome) => {
          if (unlisten) unlisten();
          statusEl.textContent = "Clone complete.";
          dlg.close();
          resolve({ repoPath: outcome.repo_path, workspacePath: outcome.workspace_path });
        })
        .catch((err) => {
          if (unlisten) unlisten();
          dlg.setBody(`
            <div class="info-box ib-red">Clone failed: ${String(err)}</div>
          `);
          dlg.setFooter(`
            <div class="btn btn-neutral" id="clone-cancel-2">Cancel</div>
            <div class="btn btn-blue" id="clone-retry">Retry</div>
          `);
          dlg.footerEl.querySelector("#clone-cancel-2").addEventListener("click", () => dlg.close());
          dlg.footerEl.querySelector("#clone-retry").addEventListener("click", () => renderStep2());
        });
    }

    dlg = openDialog({
      icon: "+",
      iconVariant: "green",
      title: "Clone Repository",
      subtitle: "Clone a repository from a URL",
      size: "standard",
    });
    renderStep1();
  });
}
