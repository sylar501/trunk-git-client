//! Interactive rebase plan construction + hand-rolled execution (PRD §16, SPEC.md item 10).
//!
//! `git2::Repository::rebase()`/the `git2::Rebase` struct only ever auto-generates sequential
//! `Pick` operations, in original commit order, between two refs (verified against the vendored
//! git2 0.19.0 source) — there is no API to inject a custom todo list, reorder, drop, squash, or
//! fixup. This module is therefore built entirely on `Repository::cherrypick()` (the full
//! method — writes `CHERRY_PICK_HEAD`, sets `repo.state() == CherryPick`, so a conflict here
//! plugs straight into the existing, operation-agnostic conflict resolver) plus manual commit
//! construction per step, never `git2::Rebase`.
//!
//! Restart durability is best-effort: a small sidecar JSON file under `.git/` records the
//! remaining plan + progress, written after every step transition and deleted on success/abort.
//! Its mere presence (`has_interactive_rebase_session`) is what lets a relaunch tell "this repo
//! is mid a Trunk interactive rebase" apart from a one-off cherry-pick conflict, which has no
//! sidecar at all.

use super::{commit_summaries_between, is_working_tree_clean};
use git2::{Oid, Repository};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebaseAction {
    Pick,
    Reword,
    Squash,
    Fixup,
    Edit,
    Drop,
}

/// One row of the plan — both the editable commit-list row state (§16.2) and the input to
/// execution. `new_message` is only meaningful for `Reword`; `None` means keep the original.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseStep {
    pub sha: String,
    pub short_sha: String,
    pub summary: String,
    pub author_name: String,
    pub action: RebaseAction,
    pub new_message: Option<String>,
}

/// The plan as built by `build_plan` from a target ref, then freely reordered/recoloured/
/// reworded client-side before "Begin Rebase" sends it back whole. `steps` is oldest-first
/// (execution order), matching `git rebase -i`'s own listing convention (§16.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebasePlan {
    pub onto_sha: String,
    pub onto_display_name: String,
    pub branch_name: String,
    pub steps: Vec<RebaseStep>,
}

/// Sidecar progress, written to `.git/trunk-rebase-sidecar.json`. `remaining_steps` is
/// oldest-first (execution order) — already-applied steps are removed from the front as they
/// complete. `total_steps` is fixed at plan-build time (post-drop-filter) so "step N of M"
/// banners can be computed without losing the original count as steps are consumed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseSidecar {
    pub onto_sha: String,
    pub branch_name: String,
    pub total_steps: usize,
    pub remaining_steps: Vec<RebaseStep>,
    /// The commit everything executed so far now sits on top of — starts as `onto_sha`, advances
    /// after every pick/reword/edit, is reassigned (not advanced) by squash/fixup.
    pub current_tip: String,
    /// `Some(sha)` while paused on an `edit` step (that commit's own sha) — cleared by
    /// `resume_after_edit`.
    pub paused_for_edit: Option<String>,
    /// Messages folded by squash/fixup so far, oldest-absorbed-first — informational only (the
    /// frontend's own preview mirrors this independently); not read back by commit construction,
    /// which always derives the combined message from the previous commit's own message instead.
    pub pending_absorbed_messages: Vec<String>,
}

/// Outcome of driving the loop to its next stopping point.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StepOutcome {
    Finished { final_sha: String },
    Conflict { sidecar: RebaseSidecar },
    PausedForEdit { sidecar: RebaseSidecar },
}

/// Internal-only signal from `finish_step` — never surfaces past this module (the public
/// `StepOutcome` is what callers see).
enum FinishResult {
    Continue,
    Paused,
}

// --- Plan construction -------------------------------------------------------------------------

