// Commit panel (PRD §8) — right column (214px) of the staging view: message textarea, amend/
// SSH-sign/push-after-commit options, read-only author/branch display, and the commit button.
//
// Design decision (confirmed with user, deviates from PRD §8's literal "Primary: solid blue
// 'commit to main'. Secondary: blue-outline 'amend last commit'" two-button wording — see
// SPEC.md item 5): there is only ONE commit button here. The amend checkbox controls its
// label/colour — unchecked: solid blue "commit to <branch>"; checked: blue-outline "amend last
// commit" (and the textarea pre-fills with the previous commit's message). No second always-
// visible button.

/** @param {{
 *   onCommit: ({ message, amend, sshSign }) => Promise<void>|void,
 *   onAmendToggle: (turningOn: boolean) => Promise<string|null>|string|null - called when the
 *     amend checkbox changes; turning it on should resolve to the last commit's message (or
 *     `null` on an unborn HEAD) to pre-fill the textarea, turning it off is fire-and-forget.
 * }} handlers */
export function createCommitPanel({ onCommit, onAmendToggle } = {}) {
  const el = document.createElement("div");
  el.className = "sc-panel";
  el.innerHTML = `
    <textarea class="sc-textarea" placeholder="Commit message"></textarea>
    <div class="sc-stats"></div>
    <label class="cb-opt sc-toggle">
      <input type="checkbox" class="sc-amend" />
      <span>Amend last commit</span>
    </label>
    <label class="cb-opt sc-toggle" title="">
      <input type="checkbox" class="sc-sign" />
      <span>SSH sign</span>
    </label>
    <label class="cb-opt sc-toggle" title="Push isn't wired up yet.">
      <input type="checkbox" class="sc-push" disabled />
      <span>Push after commit</span>
    </label>
    <div class="sc-meta">
      <div class="sc-row"><span class="sc-lbl">Author</span><span class="sc-val sc-author-val"></span></div>
    </div>
    <div class="btn btn-blue sc-commit">commit</div>
  `;

  const textarea = el.querySelector(".sc-textarea");
  const statsEl = el.querySelector(".sc-stats");
  const amendCheckbox = el.querySelector(".sc-amend");
  const signCheckbox = el.querySelector(".sc-sign");
  const signLabel = el.querySelectorAll(".sc-toggle")[1];
  const authorVal = el.querySelector(".sc-author-val");
  const commitBtn = el.querySelector(".sc-commit");

  let currentBranch = "";
  let draftMessage = "";

  function updateButton() {
    const amend = amendCheckbox.checked;
    commitBtn.textContent = amend ? "amend last commit" : `commit to ${currentBranch}`;
    commitBtn.classList.toggle("btn-blue", !amend);
    commitBtn.classList.toggle("btn-blue-outline", amend);
  }

  amendCheckbox.addEventListener("change", async () => {
    if (amendCheckbox.checked) {
      draftMessage = textarea.value;
      const lastMessage = await onAmendToggle?.(true);
      textarea.value = lastMessage ?? "";
    } else {
      textarea.value = draftMessage;
      onAmendToggle?.(false);
    }
    updateButton();
  });

  commitBtn.addEventListener("click", async () => {
    if (commitBtn.classList.contains("disabled")) return;
    await onCommit?.({ message: textarea.value, amend: amendCheckbox.checked, sshSign: signCheckbox.checked });
  });

  return {
    el,
    setBranch(name) {
      currentBranch = name;
      updateButton();
    },
    setAuthor(name, email) {
      authorVal.textContent = `${name} · ${email}`;
    },
    setStats({ stagedCount, additions, deletions }) {
      statsEl.textContent = stagedCount
        ? `${stagedCount} file${stagedCount === 1 ? "" : "s"} staged · +${additions} −${deletions}`
        : "No changes staged";
    },
    setCanAmend(canAmend) {
      amendCheckbox.disabled = !canAmend;
    },
    setHasSigningKey(has) {
      signCheckbox.disabled = !has;
      signLabel.title = has ? "" : "Configure git config user.signingkey to enable SSH signing.";
    },
    setMessage(msg) {
      textarea.value = msg;
    },
    getMessage() {
      return textarea.value;
    },
    setBusy(busy) {
      commitBtn.classList.toggle("disabled", busy);
    },
  };
}
