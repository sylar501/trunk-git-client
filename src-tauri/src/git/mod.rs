//! libgit2 (git2-rs) wrappers — repo access, graph walk, diff, staging, branch, remote,
//! stash, tag, rebase, conflict resolution.
//!
//! Session 1 scaffold only: defines the surface future sessions implement against.
//! Each PRD feature (commit graph §7, staging §8, conflict resolver §9, branch dialogs §13,
//! tag manager §14, push/fetch/pull §12, interactive rebase §16, remote management §18) gets
//! its own implementation pass in the session that builds that screen — see SPEC.md.

use git2::Repository;

pub struct Repo {
    inner: Repository,
}

impl Repo {
    pub fn open(path: &str) -> Result<Self, git2::Error> {
        let inner = Repository::open(path)?;
        Ok(Self { inner })
    }

    pub fn path(&self) -> &std::path::Path {
        self.inner.path()
    }
}
