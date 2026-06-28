// Pull dialog (PRD §12.3): into/from pickers, incoming-commit summary, diverged-branch warning,
// integration-strategy radios. A `conflict` outcome (either strategy) hands off straight to the
// existing conflict resolver — no confirm-choice step, per §12.3's "opens automatically".

import { listBranches, listBranchesWithTracking, listRemotes, getRemoteUrl, listCommitsBehind, pullBranch, onFetchProgress } from "./app.js";
import { openDialog } from "../components/dialog.js";
import { showToast } from "../components/toast.js";
import { renderCommitList, fillCommitListText, createProgressLog, attachEnterToClose } from "./push-pull-shared.js";

// The merge/rebase/ff-only integration step itself has no native streamed text (unlike the fetch
// phase above it) — one closing line per strategy stands in for it, same spirit as real git's own
// one-line "Fast-forward."/"Successfully rebased." summaries.
const FINISH_TEXT = {
  rebase: "Successfully rebased.",
  merge: "Merge complete.",
  ff_only: "Fast-forward complete.",
};

const STRATEGY_COPY = {
  rebase: { label: "Rebase", desc: "Replays your local commits on top of the remote's — keeps history linear.", verb: "pull and rebase" },
  merge: { label: "Merge", desc: "Creates a merge commit joining your local and remote history.", verb: "pull and merge" },
  ff_only: { label: "Fast-forward only", desc: "Only succeeds if your branch hasn't diverged — refuses otherwise.", verb: "pull fast-forward only" },
};

