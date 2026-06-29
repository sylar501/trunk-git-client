// Bootstrap + Tauri invoke wrappers.

const invoke = window.__TAURI__?.core?.invoke;

export async function openRepository(path) {
  return invoke("open_repository", { path });
}

export async function openWorkspace(path) {
  return invoke("open_workspace", { path });
}

export async function listRecent() {
  return invoke("list_recent");
}

export async function removeRecent(path) {
  return invoke("remove_recent", { path });
}

export async function detectNestedRepos(path) {
  return invoke("detect_nested_repos", { path });
}

export async function createWorkspace(name, directory, initialRepos) {
  return invoke("create_workspace", { name, directory, initialRepos });
}

export async function promoteToWorkspace(name, directory) {
  return invoke("promote_to_workspace", { name, directory });
}

export async function cloneRepository(url, destination, workspaceAction) {
  return invoke("clone_repository", { url, destination, workspaceAction });
}

export async function defaultDirectory() {
  return invoke("default_directory");
}

export async function onCloneProgress(handler) {
  return window.__TAURI__.event.listen("clone-progress", (event) => handler(event.payload));
}

export async function pickFolder() {
  return window.__TAURI__.dialog.open({ directory: true });
}

export async function switchActiveRepository(repoPath) {
  return invoke("switch_active_repository", { repoPath });
}

export async function addExistingRepository(repoPath) {
  return invoke("add_existing_repository", { repoPath });
}

export async function getAppState() {
  return invoke("get_app_state");
}

export async function repoQuickInfo(path) {
  return invoke("repo_quick_info", { path });
}

export async function onDragDrop(handler) {
  const webview = window.__TAURI__.webview.getCurrentWebview();
  return webview.onDragDropEvent((event) => handler(event.payload));
}

export async function openGraph(repoPath) {
  return invoke("open_graph", { repoPath });
}

export async function getGraphRows(repoPath, start, count, filter) {
  return invoke("get_graph_rows", { repoPath, start, count, filter });
}

export async function getCommitIndex(repoPath, sha) {
  return invoke("get_commit_index", { repoPath, sha });
}

export async function listBranches(repoPath) {
  return invoke("list_branches", { repoPath });
}

export async function getCommitDetail(repoPath, sha) {
  return invoke("get_commit_detail", { repoPath, sha });
}

export async function getCommitFileDiff(repoPath, sha, filePath) {
  return invoke("get_commit_file_diff", { repoPath, sha, filePath });
}

export async function cherryPickCommit(repoPath, sha, noCommit = false) {
  return invoke("cherry_pick_commit", { repoPath, sha, noCommit });
}

export async function revertCommit(repoPath, sha, noCommit = false) {
  return invoke("revert_commit", { repoPath, sha, noCommit });
}

export async function createBranchAt(repoPath, sha, name) {
  return invoke("create_branch_at", { repoPath, sha, name });
}

export async function getConflictStatus(repoPath) {
  return invoke("get_conflict_status", { repoPath });
}

export async function getConflictFile(repoPath, filePath) {
  return invoke("get_conflict_file", { repoPath, filePath });
}

export async function finishConflictResolution(repoPath, files) {
  return invoke("finish_conflict_resolution", { repoPath, files });
}

export async function abortConflictResolution(repoPath) {
  return invoke("abort_conflict_resolution", { repoPath });
}

export async function getSettings() {
  return invoke("get_settings");
}

export async function saveSettings({ sidebarWidth, commitOverlayWidth, stagingFilesWidth, resolveMergedHeight } = {}) {
  return invoke("save_settings", { sidebarWidth, commitOverlayWidth, stagingFilesWidth, resolveMergedHeight });
}

export async function getWorkingTreeStatus(repoPath) {
  return invoke("get_working_tree_status", { repoPath });
}

export async function getWorkingFileDiff(repoPath, filePath) {
  return invoke("get_working_file_diff", { repoPath, filePath });
}

export async function stageFile(repoPath, filePath) {
  return invoke("stage_file", { repoPath, filePath });
}

