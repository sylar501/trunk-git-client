// Fetch dialog (PRD §12.2): single-step, read-only operation — remote picker, prune/tags/
// submodules checkboxes, neutral "fetch" button (fetch is neither constructive nor destructive).

import { listRemotes, getRemoteUrl, fetchRemote, onFetchProgress } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { showToast } from "../components/toast.js";
import { createProgressLog, attachEnterToClose } from "./push-pull-shared.js";

/** @param {{ repoPath: string, onMutated?: () => Promise<void>|void }} opts */
export async function openFetchDialog({ repoPath, onMutated }) {
  let remotes;
  try {
    remotes = await listRemotes(repoPath);
  } catch (err) {
    showToast({ variant: "danger", message: String(err) });
    return;
  }
  if (remotes.length === 0) {
    showToast({ variant: "danger", message: "This repository has no remotes configured." });
    return;
  }

  const dlg = openDialog({
    icon: "↓",
    iconVariant: "neutral",
    title: "Fetch",
    subtitle: "Download new commits and refs — never modifies the working tree",
  });

  function render() {
    dlg.setBody(`
      <div class="df">
        <div class="lbl">Remote</div>
        <select class="inp" id="fd-remote">
          <option value="">All remotes</option>
          ${remotes.map((r) => `<option value="${r}">${r}</option>`).join("")}
        </select>
      </div>
      <div class="info-box ib-blue">Fetch downloads new commits and refs — it never changes your working tree or current branch.</div>
      <label class="cb-opt"><input type="checkbox" id="fd-prune" checked /> Prune deleted remote branches</label>
      <label class="cb-opt"><input type="checkbox" id="fd-tags" /> Fetch tags</label>
      <label class="cb-opt"><input type="checkbox" id="fd-submodules" /> Fetch submodules</label>
    `);
    dlg.setFooter(`
      <span class="pf-remote-url"></span>
      <div class="tb-spacer"></div>
      <div class="btn btn-neutral" id="fd-cancel">Cancel</div>
      <div class="btn btn-neutral" id="fd-go">Fetch</div>
    `);

    const remoteSelect = dlg.bodyEl.querySelector("#fd-remote");
    const urlEl = dlg.footerEl.querySelector(".pf-remote-url");
    async function refreshUrl() {
      const name = remoteSelect.value || remotes[0];
      if (!name) {
        urlEl.textContent = "";
        return;
      }
      try {
        urlEl.textContent = await getRemoteUrl(repoPath, name);
      } catch {
        urlEl.textContent = "";
      }
    }
    remoteSelect.addEventListener("change", refreshUrl);
    refreshUrl();

    dlg.footerEl.querySelector("#fd-cancel").addEventListener("click", () => dlg.close());
    dlg.footerEl.querySelector("#fd-go").addEventListener("click", () => {
      const remoteName = remoteSelect.value || null;
      const prune = dlg.bodyEl.querySelector("#fd-prune").checked;
      const tags = dlg.bodyEl.querySelector("#fd-tags").checked;
      const submodules = dlg.bodyEl.querySelector("#fd-submodules").checked;
      renderProgress(remoteName, prune, tags, submodules);
    });
  }

  function renderProgress(remoteName, prune, tags, submodules) {
    dlg.setBody(`<div id="fd-log" class="pf-log"></div>`);
    dlg.setFooter(`<div class="btn btn-neutral" id="fd-close">Cancel</div>`);
    dlg.footerEl.querySelector("#fd-close").addEventListener("click", () => dlg.close());

    const logEl = dlg.bodyEl.querySelector("#fd-log");
    const log = createProgressLog();
    log.render(logEl);

    let unlisten;
    onFetchProgress((payload) => {
      log.onEvent(payload);
      log.render(logEl);
    }).then((fn) => {
      unlisten = fn;
    });

    fetchRemote(repoPath, remoteName, prune, tags, submodules)
      .then(async (outcome) => {
        if (unlisten) unlisten();
        log.appendLine("");
        log.appendLine(outcome.submodule_warnings?.length ? `Fetch complete (${outcome.submodule_warnings.join("; ")}).` : "Fetch complete.");
        log.render(logEl);
        const closeAndFinish = async () => {
          dlg.close();
          showToast({ variant: outcome.submodule_warnings?.length ? "warning" : "success", message: "Fetch complete." });
          await onMutated?.();
        };
        dlg.setFooter(`<div class="btn btn-neutral" id="fd-done">Close</div>`);
        dlg.footerEl.querySelector("#fd-done").addEventListener("click", closeAndFinish);
        attachEnterToClose(dlg, closeAndFinish);
      })
      .catch((err) => {
        if (unlisten) unlisten();
        dlg.setBody(`<div id="fd-log" class="pf-log"></div><div class="info-box ib-red">Fetch failed: ${String(err)}</div>`);
        log.render(dlg.bodyEl.querySelector("#fd-log"));
        dlg.setFooter(`
          <div class="btn btn-neutral" id="fd-cancel-2">Cancel</div>
          <div class="btn btn-neutral" id="fd-retry">Retry</div>
        `);
        dlg.footerEl.querySelector("#fd-cancel-2").addEventListener("click", () => dlg.close());
        dlg.footerEl.querySelector("#fd-retry").addEventListener("click", () => renderProgress(remoteName, prune, tags, submodules));
      });
  }

  render();
}
