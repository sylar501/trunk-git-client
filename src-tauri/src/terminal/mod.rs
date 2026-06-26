//! Terminal drawer backend (PRD §4.5): native PTY sessions via `portable-pty`.
//!
//! Session 1 scaffold only — session opening/IO streaming to the frontend, cwd-follows-active-repo
//! behaviour, and multi-tab session management are implemented in the session that builds the
//! terminal drawer feature.

pub struct TerminalSession {
    pub cwd: String,
}

impl TerminalSession {
    pub fn new(cwd: String) -> Self {
        Self { cwd }
    }
}