/// Builds the initial plan (all-Pick) from `onto_ref` (any revparse-able branch/tag/sha) to
/// HEAD. Refuses if HEAD is detached (rebase moves a branch ref, so one must be checked out) or
/// if the range contains a merge commit (cherry-picking a merge isn't supported — same refusal
/// `cherry_pick`/`revert_commit` already make).
pub fn build_plan(repo: &Repository, onto_ref: &str) -> Result<RebasePlan, String> {
    let head_ref = repo.head().map_err(|e| e.to_string())?;
    let branch_name = head_ref
        .shorthand()
        .filter(|_| head_ref.is_branch())
        .ok_or_else(|| "Cannot rebase: not currently on a branch.".to_string())?
        .to_string();
    let head_oid = head_ref.target().ok_or_else(|| "Branch has no commits yet.".to_string())?;

    let onto_object = repo.revparse_single(onto_ref).map_err(|e| e.to_string())?;
    let onto_commit = onto_object.peel_to_commit().map_err(|e| e.to_string())?;
    let onto_oid = onto_commit.id();

    let summaries = commit_summaries_between(repo, head_oid, Some(onto_oid))?;
    let mut steps = Vec::with_capacity(summaries.len());
    for summary in summaries {
        let oid = Oid::from_str(&summary.sha).map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        if commit.parent_count() > 1 {
            return Err("Rebasing a range that includes a merge commit isn't supported yet.".to_string());
        }
        steps.push(RebaseStep {
            sha: summary.sha,
            short_sha: summary.short_sha,
            summary: summary.summary,
            author_name: summary.author_name,
            action: RebaseAction::Pick,
            new_message: None,
        });
    }
    // `commit_summaries_between` walks newest-first (git log order); flip to oldest-first
    // so the plan matches `git rebase -i`'s own display convention.
    steps.reverse();

    Ok(RebasePlan {
        onto_sha: onto_oid.to_string(),
        onto_display_name: onto_ref.to_string(),
        branch_name,
        steps,
    })
}

// --- Execution -----------------------------------------------------------------------------

/// Points HEAD (detached) and the working tree at `tip_sha` — every step's commit is created via
/// `repo.commit(Some("HEAD"), ...)`, which keeps a detached HEAD in lock-step automatically, so
/// this only needs to run once per execution entry point rather than before every step.
fn sync_head_to_tip(repo: &Repository, tip_sha: &str) -> Result<(), String> {
    let oid = Oid::from_str(tip_sha).map_err(|e| e.to_string())?;
    repo.set_head_detached(oid).map_err(|e| e.to_string())?;
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_head(Some(&mut checkout_opts)).map_err(|e| e.to_string())
}

/// Creates a commit (no ref update — see the caller's note on why) and moves the detached HEAD
/// to it directly via `set_head_detached`, which has no such check.
fn commit_and_advance_head(
    repo: &Repository,
    author: &git2::Signature,
    committer: &git2::Signature,
    message: &str,
    tree: &git2::Tree,
    parents: &[&git2::Commit],
) -> Result<Oid, String> {
    let new_oid = repo
        .commit(None, author, committer, message, tree, parents)
        .map_err(|e| e.to_string())?;
    repo.set_head_detached(new_oid).map_err(|e| e.to_string())?;
    Ok(new_oid)
}

