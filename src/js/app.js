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

export async function cloneRepository(url, destination, workspacePath) {
  return invoke("clone_repository", { url, destination, workspacePath });
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
