// Push dialog (PRD §12.1): from/to branch pickers, ahead-commit summary, force/
// force-with-lease — git2 has no native force-with-lease, so the backend emulates it by
// re-fetching and refusing if the remote moved since the last known state (see git/mod.rs).

import {
  listBranches,
  listBranchesWithTracking,
  listRemotes,
  getRemoteUrl,
  listCommitsAhead,
  pushBranch,
  onPushProgress,
} from "./app.js";
import { openDialog } from "../components/dialog.js";
import { showToast } from "../components/toast.js";
import { renderCommitList, fillCommitListText, createProgressLog, attachEnterToClose } from "./push-pull-shared.js";

/** @param {{ repoPath: string, onMutated?: () => Promise<void>|void }} opts */
export async function openPushDialog({ repoPath, onMutated }) {
  let branches, tracking, remotes;
  try {
    [branches, tracking, remotes] = await Promise.all([
      listBranches(repoPath),
      listBranchesWithTracking(repoPath),
      listRemotes(repoPath),
    ]);
  } catch (err) {
    showToast({ variant: "danger", message: String(err) });
    return;
  }
  if (remotes.length === 0) {
    showToast({ variant: "danger", message: "This repository has no remotes configured." });
    return;
  }
  const currentBranch = branches.find((b) => b.is_head)?.name ?? branches[0]?.name;
  if (!currentBranch) {
    showToast({ variant: "danger", message: "No local branches to push." });
    return;
  }

  const state = {
    localBranch: currentBranch,
    remoteName: remotes[0],
    remoteBranch: currentBranch,
    setUpstream: false,
    force: false,
    forceWithLease: true,
  };
  const upstream = tracking.find((b) => b.name === currentBranch)?.upstream;
  if (upstream) {
    const slash = upstream.indexOf("/");
    const remote = slash === -1 ? upstream : upstream.slice(0, slash);
    const branch = slash === -1 ? currentBranch : upstream.slice(slash + 1);
    if (remotes.includes(remote)) {
      state.remoteName = remote;
      state.remoteBranch = branch;
    }
  } else {
    state.setUpstream = true;
  }

  const dlg = openDialog({ icon: "↑", iconVariant: "blue", title: "Push" });

  async function refreshFooterUrl() {
    const urlEl = dlg.footerEl.querySelector(".pf-remote-url");
    if (!urlEl) return;
    try {
      urlEl.textContent = await getRemoteUrl(repoPath, state.remoteName);
    } catch {
      urlEl.textContent = "";
    }
  }

  async function refreshCommitList() {
    const listEl = dlg.bodyEl.querySelector("#pd-commits");
    const goBtn = dlg.footerEl.querySelector("#pd-go");
    if (!listEl) return;
    try {
      const commits = await listCommitsAhead(repoPath, state.localBranch, state.remoteName, state.remoteBranch);
      listEl.innerHTML = renderCommitList(commits, { badgeClass: "pf-badge-new", badgeText: "new" });
      fillCommitListText(listEl, commits);
      if (goBtn) {
        goBtn.textContent = commits.length > 0 ? `push ${commits.length} commit${commits.length === 1 ? "" : "s"}` : "push";
      }
    } catch (err) {
      listEl.innerHTML = `<div class="info-box ib-red">${String(err)}</div>`;
    }
  }

  function render() {
    dlg.setBody(`
      <div class="pf-from-to">
        <select class="inp" id="pd-local">
          ${branches.map((b) => `<option value="${b.name}" ${b.name === state.localBranch ? "selected" : ""}>${b.name}</option>`).join("")}
        </select>
        <span class="pf-from-to-arrow">→</span>
        <select class="inp" id="pd-remote">
          ${remotes.map((r) => `<option value="${r}" ${r === state.remoteName ? "selected" : ""}>${r}</option>`).join("")}
        </select>
        <input class="inp" id="pd-remote-branch" style="flex:1;min-width:0;" value="${state.remoteBranch}" />
      </div>
      <div id="pd-commits"></div>
      <label class="cb-opt"><input type="checkbox" id="pd-upstream" ${state.setUpstream ? "checked" : ""} /> Set as upstream</label>
      <label class="cb-opt"><input type="checkbox" id="pd-force" ${state.force || state.forceWithLease ? "checked" : ""} /> Force push</label>
      <div id="pd-force-zone" style="${state.force || state.forceWithLease ? "" : "display:none"}">
        <div class="info-box ib-red">Force push can overwrite commits on the remote. Make sure no one else's work will be lost.</div>
        <label class="cb-opt"><input type="checkbox" id="pd-lease" ${state.forceWithLease ? "checked" : ""} /> Use --force-with-lease instead (refuses if the remote moved since your last fetch)</label>
      </div>
    `);
    dlg.setFooter(`
      <span class="pf-remote-url"></span>
      <div class="tb-spacer"></div>
      <div class="btn btn-neutral" id="pd-cancel">Cancel</div>
      <div class="btn btn-blue" id="pd-go">push</div>
    `);

    const localSelect = dlg.bodyEl.querySelector("#pd-local");
    const remoteSelect = dlg.bodyEl.querySelector("#pd-remote");
    const remoteBranchInput = dlg.bodyEl.querySelector("#pd-remote-branch");
    const upstreamCheckbox = dlg.bodyEl.querySelector("#pd-upstream");
    const forceCheckbox = dlg.bodyEl.querySelector("#pd-force");
    const leaseCheckbox = dlg.bodyEl.querySelector("#pd-lease");
    const forceZone = dlg.bodyEl.querySelector("#pd-force-zone");

    localSelect.addEventListener("change", () => {
      state.localBranch = localSelect.value;
      if (!remoteBranchInput.dataset.edited) remoteBranchInput.value = state.localBranch;
      state.remoteBranch = remoteBranchInput.value;
      refreshCommitList();
    });
    remoteSelect.addEventListener("change", () => {
      state.remoteName = remoteSelect.value;
      refreshFooterUrl();
      refreshCommitList();
    });
    remoteBranchInput.addEventListener("input", () => {
      remoteBranchInput.dataset.edited = "1";
      state.remoteBranch = remoteBranchInput.value;
      refreshCommitList();
    });
    upstreamCheckbox.addEventListener("change", () => (state.setUpstream = upstreamCheckbox.checked));
    forceCheckbox.addEventListener("change", () => {
      forceZone.style.display = forceCheckbox.checked ? "" : "none";
    });

    dlg.footerEl.querySelector("#pd-cancel").addEventListener("click", () => dlg.close());
    dlg.footerEl.querySelector("#pd-go").addEventListener("click", () => {
      const forceWithLease = forceCheckbox.checked && leaseCheckbox.checked;
      const force = forceCheckbox.checked && !leaseCheckbox.checked;
      renderProgress({ setUpstream: upstreamCheckbox.checked, force, forceWithLease });
    });

    refreshFooterUrl();
    refreshCommitList();
  }

  function renderProgress({ setUpstream, force, forceWithLease }) {
    dlg.setBody(`<div id="pd-log" class="pf-log"></div>`);
    dlg.setFooter(`<div class="btn btn-neutral" id="pd-close">Cancel</div>`);
    dlg.footerEl.querySelector("#pd-close").addEventListener("click", () => dlg.close());

    const logEl = dlg.bodyEl.querySelector("#pd-log");
    const log = createProgressLog();
    log.render(logEl);

    let unlisten;
    onPushProgress((payload) => {
      log.onEvent(payload);
      log.render(logEl);
    }).then((fn) => {
      unlisten = fn;
    });

    pushBranch(repoPath, state.localBranch, state.remoteName, state.remoteBranch, setUpstream, force, forceWithLease)
      .then(async () => {
        if (unlisten) unlisten();
        log.appendLine("");
        log.appendLine("Push complete.");
        log.render(logEl);
        const closeAndFinish = async () => {
          dlg.close();
          showToast({ variant: "success", message: "Push complete." });
          await onMutated?.();
        };
        dlg.setFooter(`<div class="btn btn-blue" id="pd-done">Close</div>`);
        dlg.footerEl.querySelector("#pd-done").addEventListener("click", closeAndFinish);
        attachEnterToClose(dlg, closeAndFinish);
      })
      .catch((err) => {
        if (unlisten) unlisten();
        dlg.setBody(`<div id="pd-log" class="pf-log"></div><div class="info-box ib-red">Push failed: ${String(err)}</div>`);
        log.render(dlg.bodyEl.querySelector("#pd-log"));
        dlg.setFooter(`
          <div class="btn btn-neutral" id="pd-cancel-2">Cancel</div>
          <div class="btn btn-blue" id="pd-retry">Retry</div>
        `);
        dlg.footerEl.querySelector("#pd-cancel-2").addEventListener("click", () => dlg.close());
        dlg.footerEl.querySelector("#pd-retry").addEventListener("click", render);
      });
  }

  render();
}