/// Applies one step's commit-construction logic once its index is conflict-free — shared by the
/// fresh-cherry-pick path in `run_remaining_steps` and the post-conflict-resolution resume path,
/// since a step whose conflicts just got resolved needs exactly the same "now finish this step's
/// action" logic as one that applied cleanly to begin with.
///
/// Every commit here is created via `commit_and_advance_head` (ref param `None`, HEAD moved
/// manually afterward) rather than `repo.commit(Some("HEAD"), ...)`: libgit2's ref-updating
/// commit path enforces a compare-and-swap — the *named ref's current target* must equal the new
/// commit's first parent — which holds for pick/reword/edit (parent is always `current_tip`,
/// which is exactly what HEAD already points to) but not for squash/fixup, whose new commit's
/// parent is `current_tip`'s own *grandparent* (skipping over it). Decoupling commit creation
/// from the HEAD move sidesteps that check uniformly, instead of special-casing it per action.
fn finish_step(repo: &Repository, sidecar: &mut RebaseSidecar, step: &RebaseStep) -> Result<FinishResult, String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
    let committer = repo.signature().map_err(|e| e.to_string())?;
    let original_commit = repo
        .find_commit(Oid::from_str(&step.sha).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    match step.action {
        RebaseAction::Drop => unreachable!("drop steps are filtered out before execution begins"),
        RebaseAction::Pick | RebaseAction::Edit => {
            let parent = repo
                .find_commit(Oid::from_str(&sidecar.current_tip).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let author = original_commit.author();
            let message = original_commit.message().unwrap_or_default();
            let new_oid = commit_and_advance_head(repo, &author, &committer, message, &tree, &[&parent])?;
            sidecar.current_tip = new_oid.to_string();
            sidecar.pending_absorbed_messages.clear();
            repo.cleanup_state().map_err(|e| e.to_string())?;
            if step.action == RebaseAction::Edit {
                // Real `git rebase -i edit` commits the step, then stops so the user can amend
                // or add commits on top before continuing — not left uncommitted.
                sidecar.paused_for_edit = Some(new_oid.to_string());
                Ok(FinishResult::Paused)
            } else {
                Ok(FinishResult::Continue)
            }
        }
        RebaseAction::Reword => {
            let parent = repo
                .find_commit(Oid::from_str(&sidecar.current_tip).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let author = original_commit.author();
            let message = step.new_message.as_deref().unwrap_or_else(|| original_commit.message().unwrap_or_default());
            let new_oid = commit_and_advance_head(repo, &author, &committer, message, &tree, &[&parent])?;
            sidecar.current_tip = new_oid.to_string();
            sidecar.pending_absorbed_messages.clear();
            repo.cleanup_state().map_err(|e| e.to_string())?;
            Ok(FinishResult::Continue)
        }
        RebaseAction::Squash | RebaseAction::Fixup => {
            let prev_tip = repo
                .find_commit(Oid::from_str(&sidecar.current_tip).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            if prev_tip.parent_count() > 1 {
                return Err("Cannot squash onto a merge commit.".to_string());
            }
            let grandparents: Vec<git2::Commit> = prev_tip.parents().collect();
            let grandparent_refs: Vec<&git2::Commit> = grandparents.iter().collect();
            let this_message = original_commit.message().unwrap_or_default().to_string();
            let prev_message = prev_tip.message().unwrap_or_default().to_string();
            let combined_message = match step.action {
                RebaseAction::Squash => format!("{prev_message}\n\n{this_message}"),
                _ => prev_message,
            };
            sidecar.pending_absorbed_messages.push(this_message);
            // Keep the original (earliest) author in the chain, matching plain `git rebase -i`'s
            // own squash/fixup behaviour.
            let author = prev_tip.author();
            let new_oid =
                commit_and_advance_head(repo, &author, &committer, &combined_message, &tree, &grandparent_refs)?;
            sidecar.current_tip = new_oid.to_string();
            repo.cleanup_state().map_err(|e| e.to_string())?;
            Ok(FinishResult::Continue)
        }
    }
}

/// Writes the final branch ref to `current_tip` and checks it out — the one point a real branch
/// (rather than the detached HEAD every intermediate step used) is touched, mirroring how plain
/// `git rebase` only fixes up the branch ref at the very end.
fn finalize(repo: &Repository, sidecar: &RebaseSidecar) -> Result<(), String> {
    let tip_oid = Oid::from_str(&sidecar.current_tip).map_err(|e| e.to_string())?;
    let refname = format!("refs/heads/{}", sidecar.branch_name);
    repo.reference(&refname, tip_oid, true, "interactive rebase: finish")
        .map_err(|e| e.to_string())?;
    repo.set_head(&refname).map_err(|e| e.to_string())?;
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_head(Some(&mut checkout_opts)).map_err(|e| e.to_string())?;
    repo.cleanup_state().map_err(|e| e.to_string())
}

/// Cherry-picks + finishes every remaining step in order, stopping at the first conflict or
/// edit-pause. Assumes HEAD/the working tree already match `sidecar.current_tip` (callers that
/// haven't just finished a step in-process — i.e. fresh begin/restart-resume — must call
/// `sync_head_to_tip` first; `resume_after_conflict_resolution`/`resume_after_edit` already left
/// HEAD in the right place via their own commit, so they skip straight to this).
fn run_remaining_steps(repo: &Repository, mut sidecar: RebaseSidecar) -> Result<StepOutcome, String> {
    loop {
        let Some(step) = sidecar.remaining_steps.first().cloned() else {
            finalize(repo, &sidecar)?;
            delete_sidecar(repo);
            return Ok(StepOutcome::Finished { final_sha: sidecar.current_tip.clone() });
        };

        let oid = Oid::from_str(&step.sha).map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.conflict_style_diff3(true);
        let mut cherrypick_opts = git2::CherrypickOptions::new();
        cherrypick_opts.checkout_builder(checkout_builder);
        repo.cherrypick(&commit, Some(&mut cherrypick_opts)).map_err(|e| e.to_string())?;

        let index = repo.index().map_err(|e| e.to_string())?;
        if index.has_conflicts() {
            write_sidecar(repo, &sidecar)?;
            return Ok(StepOutcome::Conflict { sidecar });
        }

        match finish_step(repo, &mut sidecar, &step)? {
            FinishResult::Continue => {
                sidecar.remaining_steps.remove(0);
                write_sidecar(repo, &sidecar)?;
            }
            FinishResult::Paused => {
                sidecar.remaining_steps.remove(0);
                write_sidecar(repo, &sidecar)?;
                return Ok(StepOutcome::PausedForEdit { sidecar });
            }
        }
    }
}

/// Drives a freshly-built sidecar (fresh "Begin Rebase", or a restart-resume that hasn't run in
/// this process yet) to its next stopping point.
pub fn drive_interactive_rebase(repo: &Repository, sidecar: RebaseSidecar) -> Result<StepOutcome, String> {
    sync_head_to_tip(repo, &sidecar.current_tip)?;
    run_remaining_steps(repo, sidecar)
}

/// Builds the initial sidecar from a (post-edit) `RebasePlan` and drives it. `Drop` steps are
/// filtered out here, once — never skipped at execution time. Refuses a dirty working tree (the
/// loop repeatedly force-checks-out, which would discard uncommitted changes) and a plan whose
/// first surviving step is `Squash`/`Fixup` (nothing to squash into).
pub fn begin_interactive_rebase(repo: &Repository, plan: RebasePlan) -> Result<StepOutcome, String> {
    if !is_working_tree_clean(repo).map_err(|e| e.to_string())? {
        return Err("Commit or stash your changes before rebasing.".to_string());
    }
    let remaining_steps: Vec<RebaseStep> = plan
        .steps
        .into_iter()
        .filter(|s| s.action != RebaseAction::Drop)
        .collect();
    if matches!(remaining_steps.first().map(|s| s.action), Some(RebaseAction::Squash) | Some(RebaseAction::Fixup)) {
        return Err("Cannot squash/fixup the first commit in the rebase — nothing to squash into.".to_string());
    }
    let sidecar = RebaseSidecar {
        onto_sha: plan.onto_sha.clone(),
        branch_name: plan.branch_name,
        total_steps: remaining_steps.len(),
        remaining_steps,
        current_tip: plan.onto_sha,
        paused_for_edit: None,
        pending_absorbed_messages: Vec::new(),
    };
    write_sidecar(repo, &sidecar)?;
    drive_interactive_rebase(repo, sidecar)
}

/// Restart-resume entry point: loads the sidecar from disk and continues it.
pub fn resume_interactive_rebase(repo: &Repository) -> Result<StepOutcome, String> {
    let sidecar = read_sidecar(repo).ok_or_else(|| "No rebase session to resume.".to_string())?;
    drive_interactive_rebase(repo, sidecar)
}

/// Re-entry point from `finish_conflict_resolution`'s sidecar-aware branch: the index has just
/// been written conflict-free by the frontend's resolved files. Finishes that step (no fresh
/// cherry-pick needed — it already happened before the conflict), then continues for the rest.
pub fn resume_after_conflict_resolution(repo: &Repository, mut sidecar: RebaseSidecar) -> Result<StepOutcome, String> {
    let step = sidecar
        .remaining_steps
        .first()
        .cloned()
        .ok_or_else(|| "No pending rebase step to resume.".to_string())?;
    match finish_step(repo, &mut sidecar, &step)? {
        FinishResult::Continue => {
            sidecar.remaining_steps.remove(0);
            write_sidecar(repo, &sidecar)?;
            run_remaining_steps(repo, sidecar)
        }
        FinishResult::Paused => {
            sidecar.remaining_steps.remove(0);
            write_sidecar(repo, &sidecar)?;
            Ok(StepOutcome::PausedForEdit { sidecar })
        }
    }
}

/// "Continue rebase" after an edit-pause: the caller has already amended HEAD, added commits on
/// top, or left it untouched via the normal staging/commit UI. Reads whatever HEAD now is as the
/// new `current_tip` (no forced amend — matches real `git rebase -i edit`, which allows several
/// commits before continuing) and resumes.
pub fn resume_after_edit(repo: &Repository, mut sidecar: RebaseSidecar) -> Result<StepOutcome, String> {
    let head_oid = repo
        .head()
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?
        .id();
    sidecar.current_tip = head_oid.to_string();
    sidecar.paused_for_edit = None;
    write_sidecar(repo, &sidecar)?;
    run_remaining_steps(repo, sidecar)
}

/// Aborts an in-progress interactive rebase. Deliberately does **not** reuse the shared
/// `abort_in_progress_operation` cherry-pick/revert abort helper, which resets to *current*
/// HEAD — correct for plain cherry-pick/revert (which never moves HEAD away from the original
/// branch tip), but wrong here: every step detaches HEAD onto the rebase's own chain, so
/// resetting to "current HEAD" would leave the user on a detached HEAD partway through the
/// rebase instead of back on their original branch. The branch ref itself is never touched
/// until `finalize` (success only), so checking it back out is always safe and exact.
pub fn abort_interactive_rebase(repo: &Repository) -> Result<(), String> {
    let sidecar = read_sidecar(repo).ok_or_else(|| "No rebase session to abort.".to_string())?;
    let refname = format!("refs/heads/{}", sidecar.branch_name);
    repo.set_head(&refname).map_err(|e| e.to_string())?;
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_head(Some(&mut checkout_opts)).map_err(|e| e.to_string())?;
    repo.cleanup_state().map_err(|e| e.to_string())?;
    delete_sidecar(repo);
    Ok(())
}

// --- Sidecar file I/O ------------------------------------------------------------------------

fn sidecar_path(repo: &Repository) -> std::path::PathBuf {
    repo.path().join("trunk-rebase-sidecar.json")
}

pub fn read_sidecar(repo: &Repository) -> Option<RebaseSidecar> {
    let content = std::fs::read_to_string(sidecar_path(repo)).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_sidecar(repo: &Repository, sidecar: &RebaseSidecar) -> Result<(), String> {
    let content = serde_json::to_string(sidecar).map_err(|e| e.to_string())?;
    std::fs::write(sidecar_path(repo), content).map_err(|e| e.to_string())
}

pub fn delete_sidecar(repo: &Repository) {
    let _ = std::fs::remove_file(sidecar_path(repo));
}

/// The restart-detection predicate (decision 2): a sidecar's mere presence means "this repo is
/// mid a Trunk interactive rebase" — covering every paused state (fresh conflict, edit-pause
/// with `repo.state()` already back to `Clean`, or even the brief between-steps window), unlike
/// gating on `repo.state() == CherryPick` alone, which a plain one-off cherry-pick conflict also
/// satisfies despite having no sidecar at all.
pub fn has_interactive_rebase_session(repo: &Repository) -> bool {
    sidecar_path(repo).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> (std::path::PathBuf, Repository) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "trunk-rebase-plan-test-{n}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        (dir, repo)
    }

    fn commit_file(repo: &Repository, dir: &std::path::Path, message: &str, file_name: &str, content: &str) -> Oid {
        std::fs::write(dir.join(file_name), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(file_name)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .and_then(|oid| repo.find_commit(oid).ok())
            .into_iter()
            .collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs).unwrap()
    }

    fn step_for(repo: &Repository, sha: Oid, action: RebaseAction) -> RebaseStep {
        let commit = repo.find_commit(sha).unwrap();
        let summary = commit.summary().unwrap_or_default().to_string();
        let author_name = commit.author().name().unwrap_or_default().to_string();
        RebaseStep {
            sha: sha.to_string(),
            short_sha: sha.to_string()[..7].to_string(),
            summary,
            author_name,
            action,
            new_message: None,
        }
    }

    fn plan_with(onto: Oid, branch: &str, steps: Vec<RebaseStep>) -> RebasePlan {
        RebasePlan {
            onto_sha: onto.to_string(),
            onto_display_name: onto.to_string(),
            branch_name: branch.to_string(),
            steps,
        }
    }

    #[test]
    fn pick_replays_commit_with_original_author_and_message() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        let steps = vec![step_for(&repo, c1, RebaseAction::Pick), step_for(&repo, c2, RebaseAction::Pick)];
        let plan = plan_with(root, "master", steps);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.message(), Some("second"));
        assert_eq!(tip.parent(0).unwrap().message(), Some("first"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reword_uses_edited_message_not_original() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let mut step = step_for(&repo, c1, RebaseAction::Reword);
        step.new_message = Some("reworded message".to_string());
        let plan = plan_with(root, "master", vec![step]);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.message(), Some("reworded message"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn drop_omits_commit_from_final_history() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        let steps = vec![step_for(&repo, c1, RebaseAction::Drop), step_for(&repo, c2, RebaseAction::Pick)];
        let plan = plan_with(root, "master", steps);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.message(), Some("second"));
        assert_eq!(tip.parent(0).unwrap().id(), root, "first should be dropped, parent should be root directly");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn squash_folds_into_previous_commit_combining_messages() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        let steps = vec![step_for(&repo, c1, RebaseAction::Pick), step_for(&repo, c2, RebaseAction::Squash)];
        let plan = plan_with(root, "master", steps);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.parent(0).unwrap().id(), root, "squash should replace, not stack on top of, the previous commit");
        let message = tip.message().unwrap_or_default();
        assert!(message.contains("first") && message.contains("second"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fixup_folds_into_previous_commit_keeping_only_previous_message() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        let steps = vec![step_for(&repo, c1, RebaseAction::Pick), step_for(&repo, c2, RebaseAction::Fixup)];
        let plan = plan_with(root, "master", steps);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.message(), Some("first"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn multiple_consecutive_squashes_fold_correctly() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        let c3 = commit_file(&repo, &dir, "third", "d.txt", "1");
        let steps = vec![
            step_for(&repo, c1, RebaseAction::Pick),
            step_for(&repo, c2, RebaseAction::Squash),
            step_for(&repo, c3, RebaseAction::Squash),
        ];
        let plan = plan_with(root, "master", steps);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.parent_count(), 1);
        assert_eq!(tip.parent(0).unwrap().id(), root);
        let message = tip.message().unwrap_or_default();
        assert!(message.contains("first") && message.contains("second") && message.contains("third"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reorder_changes_replay_order_and_resulting_tree() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        // c1 and c2 are siblings (both parented on `root`, touching different files) rather than
        // a chain — reordering two commits that touch the *same* line would conflict on replay
        // regardless of order (cherry-pick's merge ancestor is always each commit's own real
        // parent), which would only prove conflict detection, not reordering. Non-overlapping
        // sibling edits isolate the thing this test actually checks: which commit ends up as the
        // *newer* one in the final chain.
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        repo.reset(&repo.find_commit(root).unwrap().into_object(), git2::ResetType::Hard, None).unwrap();
        let c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        // Plan lists c2 before c1 (oldest-first) — the *opposite* of creation order in this
        // test (c1 was made on `root` first, then c2 also on `root` as a sibling) — so c1 ends
        // up newest in the replayed history, proving execution follows plan order (c2 first →
        // older; c1 second → newer) rather than original commit-graph order.
        let steps = vec![step_for(&repo, c2, RebaseAction::Pick), step_for(&repo, c1, RebaseAction::Pick)];
        let plan = plan_with(root, "master", steps);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.message(), Some("first"), "last-in-plan commit should end up newest");
        assert_eq!(tip.parent(0).unwrap().message(), Some("second"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn edit_step_commits_then_pauses() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let plan = plan_with(root, "master", vec![step_for(&repo, c1, RebaseAction::Edit)]);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::PausedForEdit { sidecar } = outcome else { panic!("expected PausedForEdit") };
        assert!(sidecar.paused_for_edit.is_some());
        assert!(sidecar.remaining_steps.is_empty());
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message(), Some("first"), "edit step should already be committed while paused");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn conflict_mid_step_stops_with_step_still_pending() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "base\n");
        // Two divergent edits to the same line, both descending from `root` directly via
        // explicit tree surgery so they conflict when cherry-picked onto each other's sibling.
        let onto = commit_file(&repo, &dir, "onto-edit", "a.txt", "onto-version\n");
        // Reset back to root before making the sibling commit, so `c1`'s parent is `root`, not `onto`.
        repo.reset(&repo.find_commit(root).unwrap().into_object(), git2::ResetType::Hard, None).unwrap();
        let c1 = commit_file(&repo, &dir, "branch-edit", "a.txt", "branch-version\n");
        let plan = plan_with(onto, "master", vec![step_for(&repo, c1, RebaseAction::Pick)]);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Conflict { sidecar } = outcome else { panic!("expected Conflict") };
        assert_eq!(sidecar.remaining_steps.len(), 1, "conflicting step should still be pending, not skipped");
        assert!(repo.index().unwrap().has_conflicts());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resume_after_conflict_resolution_commits_with_squash_semantics() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "base\n");
        let onto = commit_file(&repo, &dir, "onto-edit", "a.txt", "onto-version\n");
        repo.reset(&repo.find_commit(root).unwrap().into_object(), git2::ResetType::Hard, None).unwrap();
        let c1 = commit_file(&repo, &dir, "branch-edit", "a.txt", "branch-version\n");
        // `root` itself can't be re-picked (it's the rebase base here) — a pick step establishes
        // a "previous" commit first, then the squash step folds onto it.
        let base_for_pick = commit_file(&repo, &dir, "pick-target", "b.txt", "1\n");
        let plan = plan_with(
            onto,
            "master",
            vec![step_for(&repo, base_for_pick, RebaseAction::Pick), step_for(&repo, c1, RebaseAction::Squash)],
        );
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Conflict { sidecar } = outcome else { panic!("expected Conflict on the squash step") };

        // Resolve conflicts the same way `finish_conflict_resolution` would: write the desired
        // final content and stage it.
        std::fs::write(dir.join("a.txt"), "resolved\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();

        let outcome = resume_after_conflict_resolution(&repo, sidecar).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = repo.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        // Squash replaced the previous commit, so there should be exactly one commit between
        // `onto` and the tip (the squash target's pick step folded the squash step on top).
        assert_eq!(tip.parent(0).unwrap().id(), onto);
        let message = tip.message().unwrap_or_default();
        assert!(message.contains("pick-target") && message.contains("branch-edit"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn abort_interactive_rebase_restores_original_head_and_deletes_sidecar() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "base\n");
        let onto = commit_file(&repo, &dir, "onto-edit", "a.txt", "onto-version\n");
        repo.reset(&repo.find_commit(root).unwrap().into_object(), git2::ResetType::Hard, None).unwrap();
        let c1 = commit_file(&repo, &dir, "branch-edit", "a.txt", "branch-version\n");
        let original_head = repo.head().unwrap().target().unwrap();
        let plan = plan_with(onto, "master", vec![step_for(&repo, c1, RebaseAction::Pick)]);
        let _ = begin_interactive_rebase(&repo, plan).unwrap(); // conflicts, leaves sidecar on disk
        assert!(super::has_interactive_rebase_session(&repo));

        abort_interactive_rebase(&repo).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert_eq!(repo.head().unwrap().target().unwrap(), original_head);
        assert!(!super::has_interactive_rebase_session(&repo));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restart_resume_reads_sidecar_and_continues_remaining_steps() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        // Simulate a restart: write a sidecar directly to disk (bypassing begin_interactive_rebase)
        // and a fresh `Repository::open` to drive it, proving resume doesn't depend on in-memory state.
        let sidecar = RebaseSidecar {
            onto_sha: root.to_string(),
            branch_name: "master".to_string(),
            total_steps: 1,
            remaining_steps: vec![step_for(&repo, c1, RebaseAction::Pick)],
            current_tip: root.to_string(),
            paused_for_edit: None,
            pending_absorbed_messages: Vec::new(),
        };
        write_sidecar(&repo, &sidecar).unwrap();
        let reopened = Repository::open(&dir).unwrap();
        let outcome = resume_interactive_rebase(&reopened).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        let tip = reopened.find_commit(Oid::from_str(&final_sha).unwrap()).unwrap();
        assert_eq!(tip.message(), Some("first"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn has_interactive_rebase_session_false_for_plain_cherry_pick_conflict() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "base\n");
        let onto = commit_file(&repo, &dir, "onto-edit", "a.txt", "onto-version\n");
        repo.reset(&repo.find_commit(root).unwrap().into_object(), git2::ResetType::Hard, None).unwrap();
        let c1 = commit_file(&repo, &dir, "branch-edit", "a.txt", "branch-version\n");
        repo.reset(&repo.find_commit(onto).unwrap().into_object(), git2::ResetType::Hard, None).unwrap();

        let commit = repo.find_commit(c1).unwrap();
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.conflict_style_diff3(true);
        let mut opts = git2::CherrypickOptions::new();
        opts.checkout_builder(checkout_builder);
        repo.cherrypick(&commit, Some(&mut opts)).unwrap();
        assert!(repo.index().unwrap().has_conflicts());
        assert_eq!(repo.state(), git2::RepositoryState::CherryPick);

        assert!(!has_interactive_rebase_session(&repo), "a plain cherry-pick conflict has no sidecar");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn all_dropped_plan_produces_no_new_commits_tip_equals_onto() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let plan = plan_with(root, "master", vec![step_for(&repo, c1, RebaseAction::Drop)]);
        let outcome = begin_interactive_rebase(&repo, plan).unwrap();
        let StepOutcome::Finished { final_sha } = outcome else { panic!("expected Finished") };
        assert_eq!(final_sha, root.to_string());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn build_plan_orders_steps_oldest_first_matching_git_rebase_i_convention() {
        let (dir, repo) = temp_repo();
        let root = commit_file(&repo, &dir, "root", "a.txt", "1");
        let c1 = commit_file(&repo, &dir, "first", "b.txt", "1");
        let _c2 = commit_file(&repo, &dir, "second", "c.txt", "1");
        repo.branch("base", &repo.find_commit(root).unwrap(), false).unwrap();
        let result = build_plan(&repo, "refs/heads/base").unwrap();
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].sha, c1.to_string(), "oldest commit should be listed first");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