export async function unstageFile(repoPath, filePath) {
  return invoke("unstage_file", { repoPath, filePath });
}

export async function stageHunk(repoPath, filePath, newStart) {
  return invoke("stage_hunk", { repoPath, filePath, newStart });
}

export async function unstageHunk(repoPath, filePath, oldStart) {
  return invoke("unstage_hunk", { repoPath, filePath, oldStart });
}

export async function stageLine(repoPath, filePath, newStart, lineIndexInHunk) {
  return invoke("stage_line", { repoPath, filePath, newStart, lineIndexInHunk });
}

export async function unstageLine(repoPath, filePath, oldStart, lineIndexInHunk) {
  return invoke("unstage_line", { repoPath, filePath, oldStart, lineIndexInHunk });
}

export async function getLastCommitMessage(repoPath) {
  return invoke("get_last_commit_message", { repoPath });
}

export async function commitChanges(repoPath, message, amend, sshSign) {
  return invoke("commit_changes", { repoPath, message, amend, sshSign });
}

// --- Push / Fetch / Pull (PRD §12, SPEC.md item 7) ----------------------------------------

export async function listRemotes(repoPath) {
  return invoke("list_remotes", { repoPath });
}

export async function getRemoteUrl(repoPath, remoteName) {
  return invoke("get_remote_url", { repoPath, remoteName });
}

export async function listBranchesWithTracking(repoPath) {
  return invoke("list_branches_with_tracking", { repoPath });
}

export async function listCommitsAhead(repoPath, localBranch, remoteName, remoteBranch) {
  return invoke("list_commits_ahead", { repoPath, localBranch, remoteName, remoteBranch });
}

export async function listCommitsBehind(repoPath, localBranch, remoteName, remoteBranch) {
  return invoke("list_commits_behind", { repoPath, localBranch, remoteName, remoteBranch });
}

export async function pushBranch(
  repoPath,
  localBranch,
  remoteName,
  remoteBranch,
  setUpstream,
  force,
  forceWithLease
) {
  return invoke("push_branch", {
    repoPath,
    localBranch,
    remoteName,
    remoteBranch,
    setUpstream,
    force,
    forceWithLease,
  });
}

export async function onPushProgress(handler) {
  return window.__TAURI__.event.listen("push-progress", (event) => handler(event.payload));
}

export async function fetchRemote(repoPath, remoteName, prune, tags, submodules) {
  return invoke("fetch_remote", { repoPath, remoteName, prune, tags, submodules });
}

export async function onFetchProgress(handler) {
  return window.__TAURI__.event.listen("fetch-progress", (event) => handler(event.payload));
}

export async function pullBranch(repoPath, localBranch, remoteName, remoteBranch, strategy) {
  return invoke("pull_branch", { repoPath, localBranch, remoteName, remoteBranch, strategy });
}

// --- Branch dialogs (PRD §13, SPEC.md item 8) ---------------------------------------------

export async function listBranchesForSwitch(repoPath) {
  return invoke("list_branches_for_switch", { repoPath });
}

/** @param {{ remoteName?: string, remoteBranch?: string, dirtyStrategy?: "stash"|"carry" }} opts */
export async function checkoutBranch(repoPath, name, { remoteName, remoteBranch, dirtyStrategy } = {}) {
  return invoke("checkout_branch", {
    repoPath,
    name,
    remoteName: remoteName ?? null,
    remoteBranch: remoteBranch ?? null,
    dirtyStrategy: dirtyStrategy ?? null,
  });
}

export async function getBranchDeleteInfo(repoPath, name) {
  return invoke("get_branch_delete_info", { repoPath, name });
}

export async function renameBranch(repoPath, oldName, newName) {
  return invoke("rename_branch", { repoPath, oldName, newName });
}

export async function deleteBranch(repoPath, name, force, alsoDeleteRemote) {
  return invoke("delete_branch", { repoPath, name, force, alsoDeleteRemote });
}
