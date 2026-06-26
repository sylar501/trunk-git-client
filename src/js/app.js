// Bootstrap + Tauri invoke wrappers. Session 1 scaffold only — no screen logic yet.

const invoke = window.__TAURI__?.core?.invoke;

export async function openRepository(path) {
  return invoke("open_repository", { path });
}

export async function openWorkspace(path) {
  return invoke("open_workspace", { path });
}
