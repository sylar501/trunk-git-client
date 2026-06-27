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

export async function listBranches(repoPath) {
  return invoke("list_branches", { repoPath });
}

export async function getCommitDetail(repoPath, sha) {
  return invoke("get_commit_detail", { repoPath, sha });
}

export async function getCommitFileDiff(repoPath, sha, filePath) {
  return invoke("get_commit_file_diff", { repoPath, sha, filePath });
}

export async function cherryPickCommit(repoPath, sha) {
  return invoke("cherry_pick_commit", { repoPath, sha });
}

export async function revertCommit(repoPath, sha) {
  return invoke("revert_commit", { repoPath, sha });
}

export async function createBranchAt(repoPath, sha, name) {
  return invoke("create_branch_at", { repoPath, sha, name });
}

export async function getSettings() {
  return invoke("get_settings");
}

export async function saveSettings({ sidebarWidth, commitOverlayWidth } = {}) {
  return invoke("save_settings", { sidebarWidth, commitOverlayWidth });
}