/** @param {{ repoPath: string, onMutated?: () => Promise<void>|void }} opts */
export async function openPullDialog({ repoPath, onMutated }) {
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
    showToast({ variant: "danger", message: "No local branches to pull into." });
    return;
  }

  const state = { localBranch: currentBranch, remoteName: remotes[0], remoteBranch: currentBranch, strategy: "rebase" };
  const upstream = tracking.find((b) => b.name === currentBranch)?.upstream;
  if (upstream) {
    const slash = upstream.indexOf("/");
    const remote = slash === -1 ? upstream : upstream.slice(0, slash);
    const branch = slash === -1 ? currentBranch : upstream.slice(slash + 1);
    if (remotes.includes(remote)) {
      state.remoteName = remote;
      state.remoteBranch = branch;
    }
  }

  const dlg = openDialog({ icon: "⇄", iconVariant: "amber", title: "Pull" });

  async function refreshFooterUrl() {
    const urlEl = dlg.footerEl.querySelector(".pf-remote-url");
    if (!urlEl) return;
    try {
      urlEl.textContent = await getRemoteUrl(repoPath, state.remoteName);
    } catch {
      urlEl.textContent = "";
    }
  }

  async function refreshIncoming() {
    const listEl = dlg.bodyEl.querySelector("#pld-commits");
    const warnEl = dlg.bodyEl.querySelector("#pld-diverged");
    if (!listEl) return;
    const local = tracking.find((b) => b.name === state.localBranch);
    try {
      const commits = await listCommitsBehind(repoPath, state.localBranch, state.remoteName, state.remoteBranch);
      listEl.innerHTML = renderCommitList(commits, { badgeClass: "pf-badge-incoming", badgeText: "incoming" });
      fillCommitListText(listEl, commits);
      const diverged = commits.length > 0 && (local?.ahead ?? 0) > 0;
      if (warnEl) warnEl.style.display = diverged ? "" : "none";
    } catch (err) {
      listEl.innerHTML = `<div class="info-box ib-red">${String(err)}</div>`;
    }
  }

  function render() {
    dlg.setBody(`
      <div class="pf-from-to">
        <select class="inp" id="pld-local">
          ${branches.map((b) => `<option value="${b.name}" ${b.name === state.localBranch ? "selected" : ""}>${b.name}</option>`).join("")}
        </select>
        <span class="pf-from-to-arrow">←</span>
        <select class="inp" id="pld-remote">
          ${remotes.map((r) => `<option value="${r}" ${r === state.remoteName ? "selected" : ""}>${r}</option>`).join("")}
        </select>
        <input class="inp" id="pld-remote-branch" style="flex:1;min-width:0;" value="${state.remoteBranch}" />
      </div>
      <div class="info-box ib-amber" id="pld-diverged" style="display:none">Your branch has diverged from the remote — it has commits the remote doesn't, and vice versa.</div>
      <div id="pld-commits"></div>
      <div class="df">
        ${Object.entries(STRATEGY_COPY)
          .map(
            ([key, copy]) => `
            <label class="pf-radio-opt">
              <input type="radio" name="pld-strategy" value="${key}" ${state.strategy === key ? "checked" : ""} />
              <span>
                <div class="pf-radio-opt-title">${copy.label}</div>
                <div class="pf-radio-opt-desc">${copy.desc}</div>
              </span>
            </label>`
          )
          .join("")}
      </div>
    `);
    dlg.setFooter(`
      <span class="pf-remote-url"></span>
      <div class="tb-spacer"></div>
      <div class="btn btn-neutral" id="pld-cancel">Cancel</div>
      <div class="btn btn-amber" id="pld-go">${STRATEGY_COPY[state.strategy].verb}</div>
    `);

    const localSelect = dlg.bodyEl.querySelector("#pld-local");
    const remoteSelect = dlg.bodyEl.querySelector("#pld-remote");
    const remoteBranchInput = dlg.bodyEl.querySelector("#pld-remote-branch");
    const goBtn = dlg.footerEl.querySelector("#pld-go");

    localSelect.addEventListener("change", () => {
      state.localBranch = localSelect.value;
      refreshIncoming();
    });
    remoteSelect.addEventListener("change", () => {
      state.remoteName = remoteSelect.value;
      refreshFooterUrl();
      refreshIncoming();
    });
    remoteBranchInput.addEventListener("input", () => {
      state.remoteBranch = remoteBranchInput.value;
      refreshIncoming();
    });
    dlg.bodyEl.querySelectorAll('input[name="pld-strategy"]').forEach((radio) => {
      radio.addEventListener("change", () => {
        state.strategy = radio.value;
        goBtn.textContent = STRATEGY_COPY[state.strategy].verb;
      });
    });

    dlg.footerEl.querySelector("#pld-cancel").addEventListener("click", () => dlg.close());
    goBtn.addEventListener("click", () => renderProgress());

    refreshFooterUrl();
    refreshIncoming();
  }

  function renderProgress() {
    dlg.setBody(`<div id="pld-log" class="pf-log"></div>`);
    dlg.setFooter(`<div class="btn btn-neutral" id="pld-close">Cancel</div>`);
    dlg.footerEl.querySelector("#pld-close").addEventListener("click", () => dlg.close());

    const logEl = dlg.bodyEl.querySelector("#pld-log");
    const log = createProgressLog();
    log.render(logEl);

    let unlisten;
    onFetchProgress((payload) => {
      log.onEvent(payload);
      log.render(logEl);
    }).then((fn) => {
      unlisten = fn;
    });

    pullBranch(repoPath, state.localBranch, state.remoteName, state.remoteBranch, state.strategy)
      .then(async (outcome) => {
        if (unlisten) unlisten();
        if (outcome.status === "conflict") {
          dlg.close();
          window.location.href = "resolve.html";
          return;
        }
        log.appendLine("");
        log.appendLine(FINISH_TEXT[state.strategy]);
        log.render(logEl);
        const closeAndFinish = async () => {
          dlg.close();
          showToast({ variant: "success", message: "Pull complete." });
          await onMutated?.();
        };
        dlg.setFooter(`<div class="btn btn-amber" id="pld-done">Close</div>`);
        dlg.footerEl.querySelector("#pld-done").addEventListener("click", closeAndFinish);
        attachEnterToClose(dlg, closeAndFinish);
      })
      .catch((err) => {
        if (unlisten) unlisten();
        dlg.setBody(`<div id="pld-log" class="pf-log"></div><div class="info-box ib-red">Pull failed: ${String(err)}</div>`);
        log.render(dlg.bodyEl.querySelector("#pld-log"));
        dlg.setFooter(`
          <div class="btn btn-neutral" id="pld-cancel-2">Cancel</div>
          <div class="btn btn-amber" id="pld-retry">Retry</div>
        `);
        dlg.footerEl.querySelector("#pld-cancel-2").addEventListener("click", () => dlg.close());
        dlg.footerEl.querySelector("#pld-retry").addEventListener("click", render);
      });
  }

  render();
}
