//! libgit2 (git2-rs) wrappers — repo access, graph walk, diff, staging, branch, remote,
//! stash, tag, rebase, conflict resolution.
//!
//! Session 3 (PRD §7) implements the commit graph: a revwalk + lane-assignment pass run
//! once per repo and cached (see `commands::open_graph`/`get_graph_rows`), so virtualised
//! scrolling only ever slices an in-memory `Vec` instead of re-walking history per frame.
//! Diff/staging, conflict resolver, push/pull, etc. remain stubs for their own sessions —
//! see SPEC.md.

use git2::{BranchType, Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

    pub fn build_graph(&self) -> Result<GraphCache, git2::Error> {
        build_graph(&self.inner)
    }

    pub fn list_branches(&self) -> Result<Vec<BranchInfo>, git2::Error> {
        list_branches(&self.inner)
    }

    pub fn commit_detail(&self, sha: &str) -> Result<CommitDetail, git2::Error> {
        commit_detail(&self.inner, sha)
    }

    pub fn commit_file_diff(&self, sha: &str, file_path: &str) -> Result<Vec<DiffLineRow>, git2::Error> {
        commit_file_diff(&self.inner, sha, file_path)
    }

    pub fn cherry_pick(&self, sha: &str, no_commit: bool) -> Result<ConflictableOutcome, String> {
        cherry_pick(&self.inner, sha, no_commit)
    }

    pub fn revert_commit(&self, sha: &str, no_commit: bool) -> Result<ConflictableOutcome, String> {
        revert_commit(&self.inner, sha, no_commit)
    }

    /// Cheap (no graph walk, just a handful of ref/file reads) — safe to call every time a repo
    /// becomes active so `AppState.conflicted_repos` reflects a conflict left over from outside
    /// Trunk (a previous session, or a manual `git merge` in a terminal), not only ones Trunk's
    /// own cherry-pick/revert caused this session.
    pub fn has_conflict(&self) -> bool {
        self.inner.state() != git2::RepositoryState::Clean
    }

    pub fn conflict_status(&self) -> Result<Option<ConflictSession>, String> {
        conflict_status(&self.inner)
    }

    pub fn conflict_file(&self, file_path: &str) -> Result<Vec<ConflictSegment>, String> {
        conflict_file(&self.inner, file_path)
    }

    pub fn finish_conflict_resolution(&self, files: Vec<ResolvedFile>) -> Result<ConflictableOutcome, String> {
        finish_conflict_resolution(&self.inner, files)
    }

    pub fn abort_conflict_resolution(&self) -> Result<(), String> {
        abort_in_progress_operation(&self.inner).map_err(|e| e.to_string())
    }

    pub fn create_branch_at(&self, sha: &str, name: &str) -> Result<(), String> {
        create_branch_at(&self.inner, sha, name)
    }

    pub fn list_branches_for_switch(&self) -> Result<Vec<SwitchBranchEntry>, String> {
        list_branches_for_switch(&self.inner)
    }

    pub fn checkout_branch(
        &self,
        name: &str,
        remote: Option<(&str, &str)>,
        dirty_strategy: Option<DirtyTreeStrategy>,
    ) -> Result<(), String> {
        checkout_branch(&self.inner, name, remote, dirty_strategy)
    }

    pub fn get_branch_delete_info(&self, name: &str) -> Result<BranchDeleteInfo, String> {
        get_branch_delete_info(&self.inner, name)
    }

    pub fn rename_branch(&self, old_name: &str, new_name: &str) -> Result<(), String> {
        rename_branch(&self.inner, old_name, new_name)
    }

    pub fn delete_branch(&self, name: &str, force: bool, also_delete_remote: bool) -> Result<(), String> {
        delete_branch(&self.inner, name, force, also_delete_remote)
    }

    pub fn working_tree_status(&self) -> Result<WorkingTreeStatus, String> {
        working_tree_status(&self.inner)
    }

    pub fn working_file_diff(&self, file_path: &str) -> Result<FileHunkDiff, String> {
        working_file_diff(&self.inner, file_path)
    }

    pub fn stage_file(&self, file_path: &str) -> Result<(), String> {
        stage_file(&self.inner, file_path)
    }

    pub fn unstage_file(&self, file_path: &str) -> Result<(), String> {
        unstage_file(&self.inner, file_path)
    }

    pub fn stage_hunk(&self, file_path: &str, new_start: u32) -> Result<(), String> {
        stage_hunk(&self.inner, file_path, new_start)
    }

    pub fn unstage_hunk(&self, file_path: &str, old_start: u32) -> Result<(), String> {
        unstage_hunk(&self.inner, file_path, old_start)
    }

    pub fn stage_line(&self, file_path: &str, new_start: u32, line_index_in_hunk: u32) -> Result<(), String> {
        stage_line(&self.inner, file_path, new_start, line_index_in_hunk)
    }

    pub fn unstage_line(&self, file_path: &str, old_start: u32, line_index_in_hunk: u32) -> Result<(), String> {
        unstage_line(&self.inner, file_path, old_start, line_index_in_hunk)
    }

    pub fn last_commit_message(&self) -> Result<Option<String>, String> {
        last_commit_message(&self.inner)
    }

    pub fn commit_changes(&self, message: &str, amend: bool, ssh_sign: bool) -> Result<String, String> {
        commit_changes(&self.inner, message, amend, ssh_sign)
    }

    pub fn list_remotes(&self) -> Result<Vec<String>, String> {
        list_remotes(&self.inner)
    }

    pub fn remote_url(&self, name: &str) -> Result<String, String> {
        remote_url(&self.inner, name)
    }

    pub fn list_local_branches_with_tracking(&self) -> Result<Vec<RemoteBranchInfo>, String> {
        list_local_branches_with_tracking(&self.inner)
    }

    pub fn list_commits_ahead(&self, local_branch: &str, remote_name: &str, remote_branch: &str) -> Result<Vec<CommitSummary>, String> {
        list_commits_ahead(&self.inner, local_branch, remote_name, remote_branch)
    }

    pub fn list_commits_behind(&self, local_branch: &str, remote_name: &str, remote_branch: &str) -> Result<Vec<CommitSummary>, String> {
        list_commits_behind(&self.inner, local_branch, remote_name, remote_branch)
    }

    pub fn push_branch(
        &self,
        on_progress: impl FnMut(ProgressEvent),
        local_branch: &str,
        remote_name: &str,
        remote_branch: &str,
        set_upstream: bool,
        force: bool,
        force_with_lease: bool,
    ) -> Result<(), String> {
        push_branch(
            &self.inner,
            on_progress,
            local_branch,
            remote_name,
            remote_branch,
            set_upstream,
            force,
            force_with_lease,
        )
    }

    pub fn fetch_remote(
        &self,
        on_progress: impl FnMut(ProgressEvent),
        remote_name: Option<&str>,
        prune: bool,
        tags: bool,
        submodules: bool,
    ) -> Result<FetchOutcome, String> {
        fetch_remote(&self.inner, on_progress, remote_name, prune, tags, submodules)
    }

    pub fn pull_branch(
        &self,
        on_progress: impl FnMut(ProgressEvent),
        local_branch: &str,
        remote_name: &str,
        remote_branch: &str,
        strategy: PullStrategy,
    ) -> Result<ConflictableOutcome, String> {
        pull_branch(&self.inner, on_progress, local_branch, remote_name, remote_branch, strategy)
    }
}

// --- Wire types (PRD §7.1) -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Local,
    Remote,
    Tag,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefBadge {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    /// An older (parent) lane forking off below this row — merge commit territory.
    MergeIn,
    /// A newer (child) lane converging into this row's lane from above — a branch point.
    JoinIn,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Connector {
    pub lane: u32,
    pub kind: ConnectorKind,
    /// Colour of the *other* lane this connector touches — i.e. the branch being merged in
    /// (MergeIn) or closing off here (JoinIn), never the primary lane's own colour. Exposed
    /// explicitly because a MergeIn target can be a lane allocated on this very row (not yet
    /// in `through_lanes`), so the frontend has no other way to know its colour.
    pub color_index: u8,
}

/// A lane that is alive (has an open, not-yet-reached commit) as this row is drawn — lets the
/// frontend render a continuous coloured line for parallel branches that this commit doesn't
/// itself touch, without needing cross-row state (each row is fully self-describing, which is
/// what makes virtualised/recycled rendering safe).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ThroughLane {
    pub lane: u32,
    pub color_index: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphRow {
    pub sha: String,
    pub short_sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub summary: String,
    pub time: i64,
    pub lane: u32,
    pub lane_color_index: u8,
    pub is_head: bool,
    pub connectors: Vec<Connector>,
    pub through_lanes: Vec<ThroughLane>,
    pub refs: Vec<RefBadge>,
    /// Filter-match flag — always `true` as stored in the cache; `commands::get_graph_rows`
    /// overwrites it per request for the requested window only, never persisted back.
    pub matches: bool,
}

#[derive(Debug, Default)]
pub struct GraphCache {
    pub rows: Vec<GraphRow>,
    pub head_sha: Option<String>,
    /// Highest lane slot ever allocated — lets the frontend size the lane column once for
    /// the whole graph instead of per row (a per-row width caused the column to grow/shrink
    /// between rows, overflowing into the message text on wide histories).
    pub max_lane: u32,
}

impl GraphCache {
    /// All commit SHAs reachable from `branch_name`'s tip, by parent-chain BFS over the
    /// already-walked `rows` — used by the branch filter (§7.3). Bounded by history size,
    /// no extra libgit2 IO since the full parent graph is already in memory.
    pub fn branch_ancestors(&self, branch_name: &str) -> HashSet<String> {
        let mut result = HashSet::new();
        let Some(tip) = self.rows.iter().find(|r| {
            r.refs
                .iter()
                .any(|b| b.kind == RefKind::Local && b.name == branch_name)
        }) else {
            return result;
        };
        let by_sha: HashMap<&str, &GraphRow> =
            self.rows.iter().map(|r| (r.sha.as_str(), r)).collect();
        let mut stack = vec![tip.sha.clone()];
        while let Some(sha) = stack.pop() {
            if !result.insert(sha.clone()) {
                continue;
            }
            if let Some(row) = by_sha.get(sha.as_str()) {
                stack.extend(row.parents.iter().cloned());
            }
        }
        result
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub color_index: u8,
    /// Tip SHA — lets the Create-branch dialog's starting-point dropdown (§13.1, SPEC.md item
    /// 8) resolve a chosen branch straight to `create_branch_at`'s `sha` param without a second
    /// round trip.
    pub sha: String,
}

/// Composable commit-graph filter (PRD §7.3). All fields optional/AND-combined.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct GraphFilter {
    pub author: Option<String>,
    pub branch: Option<String>,
    pub message: Option<String>,
    pub path: Option<String>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub sha_prefix: Option<String>,
}

/// Stable name → 1..=7 hash so a branch's lane colour persists for its lifetime even as the
/// raw lane *slot* gets recycled across history (djb2 — no extra crate needed).
fn branch_color_index(name: &str) -> u8 {
    let mut hash: u32 = 5381;
    for b in name.as_bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(*b as u32);
    }
    (hash % 7) as u8 + 1
}

/// Cheap filters only (author/message/SHA/date) — already in the cached row, no IO. Branch
/// ancestry and path-touch are applied separately by the caller since they need data beyond
/// a single row (the full cache, or a live repo handle for diffing).
pub fn matches_basic(row: &GraphRow, filter: &GraphFilter) -> bool {
    if let Some(author) = &filter.author {
        let needle = author.to_lowercase();
        if !row.author_name.to_lowercase().contains(&needle)
            && !row.author_email.to_lowercase().contains(&needle)
        {
            return false;
        }
    }
    if let Some(message) = &filter.message {
        if !row.summary.to_lowercase().contains(&message.to_lowercase()) {
            return false;
        }
    }
    if let Some(sha_prefix) = &filter.sha_prefix {
        if !row.sha.starts_with(sha_prefix.as_str()) {
            return false;
        }
    }
    if let Some(from) = filter.date_from {
        if row.time < from {
            return false;
        }
    }
    if let Some(to) = filter.date_to {
        if row.time > to {
            return false;
        }
    }
    true
}

/// Path filter (§7.3) — requires a per-commit tree diff, so the caller only invokes this for
/// the requested (viewport-sized) window, never the whole history.
pub fn row_matches_path(repo: &Repository, row: &GraphRow, path: &str) -> bool {
    let Ok(oid) = Oid::from_str(&row.sha) else {
        return false;
    };
    let Ok(commit) = repo.find_commit(oid) else {
        return false;
    };
    let Ok(tree) = commit.tree() else {
        return false;
    };
    if row.parents.is_empty() {
        return diff_touches_path(repo, None, &tree, path);
    }
    row.parents.iter().any(|p| {
        Oid::from_str(p)
            .ok()
            .and_then(|poid| repo.find_commit(poid).ok())
            .and_then(|pc| pc.tree().ok())
            .map(|ptree| diff_touches_path(repo, Some(&ptree), &tree, path))
            .unwrap_or(false)
    })
}

fn diff_touches_path(repo: &Repository, old: Option<&git2::Tree>, new: &git2::Tree, path: &str) -> bool {
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(path);
    repo.diff_tree_to_tree(old, Some(new), Some(&mut opts))
        .map(|d| d.deltas().count() > 0)
        .unwrap_or(false)
}

// --- Branch listing (sidebar Branches section) ----------------------------------------------

fn list_branches(repo: &Repository) -> Result<Vec<BranchInfo>, git2::Error> {
    let mut out = Vec::new();
    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            let sha = branch.get().target().map(|oid| oid.to_string()).unwrap_or_default();
            out.push(BranchInfo {
                name: name.to_string(),
                is_head: branch.is_head(),
                color_index: branch_color_index(name),
                sha,
            });
        }
    }
    out.sort_by(|a, b| b.is_head.cmp(&a.is_head).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

// --- Ref collection (branch/tag/HEAD lookup) -------------------------------------------------

/// Maps every ref tip to the badges its target commit's row should render, plus a separate
/// Oid → branch-name map (local branches only) used to seed lane colour origins during the walk.
fn collect_refs(
    repo: &Repository,
) -> Result<(HashMap<Oid, Vec<RefBadge>>, HashMap<Oid, String>), git2::Error> {
    let mut refs: HashMap<Oid, Vec<RefBadge>> = HashMap::new();
    let mut local_branch_at: HashMap<Oid, String> = HashMap::new();

    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            if let Some(target) = branch.get().target() {
                refs.entry(target).or_default().push(RefBadge {
                    name: name.to_string(),
                    kind: RefKind::Local,
                });
                local_branch_at.entry(target).or_insert_with(|| name.to_string());
            }
        }
    }
    for branch in repo.branches(Some(BranchType::Remote))? {
        let (branch, _) = branch?;
        if let Some(name) = branch.name()? {
            if let Some(target) = branch.get().target() {
                refs.entry(target).or_default().push(RefBadge {
                    name: name.to_string(),
                    kind: RefKind::Remote,
                });
            }
        }
    }
    for tag_name in repo.tag_names(None)?.iter().flatten() {
        if let Ok(reference) = repo.find_reference(&format!("refs/tags/{tag_name}")) {
            if let Some(target) = reference.target() {
                // Annotated tags point at a tag object; peel to the commit it targets.
                // Lightweight tags already point directly at the commit.
                let commit_oid = repo
                    .find_tag(target)
                    .map(|t| t.target_id())
                    .unwrap_or(target);
                refs.entry(commit_oid).or_default().push(RefBadge {
                    name: tag_name.to_string(),
                    kind: RefKind::Tag,
                });
            }
        }
    }
    Ok((refs, local_branch_at))
}

// --- Commit walk + lane assignment (PRD §7.1, §7.2) ------------------------------------------

/// Standard gitk/git-log-graph-style lane assignment: walk newest-first, track which Oid each
/// open lane is waiting for next. A commit either continues an existing lane (it's the next
/// expected node) or opens a new one (an unreached branch tip); merge parents beyond the
/// first either join an already-tracked lane or open a fresh one. Lanes are recycled once
/// freed so lane count stays bounded on wide histories instead of growing forever.
fn build_graph(repo: &Repository) -> Result<GraphCache, git2::Error> {
    let (ref_map, local_branch_at) = collect_refs(repo)?;
    let head_oid = repo.head().ok().and_then(|h| h.target());

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    let _ = revwalk.push_head();
    // Local branches only — NOT remote-tracking branches. A repo can have hundreds of stale
    // `origin/*` refs (forks of active team repos routinely do); seeding the walk with every
    // one of them opens a lane per ref that's never reached again, which is exactly what blew
    // the lane column out to 50+ columns and overflowed into the message text. Remote-tracking
    // commits already reachable through local history still render (and still get their
    // `origin/*` pill via `collect_refs`); only genuinely remote-only, unmerged topic branches
    // are skipped — the same default most Git GUIs use.
    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        if let Some(target) = branch.get().target() {
            let _ = revwalk.push(target);
        }
    }

    let mut active: Vec<Option<Oid>> = Vec::new();
    let mut lane_branch: Vec<Option<String>> = Vec::new();
    let mut lane_color: Vec<u8> = Vec::new();
    let mut rows: Vec<GraphRow> = Vec::new();

    for oid_result in revwalk {
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;

        // Snapshot before any mutation below — every lane still open at this point is
        // "passing through" this row's vertical band regardless of whether this commit
        // touches it.
        let through_lanes: Vec<ThroughLane> = active
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                a.map(|_| ThroughLane {
                    lane: i as u32,
                    color_index: lane_color[i],
                })
            })
            .collect();

        let incoming: Vec<usize> = active
            .iter()
            .enumerate()
            .filter_map(|(i, a)| (*a == Some(oid)).then_some(i))
            .collect();

        let primary_lane = if let Some(&first) = incoming.first() {
            first
        } else {
            match active.iter().position(|a| a.is_none()) {
                Some(idx) => idx,
                None => {
                    active.push(None);
                    lane_branch.push(None);
                    lane_color.push(0);
                    active.len() - 1
                }
            }
        };

        let origin_branch = lane_branch
            .get(primary_lane)
            .cloned()
            .flatten()
            .or_else(|| local_branch_at.get(&oid).cloned());

        let is_head = head_oid == Some(oid);
        let lane_color_index = if is_head {
            1
        } else if let Some(name) = &origin_branch {
            branch_color_index(name)
        } else {
            (primary_lane as u32 % 7) as u8 + 1
        };

        let mut connectors = Vec::new();
        for &lane_idx in incoming.iter().skip(1) {
            connectors.push(Connector {
                lane: lane_idx as u32,
                kind: ConnectorKind::JoinIn,
                color_index: lane_color[lane_idx],
            });
            active[lane_idx] = None;
            lane_branch[lane_idx] = None;
        }

        let parents: Vec<Oid> = commit.parent_ids().collect();
        if parents.is_empty() {
            active[primary_lane] = None;
            lane_branch[primary_lane] = None;
        } else {
            active[primary_lane] = Some(parents[0]);
            lane_branch[primary_lane] = origin_branch.clone();
            lane_color[primary_lane] = lane_color_index;
            for &parent_oid in parents.iter().skip(1) {
                if let Some(existing_lane) = active.iter().position(|a| *a == Some(parent_oid)) {
                    connectors.push(Connector {
                        lane: existing_lane as u32,
                        kind: ConnectorKind::MergeIn,
                        color_index: lane_color[existing_lane],
                    });
                } else {
                    let parent_origin = local_branch_at.get(&parent_oid).cloned();
                    let new_lane = match active.iter().position(|a| a.is_none()) {
                        Some(idx) => idx,
                        None => {
                            active.push(None);
                            lane_branch.push(None);
                            lane_color.push(0);
                            active.len() - 1
                        }
                    };
                    let new_color = parent_origin
                        .as_deref()
                        .map(branch_color_index)
                        .unwrap_or((new_lane as u32 % 7) as u8 + 1);
                    active[new_lane] = Some(parent_oid);
                    lane_branch[new_lane] = parent_origin;
                    lane_color[new_lane] = new_color;
                    connectors.push(Connector {
                        lane: new_lane as u32,
                        kind: ConnectorKind::MergeIn,
                        color_index: new_color,
                    });
                }
            }
        }

        let author = commit.author();
        let sha = oid.to_string();
        let short_sha = sha[..7.min(sha.len())].to_string();
        rows.push(GraphRow {
            sha,
            short_sha,
            parents: parents.iter().map(Oid::to_string).collect(),
            author_name: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            summary: commit.summary().unwrap_or_default().to_string(),
            time: commit.time().seconds(),
            lane: primary_lane as u32,
            lane_color_index,
            is_head,
            connectors,
            through_lanes,
            refs: ref_map.get(&oid).cloned().unwrap_or_default(),
            matches: true,
        });
    }

    let max_lane = active.len().saturating_sub(1) as u32;
    Ok(GraphCache {
        rows,
        head_sha: head_oid.map(|o| o.to_string()),
        max_lane,
    })
}

// --- Commit detail (PRD §4.3, SPEC.md item 4) ------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CommitFileChange {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitDetail {
    pub sha: String,
    pub short_sha: String,
    pub author_name: String,
    pub author_email: String,
    pub summary: String,
    pub time: i64,
    pub parents: Vec<String>,
    pub files: Vec<CommitFileChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffLineRow {
    pub kind: DiffLineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

/// Mainline-only (parent 0) base tree for diffing a commit against its predecessor — same
/// merge-commit convention `row_matches_path` above already uses, applied here for the
/// overlay's file list/diff display. `None` for root commits (diff against an empty tree).
/// This is purely a *display* convention and is independent of `cherry_pick`/`revert_commit`
/// below, which refuse merge commits outright rather than guessing a mainline for an action.
fn diff_base_tree<'a>(commit: &git2::Commit<'a>) -> Result<Option<git2::Tree<'a>>, git2::Error> {
    match commit.parent(0) {
        Ok(parent) => Ok(Some(parent.tree()?)),
        Err(_) => Ok(None),
    }
}

/// Maps a `DiffLine::origin()` char to a kind, or `None` to skip the line entirely — covers
/// hunk/file headers and "no newline at end of file" annotation lines (`=`/`>`/`<`), neither of
/// which represents real file content worth a row in the overlay's diff view.
fn line_kind(origin: char) -> Option<DiffLineKind> {
    match origin {
        '+' => Some(DiffLineKind::Addition),
        '-' => Some(DiffLineKind::Deletion),
        ' ' => Some(DiffLineKind::Context),
        _ => None,
    }
}

fn delta_path(delta: &git2::DiffDelta) -> Option<String> {
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Per-file +/- counts (PRD §4.3's changed-files list) via a line-callback tally, then assembled
/// in `diff.deltas()`'s natural order so the file list matches git's own ordering.
fn diff_file_stats(diff: &git2::Diff) -> Result<Vec<CommitFileChange>, git2::Error> {
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |delta, _hunk, line| {
            let Some(path) = delta_path(&delta) else { return true };
            let entry = counts.entry(path).or_insert((0, 0));
            match line_kind(line.origin()) {
                Some(DiffLineKind::Addition) => entry.0 += 1,
                Some(DiffLineKind::Deletion) => entry.1 += 1,
                _ => {}
            }
            true
        }),
    )?;

    let mut files = Vec::new();
    for delta in diff.deltas() {
        let Some(path) = delta_path(&delta) else { continue };
        let (additions, deletions) = counts.get(&path).copied().unwrap_or((0, 0));
        files.push(CommitFileChange { path, additions, deletions });
    }
    Ok(files)
}

fn commit_detail(repo: &Repository, sha: &str) -> Result<CommitDetail, git2::Error> {
    let oid = Oid::from_str(sha)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let base_tree = diff_base_tree(&commit)?;
    let diff = repo.diff_tree_to_tree(base_tree.as_ref(), Some(&tree), None)?;
    let files = diff_file_stats(&diff)?;

    let author = commit.author();
    let sha_string = oid.to_string();
    let short_sha = sha_string[..7.min(sha_string.len())].to_string();
    Ok(CommitDetail {
        sha: sha_string,
        short_sha,
        author_name: author.name().unwrap_or_default().to_string(),
        author_email: author.email().unwrap_or_default().to_string(),
        summary: commit.summary().unwrap_or_default().to_string(),
        time: commit.time().seconds(),
        parents: commit.parent_ids().map(|p| p.to_string()).collect(),
        files,
    })
}

/// Unified diff for one file within a commit, scoped via `DiffOptions::pathspec` (same pattern
/// `diff_touches_path` above already uses) — `diff.foreach`'s line callback already carries the
/// delta per line, so no separate `file_cb` bookkeeping is needed for a single-file diff.
fn commit_file_diff(repo: &Repository, sha: &str, file_path: &str) -> Result<Vec<DiffLineRow>, git2::Error> {
    let oid = Oid::from_str(sha)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let base_tree = diff_base_tree(&commit)?;
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(file_path);
    let diff = repo.diff_tree_to_tree(base_tree.as_ref(), Some(&tree), Some(&mut opts))?;

    let mut lines = Vec::new();
    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            if let Some(kind) = line_kind(line.origin()) {
                let content = String::from_utf8_lossy(line.content())
                    .trim_end_matches('\n')
                    .to_string();
                lines.push(DiffLineRow {
                    kind,
                    old_lineno: line.old_lineno(),
                    new_lineno: line.new_lineno(),
                    content,
                });
            }
            true
        }),
    )?;
    Ok(lines)
}

/// Working tree must be clean before `cherry_pick`/`revert_commit` start — both write results
/// into the index *and* the working directory, so recovering from a conflict means resetting
/// both back to HEAD (see `abort_in_progress_operation`). That reset is only safe if HEAD's tree
/// really was the pre-operation state; this precheck guarantees that rather than risking
/// collateral damage to unrelated uncommitted work.
fn is_working_tree_clean(repo: &Repository) -> Result<bool, git2::Error> {
    Ok(repo.statuses(None)?.is_empty())
}

/// Resets index+workdir to HEAD and clears the in-progress cherry-pick/revert state — the same
/// outcome as `git cherry-pick --abort`/`git revert --abort` (which are themselves implemented
/// as exactly this: libgit2 exposes no separate "abort" primitive for a single `cherrypick()`/
/// `revert()` call). Only called once `is_working_tree_clean` has already confirmed HEAD is the
/// true pre-operation state, so nothing unrelated is at risk.
fn abort_in_progress_operation(repo: &Repository) -> Result<(), git2::Error> {
    let head_commit = repo.head()?.peel_to_commit()?;
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.reset(head_commit.as_object(), git2::ResetType::Hard, Some(&mut checkout_opts))?;
    repo.cleanup_state()
}

/// Outcome of a cherry-pick/revert attempt — `Conflict` means libgit2 has already written
/// diff3-marker-annotated files into the working tree and left index conflict entries in place
/// (SPEC.md item 6, PRD §9): unlike the old behaviour, this is *not* an error to recover from,
/// it's a handoff to the conflict resolver. Caller is responsible for setting
/// `AppState.conflict_resolution_in_progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConflictableOutcome {
    Completed { sha: String },
    /// `no_commit: true` and the apply was clean — matches plain `git cherry-pick -n`/
    /// `git revert -n`: the index/working tree carry the applied change, but deliberately no
    /// commit is created. Unlike the conflict path below, `CHERRY_PICK_HEAD`/`REVERT_HEAD` *is*
    /// cleaned up here (`cleanup_state()`) — a single non-conflicting `-n` pick leaves no
    /// lingering sequencer state in real git either, and leaving it would make `has_conflict()`
    /// (`state() != Clean`) wrongly treat these intentionally-uncommitted changes as a conflict.
    AppliedNoCommit,
    Conflict,
}

/// Cherry-picks a single commit onto HEAD. Refuses merge commits outright (matching plain `git
/// cherry-pick`'s own default refusal without `-m` — libgit2 hard-errors rather than defaulting
/// to a mainline if one isn't specified) and refuses a dirty working tree (see
/// `is_working_tree_clean`) before calling into libgit2 at all. `no_commit` mirrors `-n`/
/// `--no-commit`: apply without creating a commit (see `ConflictableOutcome::AppliedNoCommit`).
///
/// On conflicts, leaves the index/working tree exactly as `repo.cherrypick()` produced them
/// (same as plain `git cherry-pick` would) instead of aborting — `conflict_style_diff3` on the
/// checkout builder (not the merge options; libgit2's checkout step recomputes each conflicting
/// file's on-disk content from scratch and only reads this style flag, ignoring whatever the
/// merge step itself used) makes libgit2 write `<<<<<<<`/`|||||||`/`=======`/`>>>>>>>` markers
/// straight into the conflicting files, which the conflict resolver reads back via
/// `get_conflict_file`.
fn cherry_pick(repo: &Repository, sha: &str, no_commit: bool) -> Result<ConflictableOutcome, String> {
    let oid = Oid::from_str(sha).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    if commit.parent_count() > 1 {
        return Err("Cherry-picking a merge commit isn't supported yet.".to_string());
    }
    if !is_working_tree_clean(repo).map_err(|e| e.to_string())? {
        return Err("Commit or stash your changes before cherry-picking.".to_string());
    }
    let head_commit = repo
        .head()
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?;

    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.conflict_style_diff3(true);
    let mut cherrypick_opts = git2::CherrypickOptions::new();
    cherrypick_opts.checkout_builder(checkout_builder);
    repo.cherrypick(&commit, Some(&mut cherrypick_opts)).map_err(|e| e.to_string())?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        return Ok(ConflictableOutcome::Conflict);
    }
    if no_commit {
        // libgit2's `cherrypick()` always writes `CHERRY_PICK_HEAD` and sets the repo state to
        // "in progress", regardless of `no_commit` — real `git cherry-pick -n` doesn't leave that
        // lingering for a single non-conflicting commit, and neither should this: without the
        // cleanup, `has_conflict()` (state != Clean) would report a conflict that doesn't exist,
        // surfacing a phantom "Resolve conflicts" button whose Abort would then discard these
        // intentionally-uncommitted changes. `cleanup_state()` only removes the sequencer
        // bookkeeping files — it doesn't touch the index/working tree, so the applied changes
        // this call exists to produce are left exactly as they are.
        repo.cleanup_state().map_err(|e| e.to_string())?;
        return Ok(ConflictableOutcome::AppliedNoCommit);
    }

    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
    let author = commit.author();
    let committer = repo.signature().map_err(|e| e.to_string())?;
    let message = commit.message().unwrap_or_default();
    let new_oid = repo
        .commit(Some("HEAD"), &author, &committer, message, &tree, &[&head_commit])
        .map_err(|e| e.to_string())?;
    repo.cleanup_state().map_err(|e| e.to_string())?;
    Ok(ConflictableOutcome::Completed { sha: new_oid.to_string() })
}

/// Reverts a single commit on top of HEAD with a new commit (author = committer = current user,
/// message matches plain `git revert`'s default format). Same merge-commit and dirty-tree
/// upfront refusals, the same conflict-handoff shape, and the same `no_commit` (`-n`) meaning,
/// as `cherry_pick` above.
fn revert_commit(repo: &Repository, sha: &str, no_commit: bool) -> Result<ConflictableOutcome, String> {
    let oid = Oid::from_str(sha).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    if commit.parent_count() > 1 {
        return Err("Reverting a merge commit isn't supported yet.".to_string());
    }
    if !is_working_tree_clean(repo).map_err(|e| e.to_string())? {
        return Err("Commit or stash your changes before reverting.".to_string());
    }
    let head_commit = repo
        .head()
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?;

    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.conflict_style_diff3(true);
    let mut revert_opts = git2::RevertOptions::new();
    revert_opts.checkout_builder(checkout_builder);
    repo.revert(&commit, Some(&mut revert_opts)).map_err(|e| e.to_string())?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        return Ok(ConflictableOutcome::Conflict);
    }
    if no_commit {
        // Same lingering-state issue `cherry_pick` has — see its matching comment above.
        repo.cleanup_state().map_err(|e| e.to_string())?;
        return Ok(ConflictableOutcome::AppliedNoCommit);
    }

    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
    let sig = repo.signature().map_err(|e| e.to_string())?;
    let message = format!(
        "Revert \"{}\"\n\nThis reverts commit {}.\n",
        commit.summary().unwrap_or_default(),
        oid
    );
    let new_oid = repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&head_commit])
        .map_err(|e| e.to_string())?;
    repo.cleanup_state().map_err(|e| e.to_string())?;
    Ok(ConflictableOutcome::Completed { sha: new_oid.to_string() })
}

// --- Conflict resolver (SPEC.md item 6, PRD §4.6/§9) --------------------------------------

/// One conflicting file's three-way content, already split on the diff3 markers `cherry_pick`/
/// `revert_commit` asked libgit2 to write. `Context` segments need no resolution; `Conflict`
/// segments are exactly what the three-panel editor + per-hunk accept controls operate on.
/// Resolution choices themselves are frontend-only state (composing one of `ours`/`base`/
/// `theirs`/both, or a manual edit) — nothing here is mutated until `finish_conflict_resolution`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictSegment {
    Context { lines: Vec<String> },
    Conflict { ours: Vec<String>, base: Vec<String>, theirs: Vec<String> },
}

/// Snapshot of the operation currently in progress (`repo.state()`-derived, so it survives an
/// app restart and matches whatever plain `git status` would also report) plus the set of
/// still-conflicting file paths. `None` means nothing is in progress.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictSession {
    pub operation: String,
    pub summary: String,
    pub files: Vec<String>,
}

/// A single resolved file as sent back by `finish_conflict_resolution` — `content` is the final
/// marker-free text the frontend composed from its hunk choices (or manual edit).
#[derive(Debug, Clone, Deserialize)]
pub struct ResolvedFile {
    pub path: String,
    pub content: String,
}

fn conflict_operation_name(state: git2::RepositoryState) -> &'static str {
    use git2::RepositoryState::*;
    match state {
        Merge => "merge",
        Revert | RevertSequence => "revert",
        CherryPick | CherryPickSequence => "cherry-pick",
        Rebase | RebaseInteractive | RebaseMerge => "rebase",
        _ => "operation",
    }
}

fn conflict_status(repo: &Repository) -> Result<Option<ConflictSession>, String> {
    let state = repo.state();
    if state == git2::RepositoryState::Clean {
        return Ok(None);
    }
    let operation = conflict_operation_name(state).to_string();
    let summary = repo.message().unwrap_or_default();

    let index = repo.index().map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for conflict in index.conflicts().map_err(|e| e.to_string())? {
        let conflict = conflict.map_err(|e| e.to_string())?;
        let entry = conflict.our.as_ref().or(conflict.their.as_ref()).or(conflict.ancestor.as_ref());
        if let Some(entry) = entry {
            files.push(String::from_utf8_lossy(&entry.path).to_string());
        }
    }
    Ok(Some(ConflictSession { operation, summary, files }))
}

/// Splits a diff3-marker-annotated file's text into context/conflict segments. Marker lines are
/// matched on the standard 7-character prefixes (`<<<<<<<`/`|||||||`/`=======`/`>>>>>>>`), with
/// or without a trailing label (libgit2 always adds one, but the bare prefix is matched too in
/// case a manual edit strips it) — see PRD §9.2/§9.4.
fn parse_conflict_segments(content: &str) -> Vec<ConflictSegment> {
    let mut segments = Vec::new();
    let mut context: Vec<String> = Vec::new();
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            if !context.is_empty() {
                segments.push(ConflictSegment::Context { lines: std::mem::take(&mut context) });
            }
            let mut ours = Vec::new();
            let mut base = Vec::new();
            let mut theirs = Vec::new();
            let mut in_base = false;
            let mut in_theirs = false;
            for l in lines.by_ref() {
                if l.starts_with("|||||||") {
                    in_base = true;
                    continue;
                }
                if l == "=======" {
                    in_base = false;
                    in_theirs = true;
                    continue;
                }
                if l.starts_with(">>>>>>>") {
                    break;
                }
                if in_theirs {
                    theirs.push(l.to_string());
                } else if in_base {
                    base.push(l.to_string());
                } else {
                    ours.push(l.to_string());
                }
            }
            segments.push(ConflictSegment::Conflict { ours, base, theirs });
        } else {
            context.push(line.to_string());
        }
    }
    if !context.is_empty() {
        segments.push(ConflictSegment::Context { lines: context });
    }
    segments
}

fn conflict_file(repo: &Repository, file_path: &str) -> Result<Vec<ConflictSegment>, String> {
    let workdir = repo.workdir().ok_or_else(|| "Bare repositories have no working tree.".to_string())?;
    let content = std::fs::read_to_string(workdir.join(file_path)).map_err(|e| e.to_string())?;
    Ok(parse_conflict_segments(&content))
}

/// Writes every resolved file's final (marker-free) content to the working tree, stages it
/// (`index.add_path` is what actually clears that path's index conflict entry — the same effect
/// `git add` has after resolving by hand), and creates the final commit. Which parents/author/
/// message to use is derived from `repo.state()` plus the operation's own ref
/// (`CHERRY_PICK_HEAD`/`MERGE_HEAD`) or prepared message file, the same sources plain `git
/// cherry-pick --continue`/`git merge --continue` read from — so this works whether or not the
/// app was restarted mid-resolution.
fn finish_conflict_resolution(repo: &Repository, files: Vec<ResolvedFile>) -> Result<ConflictableOutcome, String> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| "Bare repositories have no working tree.".to_string())?
        .to_path_buf();
    let mut index = repo.index().map_err(|e| e.to_string())?;
    for file in &files {
        std::fs::write(workdir.join(&file.path), &file.content).map_err(|e| e.to_string())?;
        index.add_path(std::path::Path::new(&file.path)).map_err(|e| e.to_string())?;
    }
    index.write().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        return Err("Some files still have unresolved conflicts.".to_string());
    }

    // Rebase has no single fixed parent set (each step is its own commit, and a later step may
    // still conflict) — handed off to `drive_rebase_to_completion` instead of the fixed-parents
    // commit logic below, which only fits cherry-pick/revert/merge's "exactly one commit" shape.
    if matches!(
        repo.state(),
        git2::RepositoryState::Rebase | git2::RepositoryState::RebaseInteractive | git2::RepositoryState::RebaseMerge
    ) {
        let mut rebase = repo.open_rebase(None).map_err(|e| e.to_string())?;
        let committer = repo.signature().map_err(|e| e.to_string())?;
        rebase.commit(None, &committer, None).map_err(|e| e.to_string())?;
        return drive_rebase_to_completion(repo, rebase);
    }

    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
    let head_commit = repo
        .head()
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?;
    let committer = repo.signature().map_err(|e| e.to_string())?;
    let state = repo.state();

    let (author, raw_message, parents): (git2::Signature<'static>, String, Vec<Oid>) = match state {
        git2::RepositoryState::CherryPick | git2::RepositoryState::CherryPickSequence => {
            let cherry_commit = repo
                .find_reference("CHERRY_PICK_HEAD")
                .map_err(|e| e.to_string())?
                .peel_to_commit()
                .map_err(|e| e.to_string())?;
            let author = cherry_commit.author().to_owned();
            let message = repo
                .message()
                .unwrap_or_else(|_| cherry_commit.message().unwrap_or_default().to_string());
            (author, message, vec![head_commit.id()])
        }
        git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence => {
            let message = repo.message().map_err(|e| e.to_string())?;
            (committer.to_owned(), message, vec![head_commit.id()])
        }
        git2::RepositoryState::Merge => {
            let merge_oid = repo
                .find_reference("MERGE_HEAD")
                .map_err(|e| e.to_string())?
                .peel_to_commit()
                .map_err(|e| e.to_string())?
                .id();
            let message = repo.message().unwrap_or_default();
            (committer.to_owned(), message, vec![head_commit.id(), merge_oid])
        }
        _ => return Err("No cherry-pick, revert, or merge is currently in progress.".to_string()),
    };
    // `repo.message()` is `.git/MERGE_MSG`, which libgit2 appends a "#Conflicts:" comment
    // block to once a conflict occurs — strip it the same way `git commit`'s own message editor
    // would (comment lines starting with `#`) rather than baking that block into the final commit.
    let message = git2::message_prettify(raw_message, git2::DEFAULT_COMMENT_CHAR).map_err(|e| e.to_string())?;

    let parent_commits: Vec<git2::Commit> = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();

    let new_oid = repo
        .commit(Some("HEAD"), &author, &committer, &message, &tree, &parent_refs)
        .map_err(|e| e.to_string())?;
    repo.cleanup_state().map_err(|e| e.to_string())?;
    Ok(ConflictableOutcome::Completed { sha: new_oid.to_string() })
}

/// Creates a branch at `sha`. `repo.branch(..., force: false)` surfaces git2's natural
/// duplicate-name error rather than silently overwriting.
///
/// Deliberately ref-creation only — never checks out. SPEC.md item 8's "Checkout after
/// creating" checkbox is handled by the frontend as a *separate* call to `checkout_branch` once
/// this one succeeds (create-branch-dialog.js), rather than this function doing both: the
/// starting point can be any commit (graph context menu's "branch from here" passes whichever
/// row was clicked), so checking it out afterward is exactly `checkout_branch`'s own "move across
/// two potentially very different trees" case (dirty-tree stash/carry handling included) — no
/// reason to duplicate that logic here. It also gives the two steps independent failure
/// boundaries: a failed checkout shouldn't read back as "branch creation failed" when the branch
/// genuinely exists.
fn create_branch_at(repo: &Repository, sha: &str, name: &str) -> Result<(), String> {
    let oid = Oid::from_str(sha).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    repo.branch(name, &commit, false).map_err(|e| e.to_string())?;
    Ok(())
}

// --- Branch CRUD: switch/rename/delete (SPEC.md item 8, PRD §13) ---------------------------

/// One row in the Switch-branch dialog's combined local + remote-only list (§13.2).
#[derive(Debug, Clone, Serialize)]
pub struct SwitchBranchEntry {
    pub name: String,
    pub is_head: bool,
    pub color_index: u8,
    pub is_remote_only: bool,
    /// `<remote>/<branch>` for a remote-only row; `None` for a local branch (its tracking info,
    /// if any, is already covered by `list_branches_with_tracking` elsewhere).
    pub remote_label: Option<String>,
    pub last_commit_time: i64,
}

fn list_branches_for_switch(repo: &Repository) -> Result<Vec<SwitchBranchEntry>, String> {
    let mut local_names: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for branch in repo.branches(Some(BranchType::Local)).map_err(|e| e.to_string())? {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        let Some(name) = branch.name().map_err(|e| e.to_string())?.map(str::to_string) else {
            continue;
        };
        local_names.insert(name.clone());
        let last_commit_time = branch
            .get()
            .target()
            .and_then(|oid| repo.find_commit(oid).ok())
            .map(|c| c.time().seconds())
            .unwrap_or(0);
        out.push(SwitchBranchEntry {
            name: name.clone(),
            is_head: branch.is_head(),
            color_index: branch_color_index(&name),
            is_remote_only: false,
            remote_label: None,
            last_commit_time,
        });
    }
    out.sort_by(|a, b| b.last_commit_time.cmp(&a.last_commit_time));

    let mut remote_only = Vec::new();
    for branch in repo.branches(Some(BranchType::Remote)).map_err(|e| e.to_string())? {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        let Some(full_name) = branch.name().map_err(|e| e.to_string())?.map(str::to_string) else {
            continue;
        };
        // `<remote>/HEAD` is a symbolic pointer, not a checkout target.
        let Some((remote, short_name)) = full_name.split_once('/') else { continue };
        if short_name == "HEAD" || local_names.contains(short_name) {
            continue;
        }
        let last_commit_time = branch
            .get()
            .target()
            .and_then(|oid| repo.find_commit(oid).ok())
            .map(|c| c.time().seconds())
            .unwrap_or(0);
        remote_only.push(SwitchBranchEntry {
            name: short_name.to_string(),
            is_head: false,
            color_index: branch_color_index(short_name),
            is_remote_only: true,
            remote_label: Some(format!("{remote}/{short_name}")),
            last_commit_time,
        });
    }
    remote_only.sort_by(|a, b| b.last_commit_time.cmp(&a.last_commit_time));
    out.extend(remote_only);
    Ok(out)
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirtyTreeStrategy {
    /// `git stash` before checkout, reapply after (§13.2's default radio option).
    Stash,
    /// Check out without stashing — libgit2's own safe (non-forced) checkout naturally refuses
    /// if it would clobber a dirty path, which is exactly the "may fail on conflict" behaviour
    /// §13.2 describes for this option.
    Carry,
}

/// Moves HEAD to `refname` and applies it to the index/working tree — shared by `checkout_branch`
/// (Switch branch, §13.2) and `create_branch_at`'s "checkout after creating" step (Create branch,
/// §13.1), since both are exactly the same "move across two potentially very different trees"
/// operation. `dirty_strategy` is only consulted when the working tree is actually dirty; a
/// clean tree ignores it entirely and always proceeds.
///
/// Must check dirtiness *before* `set_head` — moving HEAD alone (before any checkout) already
/// changes what the index is compared against, so checking after would compare the new tree
/// against the still-unchanged old index and spuriously report "dirty" even when nothing is
/// actually uncommitted.
/// Paths `repo.statuses(None)` reports as touched — staged or unstaged, matching
/// `is_working_tree_clean`'s own definition of "dirty" (untracked files excluded, same as that
/// function). Used both to decide *whether* the tree is dirty and, for the "carry over" strategy
/// below, exactly *which* paths are real local edits that must not be clobbered.
fn dirty_status_paths(repo: &Repository) -> Result<HashSet<String>, String> {
    Ok(repo
        .statuses(None)
        .map_err(|e| e.to_string())?
        .iter()
        .filter_map(|e| e.path().map(String::from))
        .collect())
}

/// Paths whose blob differs between `old_tree` and `new_tree` — i.e. exactly the paths a checkout
/// from one to the other needs to touch.
fn tree_diff_paths(repo: &Repository, old_tree: &git2::Tree, new_tree: &git2::Tree) -> Result<HashSet<String>, String> {
    let diff = repo
        .diff_tree_to_tree(Some(old_tree), Some(new_tree), None)
        .map_err(|e| e.to_string())?;
    let mut paths = HashSet::new();
    for delta in diff.deltas() {
        if let Some(p) = delta.old_file().path() {
            paths.insert(p.to_string_lossy().into_owned());
        }
        if let Some(p) = delta.new_file().path() {
            paths.insert(p.to_string_lossy().into_owned());
        }
    }
    Ok(paths)
}

fn checkout_ref_with_dirty_handling(
    repo: &Repository,
    refname: &str,
    dirty_strategy: Option<DirtyTreeStrategy>,
) -> Result<(), String> {
    let dirty_paths = dirty_status_paths(repo)?;
    let dirty = !dirty_paths.is_empty();
    let stash_sig = if dirty && matches!(dirty_strategy, Some(DirtyTreeStrategy::Stash)) {
        Some(repo.signature().map_err(|e| e.to_string())?)
    } else {
        None
    };

    if dirty && dirty_strategy.is_none() {
        return Err("Working tree has uncommitted changes — choose how to handle them before switching.".to_string());
    }

    // "Carry over" keeps the dirty paths in place rather than stashing them — so unlike the
    // clean/stashed cases below, this checkout must not touch every path the target tree
    // differs on, only the ones that don't collide with the user's actual edits. Originally this
    // relied on libgit2's non-forced `GIT_CHECKOUT_SAFE` strategy to make that per-path call —
    // but its stat-based "would this overwrite something" heuristic turned out to be unreliable
    // in two different ways (see `create_branch_at`'s and `checkout_branch`'s history): it can
    // both leave a file's on-disk content stale when nothing was at risk, *and* — the case this
    // fixes — silently leave OTHER, never-touched-by-the-user files half-applied (index updated,
    // workdir not) instead of cleanly checking them out. So the overlap is now computed
    // explicitly: diff old HEAD's tree against the target tree for the exact path set checkout
    // needs to touch, refuse loudly (matching §13.2's "may fail on conflict") only if that set
    // overlaps the user's real dirty paths, and otherwise force exactly those paths — guaranteed
    // correct, and guaranteed to never touch a path the user didn't ask to change.
    let carrying_real_changes = dirty && matches!(dirty_strategy, Some(DirtyTreeStrategy::Carry));
    let touched_paths = if carrying_real_changes {
        let old_tree = repo.head().map_err(|e| e.to_string())?.peel_to_tree().map_err(|e| e.to_string())?;
        let target_oid = repo.refname_to_id(refname).map_err(|e| e.to_string())?;
        let new_tree = repo.find_commit(target_oid).map_err(|e| e.to_string())?.tree().map_err(|e| e.to_string())?;
        let touched = tree_diff_paths(repo, &old_tree, &new_tree)?;
        let conflicting: Vec<&String> = touched.iter().filter(|p| dirty_paths.contains(*p)).collect();
        if !conflicting.is_empty() {
            let mut names: Vec<String> = conflicting.into_iter().cloned().collect();
            names.sort();
            return Err(format!("Switching would overwrite uncommitted changes in: {}", names.join(", ")));
        }
        Some(touched)
    } else {
        None
    };

    if let Some(sig) = &stash_sig {
        // `stash_save2` needs `&mut Repository`, but this function only takes `&Repository`
        // (matching every other free function in this file, which all go through `Repo`'s
        // shared `&self.inner`) — open a second, independent handle onto the same on-disk repo
        // rather than threading `&mut` through the whole call chain for this one caller.
        let mut repo_mut = Repository::open(repo.path()).map_err(|e| e.to_string())?;
        repo_mut
            .stash_save2(sig, None, Some(git2::StashFlags::INCLUDE_UNTRACKED))
            .map_err(|e| e.to_string())?;
    }

    let checkout_result = (|| {
        repo.set_head(refname).map_err(|e| e.to_string())?;
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        // Restrict the forced checkout to exactly the paths the carry-over case verified above
        // are safe (no overlap with real dirty paths) — every other case (clean tree, or one
        // just emptied by `stash_save2`) has nothing left to protect, so it forces the whole tree.
        if let Some(touched) = &touched_paths {
            for path in touched {
                builder.path(path);
            }
        }
        repo.checkout_head(Some(&mut builder)).map_err(|e| e.to_string())
    })();

    if stash_sig.is_some() {
        let mut repo_mut = Repository::open(repo.path()).map_err(|e| e.to_string())?;
        // Best-effort: if the checkout itself failed, there's nothing to reapply onto; if it
        // succeeded but the pop conflicts, surface that as the operation's own error rather than
        // swallowing it — the stash stays on the stack either way so nothing is lost.
        if checkout_result.is_ok() {
            repo_mut.stash_pop(0, None).map_err(|e| e.to_string())?;
        }
    }

    checkout_result
}

/// Switches HEAD to `name` (a local branch, or — when `remote` is given — creates a local
/// tracking branch for a remote-only one first, matching the "Checkout & track" button label).
/// `dirty_strategy` is only consulted when the working tree is actually dirty; a clean tree
/// ignores it entirely.
fn checkout_branch(
    repo: &Repository,
    name: &str,
    remote: Option<(&str, &str)>,
    dirty_strategy: Option<DirtyTreeStrategy>,
) -> Result<(), String> {
    if let Some((remote_name, remote_branch)) = remote {
        let remote_ref = format!("refs/remotes/{remote_name}/{remote_branch}");
        let target = repo.refname_to_id(&remote_ref).map_err(|e| e.to_string())?;
        let commit = repo.find_commit(target).map_err(|e| e.to_string())?;
        let mut branch = repo.branch(name, &commit, false).map_err(|e| e.to_string())?;
        branch
            .set_upstream(Some(&format!("{remote_name}/{remote_branch}")))
            .map_err(|e| e.to_string())?;
    }

    let branch_ref = repo.find_branch(name, BranchType::Local).map_err(|e| e.to_string())?;
    let refname = branch_ref
        .get()
        .name()
        .ok_or_else(|| "Branch has an invalid reference name.".to_string())?
        .to_string();

    checkout_ref_with_dirty_handling(repo, &refname, dirty_strategy)
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchDeleteInfo {
    pub merged: bool,
    pub commit_loss_count: usize,
}

/// §13.4's safe/destructive delete-dialog split: a branch is "safe" when its tip is an ancestor
/// of HEAD (fully merged in); otherwise reports how many of its commits HEAD doesn't have.
fn get_branch_delete_info(repo: &Repository, name: &str) -> Result<BranchDeleteInfo, String> {
    let branch_oid = repo
        .find_branch(name, BranchType::Local)
        .map_err(|e| e.to_string())?
        .get()
        .target()
        .ok_or_else(|| "Branch has no commits yet.".to_string())?;
    let head_oid = repo
        .head()
        .map_err(|e| e.to_string())?
        .target()
        .ok_or_else(|| "HEAD has no commits yet.".to_string())?;
    let merged = branch_oid == head_oid || repo.graph_descendant_of(head_oid, branch_oid).map_err(|e| e.to_string())?;
    let commit_loss_count = if merged {
        0
    } else {
        let mut walk = repo.revwalk().map_err(|e| e.to_string())?;
        walk.push(branch_oid).map_err(|e| e.to_string())?;
        walk.hide(head_oid).map_err(|e| e.to_string())?;
        walk.count()
    };
    Ok(BranchDeleteInfo { merged, commit_loss_count })
}

/// No remote-tracking-ref rename — §13.3's dialog explains this as a static warning rather than
/// the backend trying to also rename the upstream ref (which git itself doesn't do either).
fn rename_branch(repo: &Repository, old_name: &str, new_name: &str) -> Result<(), String> {
    let mut branch = repo.find_branch(old_name, BranchType::Local).map_err(|e| e.to_string())?;
    branch.rename(new_name, false).map_err(|e| e.to_string())?;
    Ok(())
}

/// `force` only changes whether git2's own merge-safety check is bypassed — `delete_branch_info`
/// is what actually drives the dialog's safe/destructive split; this just performs the deletion
/// once the dialog has gated it via the mandatory acknowledgement checkbox. git2 itself refuses
/// to delete the currently checked-out branch, which is the right behaviour here too (no special
/// casing needed).
fn delete_branch(repo: &Repository, name: &str, force: bool, also_delete_remote: bool) -> Result<(), String> {
    let mut branch = repo.find_branch(name, BranchType::Local).map_err(|e| e.to_string())?;
    if !force {
        let info = get_branch_delete_info(repo, name)?;
        if !info.merged {
            return Err(format!("{name} is not fully merged — {} commit(s) would be lost.", info.commit_loss_count));
        }
    }
    let upstream = branch.upstream().ok();
    branch.delete().map_err(|e| e.to_string())?;
    if also_delete_remote {
        if let Some(up) = upstream {
            if let Some(up_name) = up.name().ok().flatten() {
                if let Some((remote_name, remote_branch)) = up_name.split_once('/') {
                    delete_remote_branch(repo, remote_name, remote_branch)?;
                }
            }
        }
    }
    Ok(())
}

/// Pushes a delete refspec (`:refs/heads/<branch>`) — the same mechanism `push_branch` uses for
/// an ordinary push, just with an empty source side, which is git's own convention for "delete
/// this ref on the remote".
fn delete_remote_branch(repo: &Repository, remote_name: &str, remote_branch: &str) -> Result<(), String> {
    let mut remote = repo.find_remote(remote_name).map_err(|e| e.to_string())?;
    let mut rejected: Vec<String> = Vec::new();
    {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(credentials_callback(repo));
        callbacks.push_update_reference(|refname, status| {
            if let Some(msg) = status {
                rejected.push(format!("{refname}: {msg}"));
            }
            Ok(())
        });
        let mut push_opts = git2::PushOptions::new();
        push_opts.remote_callbacks(callbacks);
        let refspec = format!(":refs/heads/{remote_branch}");
        remote.push(&[refspec], Some(&mut push_opts)).map_err(|e| e.to_string())?;
    }
    if !rejected.is_empty() {
        return Err(format!("Remote delete rejected: {}", rejected.join(", ")));
    }
    Ok(())
}

// --- Working tree staging (PRD §4.4, §8, SPEC.md item 5) ------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatusKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkingFileEntry {
    pub path: String,
    pub status: FileStatusKind,
    pub additions: u32,
    pub deletions: u32,
    /// Tri-state checkbox = `staged && !unstaged` → checked, `staged` → partial, else unchecked.
    pub staged: bool,
    pub unstaged: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkingTreeStatus {
    pub files: Vec<WorkingFileEntry>,
    pub branch_name: String,
    pub author_name: String,
    pub author_email: String,
    /// Drives the commit panel's SSH-sign toggle: pre-disabled (not just erroring at commit
    /// time) when `git config user.signingkey` is unset.
    pub has_signing_key: bool,
    /// Drives the amend toggle: disabled on an unborn HEAD (nothing to amend yet).
    pub can_amend: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StagedState {
    Staged,
    Unstaged,
}

#[derive(Debug, Clone, Serialize)]
pub struct HunkLineRow {
    pub kind: DiffLineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
    /// Only meaningful for addition/deletion lines — context lines are always `Staged` and
    /// never rendered with a gutter dot by the frontend.
    pub staged: StagedState,
    /// This line's position within `HunkRow.lines` — the stable per-line address passed back to
    /// `stage_line`/`unstage_line`. See `HunkRow.old_start`/`new_start` for why a position
    /// within the hunk, rather than an absolute line number, is what's stable here too.
    pub line_index_in_hunk: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HunkRow {
    /// This hunk's start line in HEAD's version of the file — stable across this diff (HEAD vs.
    /// workdir, used for display) and the HEAD-vs-index diff `unstage_hunk`/`unstage_line`
    /// actually operate on, since both use HEAD as their "old" side. NOT stable against the
    /// index-vs-workdir diff `stage_hunk`/`stage_line` operate on, since the index moves as
    /// other hunks in the same file get staged/unstaged — use `new_start` for those instead.
    pub old_start: u32,
    /// This hunk's start line in the *current working directory* file — stable across this diff
    /// and the index-vs-workdir diff `stage_hunk`/`stage_line` operate on, since both use the
    /// workdir as their "new" side (staging only ever touches the index, never the workdir).
    pub new_start: u32,
    pub header: String,
    pub lines: Vec<HunkLineRow>,
    /// Drives the green "stage hunk" / amber "unstage hunk" button flip.
    pub fully_staged: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileHunkDiff {
    pub path: String,
    pub hunks: Vec<HunkRow>,
    pub is_binary: bool,
}

fn working_tree_status(repo: &Repository) -> Result<WorkingTreeStatus, String> {
    let mut status_opts = git2::StatusOptions::new();
    status_opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut status_opts)).map_err(|e| e.to_string())?;

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let index = repo.index().map_err(|e| e.to_string())?;

    let unstaged_diff = repo
        .diff_index_to_workdir(Some(&index), None)
        .map_err(|e| e.to_string())?;
    let unstaged_by_path: HashMap<String, (u32, u32)> = diff_file_stats(&unstaged_diff)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| (f.path, (f.additions, f.deletions)))
        .collect();

    let staged_diff = repo
        .diff_tree_to_index(head_tree.as_ref(), Some(&index), None)
        .map_err(|e| e.to_string())?;
    let staged_by_path: HashMap<String, (u32, u32)> = diff_file_stats(&staged_diff)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| (f.path, (f.additions, f.deletions)))
        .collect();

    let mut files = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        if status.contains(git2::Status::IGNORED) {
            continue;
        }
        let Some(path) = entry.path().map(|p| p.to_string()) else { continue };

        let staged = status.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED
                | git2::Status::INDEX_TYPECHANGE,
        );
        let unstaged = status.intersects(
            git2::Status::WT_NEW
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_DELETED
                | git2::Status::WT_RENAMED
                | git2::Status::WT_TYPECHANGE,
        );

        let status_kind = if status.intersects(git2::Status::INDEX_DELETED | git2::Status::WT_DELETED) {
            FileStatusKind::Deleted
        } else if status.intersects(git2::Status::INDEX_NEW | git2::Status::WT_NEW) {
            FileStatusKind::Added
        } else {
            FileStatusKind::Modified
        };

        let (ua, ud) = unstaged_by_path.get(&path).copied().unwrap_or((0, 0));
        let (sa, sd) = staged_by_path.get(&path).copied().unwrap_or((0, 0));

        files.push(WorkingFileEntry {
            path,
            status: status_kind,
            additions: ua + sa,
            deletions: ud + sd,
            staged,
            unstaged,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let branch_name = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "HEAD".to_string());
    let (author_name, author_email) = repo
        .signature()
        .map(|s| (s.name().unwrap_or_default().to_string(), s.email().unwrap_or_default().to_string()))
        .unwrap_or_default();
    let has_signing_key = repo
        .config()
        .ok()
        .and_then(|c| c.get_string("user.signingkey").ok())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let can_amend = repo.head().and_then(|h| h.peel_to_commit()).is_ok();

    Ok(WorkingTreeStatus {
        files,
        branch_name,
        author_name,
        author_email,
        has_signing_key,
        can_amend,
    })
}

/// Set of `(kind, old_lineno, new_lineno)` triples touched by `diff` — used to mark which lines
/// of the combined HEAD-vs-workdir diff are already present in the index (i.e. already staged).
fn collect_line_set(diff: &git2::Diff) -> Result<HashSet<(DiffLineKind, Option<u32>, Option<u32>)>, git2::Error> {
    let mut set = HashSet::new();
    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            if let Some(kind) = line_kind(line.origin()) {
                set.insert((kind, line.old_lineno(), line.new_lineno()));
            }
            true
        }),
    )?;
    Ok(set)
}

/// Hunk-structured diff for one file (PRD §8) — the union of unstaged (index→workdir) and
/// already-staged (HEAD→index) changes for that file, so the UI can render one diff with a
/// per-line staged/unstaged gutter rather than two separate diffs. Computed via
/// `diff_tree_to_workdir_with_index` (HEAD vs workdir, using the index for stat info) for hunk
/// shape, cross-referenced against a `diff_tree_to_index` (HEAD vs index) line set to know which
/// of those lines are already staged.
fn working_file_diff(repo: &Repository, file_path: &str) -> Result<FileHunkDiff, String> {
    let index = repo.index().map_err(|e| e.to_string())?;
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());

    let mut full_opts = git2::DiffOptions::new();
    full_opts.pathspec(file_path);
    let full_diff = repo
        .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut full_opts))
        .map_err(|e| e.to_string())?;

    if full_diff.deltas().count() == 0 {
        return Ok(FileHunkDiff { path: file_path.to_string(), hunks: Vec::new(), is_binary: false });
    }
    if full_diff.deltas().next().map(|d| d.flags().contains(git2::DiffFlags::BINARY)).unwrap_or(false) {
        return Ok(FileHunkDiff { path: file_path.to_string(), hunks: Vec::new(), is_binary: true });
    }

    let mut staged_opts = git2::DiffOptions::new();
    staged_opts.pathspec(file_path);
    let staged_diff = repo
        .diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut staged_opts))
        .map_err(|e| e.to_string())?;
    let staged_lines = collect_line_set(&staged_diff).map_err(|e| e.to_string())?;

    let mut patch = git2::Patch::from_diff(&full_diff, 0)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No diff produced for this file.".to_string())?;

    let mut hunks = Vec::new();
    for h in 0..patch.num_hunks() {
        let (hunk, line_count) = patch.hunk(h).map_err(|e| e.to_string())?;
        let header = String::from_utf8_lossy(hunk.header()).trim_end().to_string();
        let mut lines = Vec::with_capacity(line_count);
        let mut any_unstaged = false;
        for li in 0..line_count {
            let line = patch.line_in_hunk(h, li).map_err(|e| e.to_string())?;
            let Some(kind) = line_kind(line.origin()) else { continue };
            let content = String::from_utf8_lossy(line.content()).trim_end_matches('\n').to_string();
            let key = (kind, line.old_lineno(), line.new_lineno());
            let staged = if kind == DiffLineKind::Context || staged_lines.contains(&key) {
                StagedState::Staged
            } else {
                StagedState::Unstaged
            };
            if kind != DiffLineKind::Context && staged == StagedState::Unstaged {
                any_unstaged = true;
            }
            let line_index_in_hunk = lines.len() as u32;
            lines.push(HunkLineRow {
                kind,
                old_lineno: line.old_lineno(),
                new_lineno: line.new_lineno(),
                content,
                staged,
                line_index_in_hunk,
            });
        }
        hunks.push(HunkRow {
            old_start: hunk.old_start(),
            new_start: hunk.new_start(),
            header,
            lines,
            fully_staged: !any_unstaged,
        });
    }

    Ok(FileHunkDiff { path: file_path.to_string(), hunks, is_binary: false })
}

fn stage_file(repo: &Repository, file_path: &str) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let path = std::path::Path::new(file_path);
    let exists_on_disk = repo
        .workdir()
        .map(|w| w.join(path).exists())
        .unwrap_or_else(|| path.exists());
    if exists_on_disk {
        index.add_path(path).map_err(|e| e.to_string())?;
    } else {
        index.remove_path(path).map_err(|e| e.to_string())?;
    }
    index.write().map_err(|e| e.to_string())
}

/// Whole-file unstage — equivalent to `git reset HEAD -- <path>`. On an unborn HEAD (no commits
/// yet) there's no tree to reset to, so "unstage" just means dropping the path from the index.
fn unstage_file(repo: &Repository, file_path: &str) -> Result<(), String> {
    match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(head_commit) => repo
            .reset_default(Some(head_commit.as_object()), [file_path])
            .map_err(|e| e.to_string()),
        Err(_) => {
            let mut index = repo.index().map_err(|e| e.to_string())?;
            index.remove_path(std::path::Path::new(file_path)).map_err(|e| e.to_string())?;
            index.write().map_err(|e| e.to_string())
        }
    }
}

struct RawLine {
    origin: char,
    content: Vec<u8>,
}

/// Builds a standalone single-hunk unified-diff buffer suitable for `Repository::apply` against
/// the index.
///
/// `keep` is `None` for a whole-hunk operation (every non-context line participates) or
/// `Some(line-indices-to-keep)` for a single-line toggle, identified by position within `lines`
/// (which is always freshly re-derived from the *same* diff direction this patch will be applied
/// against — see `apply_hunk_change` — so a position is stable here, unlike an absolute line
/// number, which isn't: see `HunkRow.old_start`/`new_start`'s doc comments for why). Known
/// limitation: this still assumes `lines` matches what the frontend saw when the hunk hadn't yet
/// been *partially* staged — toggling a second line of an already-partially-staged hunk without
/// an intervening refetch can address the wrong line, since the two diff directions' line sets
/// for that hunk only agree before any of it has been staged. The frontend always refetches
/// after each action, so this doesn't arise in normal use.
///
/// `reverse` is true for unstage operations, which are built from the HEAD→index diff (where
/// `+` means "already in the index") rather than the index→workdir diff (where `+` means "not
/// yet in the index") — so an *unselected* line's demotion rule (context vs. dropped) flips
/// depending on which of those two a `+`/`-` origin actually means "currently present in the
/// index", and the kept lines' signs are flipped at the end so applying the result to the index
/// actually moves it *back toward* HEAD instead of further from it.
fn build_partial_hunk_patch(
    file_path: &str,
    old_start: u32,
    new_start: u32,
    lines: &[RawLine],
    keep: Option<&HashSet<u32>>,
    reverse: bool,
) -> String {
    let mut body = String::new();
    let mut old_count = 0u32;
    let mut new_count = 0u32;
    // Real unified diffs mark a line lacking a trailing newline with a literal `\ No newline at
    // end of file` line immediately after it, rather than just silently omitting the `\n` —
    // omitting that marker (while still padding the patch *text* itself with a `\n` so the patch
    // stays line-oriented) made libgit2 believe the line's content disagreed with what's
    // actually on disk, which is what caused real "hunk did not apply" failures on files lacking
    // a final newline. Since only the file's true last line can lack one, it's always the last
    // line we actually emit (if any line lacks one at all), so a single flag checked once after
    // the loop is enough — no need to track it per-line.
    let mut last_emitted_no_newline = false;

    for (i, line) in lines.iter().enumerate() {
        let kept = keep.map(|k| k.contains(&(i as u32))).unwrap_or(true);
        let mut origin = line.origin;

        if origin != ' ' && !kept {
            // Whichever sign currently means "present in the index" for this diff direction
            // becomes context (still present, untouched by this operation); the other sign is
            // dropped entirely (it doesn't currently exist in the index, so it can't be a
            // context line).
            let currently_in_index = if reverse { origin == '+' } else { origin == '-' };
            if currently_in_index {
                origin = ' ';
            } else {
                continue;
            }
        }

        if reverse && (origin == '+' || origin == '-') {
            origin = if origin == '+' { '-' } else { '+' };
        }

        match origin {
            ' ' => {
                old_count += 1;
                new_count += 1;
            }
            '+' => new_count += 1,
            '-' => old_count += 1,
            _ => {}
        }

        body.push(origin);
        body.push_str(&String::from_utf8_lossy(&line.content));
        last_emitted_no_newline = !line.content.ends_with(b"\n");
        if last_emitted_no_newline {
            body.push('\n');
        }
    }

    if last_emitted_no_newline {
        body.push_str("\\ No newline at end of file\n");
    }

    let (header_old_start, header_new_start) = if reverse { (new_start, old_start) } else { (old_start, new_start) };
    format!(
        "diff --git a/{p} b/{p}\n--- a/{p}\n+++ b/{p}\n@@ -{ho},{oc} +{hn},{nc} @@\n{body}",
        p = file_path,
        ho = header_old_start,
        oc = old_count,
        hn = header_new_start,
        nc = new_count,
        body = body
    )
}

/// Locates a hunk by `new_start` (staging — see `HunkRow.new_start`'s doc comment) or by
/// `old_start` (unstaging — see `HunkRow.old_start`'s).
fn find_hunk(patch: &mut git2::Patch, position: u32, match_new_start: bool) -> Result<usize, String> {
    for h in 0..patch.num_hunks() {
        let (hunk, _) = patch.hunk(h).map_err(|e| e.to_string())?;
        let candidate = if match_new_start { hunk.new_start() } else { hunk.old_start() };
        if candidate == position {
            return Ok(h);
        }
    }
    Err("Hunk not found — the working tree may have changed since the diff was loaded.".to_string())
}

fn apply_patch_text_to_index(repo: &Repository, patch_text: &str) -> Result<(), String> {
    let diff = git2::Diff::from_buffer(patch_text.as_bytes()).map_err(|e| e.to_string())?;
    repo.apply(&diff, git2::ApplyLocation::Index, None).map_err(|e| e.to_string())
}

/// Shared mechanics for `stage_hunk`/`unstage_hunk`/`stage_line`/`unstage_line` — re-derives the
/// relevant diff direction fresh (never trusts a previously-fetched `FileHunkDiff`), locates the
/// target hunk, and applies a (possibly partial, possibly reversed) patch built from it.
///
/// `position` is `new_start` when `!reverse` (staging, against the index→workdir diff) or
/// `old_start` when `reverse` (unstaging, against the HEAD→index diff) — whichever axis that
/// diff direction actually shares with the diff the frontend displayed the hunk from. Using the
/// *other* axis (e.g. `old_start` to locate a hunk for staging) is exactly the bug this function
/// used to have: `old_start` is HEAD-relative, but the staging diff's "old" side is the *index*,
/// which drifts away from HEAD as soon as anything earlier in the same file gets staged —
/// `new_start` (workdir-relative) is what staging's diff actually shares with the display diff.
fn apply_hunk_change(
    repo: &Repository,
    file_path: &str,
    position: u32,
    keep: Option<&HashSet<u32>>,
    reverse: bool,
) -> Result<(), String> {
    let index = repo.index().map_err(|e| e.to_string())?;
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(file_path);
    let diff = if reverse {
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))
            .map_err(|e| e.to_string())?
    } else {
        repo.diff_index_to_workdir(Some(&index), Some(&mut opts)).map_err(|e| e.to_string())?
    };

    let mut patch = git2::Patch::from_diff(&diff, 0)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No matching changes for this file.".to_string())?;
    let h = find_hunk(&mut patch, position, !reverse)?;
    let (hunk, line_count) = patch.hunk(h).map_err(|e| e.to_string())?;
    let old_start = hunk.old_start();
    let new_start = hunk.new_start();

    // Only real content lines — `line_kind` already excludes the "no newline at end of file"
    // pseudo-lines libgit2 reports separately within a hunk's line count; this loop must filter
    // the same way so `lines`' indices line up with `HunkRow.lines`' (and so the EOF-newline
    // marker handling above, driven off the *real* lines' own raw bytes, isn't confused by an
    // extra pseudo-line entry).
    let mut lines = Vec::with_capacity(line_count);
    for li in 0..line_count {
        let line = patch.line_in_hunk(h, li).map_err(|e| e.to_string())?;
        if line_kind(line.origin()).is_none() {
            continue;
        }
        lines.push(RawLine { origin: line.origin(), content: line.content().to_vec() });
    }

    let text = build_partial_hunk_patch(file_path, old_start, new_start, &lines, keep, reverse);
    apply_patch_text_to_index(repo, &text)
}

fn stage_hunk(repo: &Repository, file_path: &str, new_start: u32) -> Result<(), String> {
    apply_hunk_change(repo, file_path, new_start, None, false)
}

fn unstage_hunk(repo: &Repository, file_path: &str, old_start: u32) -> Result<(), String> {
    apply_hunk_change(repo, file_path, old_start, None, true)
}

fn stage_line(repo: &Repository, file_path: &str, new_start: u32, line_index_in_hunk: u32) -> Result<(), String> {
    let mut keep = HashSet::new();
    keep.insert(line_index_in_hunk);
    apply_hunk_change(repo, file_path, new_start, Some(&keep), false)
}

fn unstage_line(repo: &Repository, file_path: &str, old_start: u32, line_index_in_hunk: u32) -> Result<(), String> {
    let mut keep = HashSet::new();
    keep.insert(line_index_in_hunk);
    apply_hunk_change(repo, file_path, old_start, Some(&keep), true)
}

/// Full message of HEAD's commit, for the amend toggle's message-textarea pre-fill — `None` on
/// an unborn HEAD (nothing to amend). Kept separate from `CommitDetail` (which belongs to the
/// commit-overlay feature) to keep this session's blast radius small.
fn last_commit_message(repo: &Repository) -> Result<Option<String>, String> {
    match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(commit) => Ok(Some(commit.message().unwrap_or_default().to_string())),
        Err(_) => Ok(None),
    }
}

/// Shells out to `ssh-keygen -Y sign` to produce an SSH commit signature — git2-rs has no
/// SSH-signing support of its own, and this is the same external mechanism plain `git`'s
/// `gpg.format=ssh` path uses internally, so there's no way to do this through libgit2 alone.
/// Returns the new (signed) commit's Oid; does not move any ref.
fn sign_and_create_commit(
    repo: &Repository,
    sig: &git2::Signature,
    message: &str,
    tree: &git2::Tree,
    parents: &[&git2::Commit],
) -> Result<Oid, String> {
    let key_path = repo
        .config()
        .map_err(|e| e.to_string())?
        .get_string("user.signingkey")
        .map_err(|_| "No SSH signing key configured (git config user.signingkey).".to_string())?;

    let buf = repo
        .commit_create_buffer(sig, sig, message, tree, parents)
        .map_err(|e| e.to_string())?;
    let content = std::str::from_utf8(&buf).map_err(|e| e.to_string())?.to_string();

    let pid = std::process::id();
    let scratch_dir = std::env::temp_dir();
    let scratch_path = scratch_dir.join(format!("trunk-commit-{pid}.txt"));
    let sig_path = scratch_dir.join(format!("trunk-commit-{pid}.txt.sig"));
    let cleanup = || {
        let _ = std::fs::remove_file(&scratch_path);
        let _ = std::fs::remove_file(&sig_path);
    };

    std::fs::write(&scratch_path, &content).map_err(|e| e.to_string())?;
    let output = std::process::Command::new("ssh-keygen")
        .args(["-Y", "sign", "-f", &key_path, "-n", "git"])
        .arg(&scratch_path)
        .output();
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            cleanup();
            return Err(format!("Failed to run ssh-keygen: {e}"));
        }
    };
    if !output.status.success() {
        cleanup();
        return Err(format!("ssh-keygen signing failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let signature = match std::fs::read_to_string(&sig_path) {
        Ok(s) => s,
        Err(e) => {
            cleanup();
            return Err(format!("Couldn't read signature file: {e}"));
        }
    };
    cleanup();

    repo.commit_signed(&content, &signature, None).map_err(|e| e.to_string())
}

/// Moves the current branch's tip to `new_oid` — used instead of `repo.commit(Some("HEAD"),
/// ...)`'s built-in ref update because that path requires the new commit's first parent to match
/// HEAD's *current* tip, which amend (parents = HEAD's own parents, not `[HEAD]`) deliberately
/// violates, and because `commit_signed` never takes an `update_ref` at all. Reads "HEAD"'s own
/// symbolic target directly (rather than `repo.head()`, which fails on an unborn branch) so this
/// works whether or not the branch already has a tip, and naturally refuses a detached HEAD
/// (whose target isn't symbolic).
fn move_head_to(repo: &Repository, new_oid: Oid, message: &str) -> Result<(), String> {
    let refname = repo
        .find_reference("HEAD")
        .ok()
        .and_then(|h| h.symbolic_target().map(|s| s.to_string()))
        .ok_or_else(|| "Can't determine the current branch to update (detached HEAD?).".to_string())?;
    repo.reference(&refname, new_oid, true, &format!("commit: {message}"))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Commits the current index (PRD §8). `amend` replaces HEAD with a new commit carrying HEAD's
/// own parents (i.e. skips past HEAD entirely, matching `git commit --amend`) instead of adding
/// HEAD as a parent.
fn commit_changes(repo: &Repository, message: &str, amend: bool, ssh_sign: bool) -> Result<String, String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;

    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if !amend {
        if let Some(hc) = &head_commit {
            if hc.tree_id() == tree_oid {
                return Err("Nothing staged to commit.".to_string());
            }
        }
    }

    let parents: Vec<git2::Commit> = if amend {
        match &head_commit {
            Some(hc) => hc.parents().collect(),
            None => return Err("Nothing to amend — no commits yet.".to_string()),
        }
    } else {
        head_commit.iter().cloned().collect()
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    let sig = repo.signature().map_err(|e| e.to_string())?;

    let new_oid = if ssh_sign {
        sign_and_create_commit(repo, &sig, message, &tree, &parent_refs)?
    } else {
        repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
            .map_err(|e| e.to_string())?
    };

    move_head_to(repo, new_oid, message)?;
    Ok(new_oid.to_string())
}

// --- Remote operations: push/fetch/pull (SPEC.md item 7, PRD §12) -------------------------

/// One local branch's upstream-tracking summary for the Push/Pull dialogs' from/to dropdowns.
#[derive(Debug, Clone, Serialize)]
pub struct RemoteBranchInfo {
    pub name: String,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}

/// One line of terminal-style push/fetch output (PRD §12's progress areas) — `Remote` is the
/// server's own sideband text (e.g. "Enumerating objects: 2073, done."), passed through verbatim
/// except for the `remote: ` prefix git's own CLI adds when displaying it; `Stage` is a raw
/// counter update for a client-computed stage ("Counting objects"/"Compressing objects" from
/// `pack_progress`, "Writing objects" from `push_transfer_progress`, "Receiving objects"/
/// "Resolving deltas" from `transfer_progress`). All percentage/byte-size/rate formatting — the
/// part that makes these counters look like `git`'s own output — is deliberately left to the
/// frontend, which already needs wall-clock timestamps (for rate) that don't belong here.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressEvent {
    Remote { text: String },
    Stage { stage: String, current: usize, total: usize, bytes: Option<usize> },
}

/// Splits a sideband chunk on `\r`/`\n` (the same characters git's own progress lines use to
/// overwrite themselves in a terminal) into the individual completed/in-flight lines it contains,
/// prefixing each with `remote: ` to match real git's display convention for this channel.
pub(crate) fn sideband_lines(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(data)
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("remote: {s}"))
        .collect()
}

/// Turns one `transfer_progress` tick into the "Receiving objects"/"Resolving deltas" line(s) it
/// represents (PRD §12) — shared by `fetch_remote` below and `workspace::clone_repository`
/// (clone's pack download is the exact same client-side transfer, just before any repo exists
/// yet to attach a `Repo`/`Repository` method to).
pub(crate) fn transfer_progress_events(progress: &git2::Progress<'_>) -> Vec<ProgressEvent> {
    let mut events = vec![ProgressEvent::Stage {
        stage: "Receiving objects".to_string(),
        current: progress.received_objects(),
        total: progress.total_objects(),
        bytes: Some(progress.received_bytes()),
    }];
    // Real git only starts showing "Resolving deltas" once receiving has finished — it can
    // technically interleave for thin packs, but matching that exactly would mean showing both
    // lines updating at once, which reads as more confusing, not more faithful.
    if progress.received_objects() == progress.total_objects() && progress.total_deltas() > 0 {
        events.push(ProgressEvent::Stage {
            stage: "Resolving deltas".to_string(),
            current: progress.indexed_deltas(),
            total: progress.total_deltas(),
            bytes: None,
        });
    }
    events
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchOutcome {
    /// Best-effort submodule-update failures (PRD §12.2's "fetch submodules" checkbox) — these
    /// don't fail the overall fetch, they're surfaced as warnings the caller can toast.
    pub submodule_warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullStrategy {
    Rebase,
    Merge,
    FfOnly,
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

/// Builds a `RemoteCallbacks::credentials` closure (PRD §12 "remote auth"): tries ssh-agent then
/// the user's default SSH keys for `SSH_KEY` requests, falls back to the system git's configured
/// `credential.helper` for username/password requests — i.e. whatever OS-native store
/// (osxkeychain/wincred/libsecret) the user's own git is already wired to, rather than a new
/// credential store of Trunk's own (CLAUDE.md's "OS-native credential storage" goal, and the
/// PRD's own note that Secret Service availability varies on Linux assumes exactly this path).
/// Capped at 3 attempts so a bad/missing credential fails cleanly instead of looping forever —
/// libgit2 re-invokes this callback on each rejected attempt.
fn credentials_callback(
    repo: &Repository,
) -> impl FnMut(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error> {
    let config = repo.config();
    let mut attempts = 0;
    move |url, username_from_url, allowed_types| {
        attempts += 1;
        if attempts > 3 {
            return Err(git2::Error::from_str("authentication failed after multiple attempts"));
        }
        let username = username_from_url.unwrap_or("git");
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            if let Ok(cred) = git2::Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }
            if let Some(home) = home_dir() {
                for key_name in ["id_ed25519", "id_rsa"] {
                    let private = home.join(".ssh").join(key_name);
                    if private.exists() {
                        if let Ok(cred) = git2::Cred::ssh_key(username, None, &private, None) {
                            return Ok(cred);
                        }
                    }
                }
            }
        }
        if allowed_types.contains(git2::CredentialType::USERNAME) {
            return git2::Cred::username(username);
        }
        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            if let Ok(cfg) = &config {
                if let Ok(cred) = git2::Cred::credential_helper(cfg, url, username_from_url) {
                    return Ok(cred);
                }
            }
        }
        Err(git2::Error::from_str("no usable credentials for this remote"))
    }
}

fn list_remotes(repo: &Repository) -> Result<Vec<String>, String> {
    Ok(repo
        .remotes()
        .map_err(|e| e.to_string())?
        .iter()
        .filter_map(|s| s.map(String::from))
        .collect())
}

fn remote_url(repo: &Repository, name: &str) -> Result<String, String> {
    repo.find_remote(name)
        .map_err(|e| e.to_string())?
        .url()
        .map(String::from)
        .ok_or_else(|| "Remote has no URL.".to_string())
}

fn list_local_branches_with_tracking(repo: &Repository) -> Result<Vec<RemoteBranchInfo>, String> {
    let mut out = Vec::new();
    for branch in repo.branches(Some(BranchType::Local)).map_err(|e| e.to_string())? {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        let name = match branch.name().map_err(|e| e.to_string())? {
            Some(n) => n.to_string(),
            None => continue,
        };
        let local_oid = branch.get().target();
        let upstream = branch.upstream().ok();
        let (upstream_name, ahead, behind) = match (&upstream, local_oid) {
            (Some(up), Some(local_oid)) => {
                let upstream_name = up.name().ok().flatten().map(String::from);
                let (ahead, behind) = match up.get().target() {
                    Some(up_oid) => repo.graph_ahead_behind(local_oid, up_oid).unwrap_or((0, 0)),
                    None => (0, 0),
                };
                (upstream_name, ahead, behind)
            }
            _ => (None, 0, 0),
        };
        out.push(RemoteBranchInfo { name, upstream: upstream_name, ahead, behind });
    }
    Ok(out)
}

/// Lightweight per-commit summary for the Push/Pull dialogs' ahead/incoming commit lists (PRD
/// §12.1/§12.3) — deliberately not `CommitDetail` (which also computes a full file/diff list,
/// far more than a one-line "SHA, message, author, time" row needs).
#[derive(Debug, Clone, Serialize)]
pub struct CommitSummary {
    pub sha: String,
    pub short_sha: String,
    pub author_name: String,
    pub author_email: String,
    pub summary: String,
    pub time: i64,
}

fn commit_summaries_between(repo: &Repository, from_oid: Oid, hide_oid: Option<Oid>) -> Result<Vec<CommitSummary>, String> {
    let mut walk = repo.revwalk().map_err(|e| e.to_string())?;
    walk.push(from_oid).map_err(|e| e.to_string())?;
    if let Some(hide_oid) = hide_oid {
        walk.hide(hide_oid).map_err(|e| e.to_string())?;
    }
    let mut out = Vec::new();
    for oid in walk {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let author = commit.author();
        let sha = oid.to_string();
        out.push(CommitSummary {
            short_sha: sha[..7].to_string(),
            sha,
            author_name: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            summary: commit.summary().unwrap_or_default().to_string(),
            time: commit.time().seconds(),
        });
    }
    Ok(out)
}

/// Local commits not yet on `remote_name`/`remote_branch` (Push dialog's commit-summary list).
fn list_commits_ahead(
    repo: &Repository,
    local_branch: &str,
    remote_name: &str,
    remote_branch: &str,
) -> Result<Vec<CommitSummary>, String> {
    let local_oid = repo
        .find_branch(local_branch, BranchType::Local)
        .map_err(|e| e.to_string())?
        .get()
        .target()
        .ok_or_else(|| "Local branch has no commits yet.".to_string())?;
    let remote_oid = repo.refname_to_id(&format!("refs/remotes/{remote_name}/{remote_branch}")).ok();
    commit_summaries_between(repo, local_oid, remote_oid)
}

/// Remote commits not yet merged into `local_branch` (Pull dialog's "incoming" commit list).
fn list_commits_behind(
    repo: &Repository,
    local_branch: &str,
    remote_name: &str,
    remote_branch: &str,
) -> Result<Vec<CommitSummary>, String> {
    let local_oid = repo
        .find_branch(local_branch, BranchType::Local)
        .map_err(|e| e.to_string())?
        .get()
        .target();
    let remote_oid = repo
        .refname_to_id(&format!("refs/remotes/{remote_name}/{remote_branch}"))
        .map_err(|e| e.to_string())?;
    commit_summaries_between(repo, remote_oid, local_oid)
}

/// Pushes `local_branch` to `remote_name`/`remote_branch` (PRD §12.1). `force_with_lease` is
/// emulated client-side since libgit2 has no native equivalent: re-fetch the remote-tracking ref
/// immediately before pushing and refuse if it moved since our last-known value — the same
/// protection real `--force-with-lease` gives, just implemented a layer up instead of inside
/// libgit2's push machinery.
fn push_branch(
    repo: &Repository,
    on_progress: impl FnMut(ProgressEvent),
    local_branch: &str,
    remote_name: &str,
    remote_branch: &str,
    set_upstream: bool,
    force: bool,
    force_with_lease: bool,
) -> Result<(), String> {
    repo.find_branch(local_branch, BranchType::Local)
        .map_err(|e| format!("local branch not found: {e}"))?;
    let mut remote = repo.find_remote(remote_name).map_err(|e| e.to_string())?;
    // Shared by every callback below (pack/transfer/sideband each fire independently, but never
    // concurrently — libgit2 calls them synchronously on this same thread) — a `RefCell` lets
    // each one borrow it for just the duration of its own invocation instead of all needing a
    // single, simultaneously-held `&mut` that the borrow checker can't reconcile across several
    // separate closures stored in the same `RemoteCallbacks`.
    let on_progress = std::cell::RefCell::new(on_progress);

    if force_with_lease {
        let tracking_ref_name = format!("refs/remotes/{remote_name}/{remote_branch}");
        let known_oid = repo.refname_to_id(&tracking_ref_name).ok();
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(credentials_callback(repo));
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        remote
            .fetch(&[remote_branch], Some(&mut fetch_opts), None)
            .map_err(|e| e.to_string())?;
        let current_oid = repo.refname_to_id(&tracking_ref_name).ok();
        if known_oid.is_some() && known_oid != current_oid {
            return Err(format!(
                "{remote_name}/{remote_branch} has new commits since your last fetch — fetch first to avoid overwriting them."
            ));
        }
    }

    let refspec = if force || force_with_lease {
        format!("+refs/heads/{local_branch}:refs/heads/{remote_branch}")
    } else {
        format!("refs/heads/{local_branch}:refs/heads/{remote_branch}")
    };

    let mut rejected: Vec<String> = Vec::new();
    {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(credentials_callback(repo));
        callbacks.pack_progress(|stage, current, total| {
            let stage = match stage {
                git2::PackBuilderStage::AddingObjects => "Counting objects",
                git2::PackBuilderStage::Deltafication => "Compressing objects",
            };
            on_progress.borrow_mut()(ProgressEvent::Stage { stage: stage.to_string(), current, total, bytes: None });
        });
        callbacks.push_transfer_progress(|current, total, bytes| {
            on_progress.borrow_mut()(ProgressEvent::Stage {
                stage: "Writing objects".to_string(),
                current,
                total,
                bytes: Some(bytes),
            });
        });
        callbacks.sideband_progress(|data| {
            for text in sideband_lines(data) {
                on_progress.borrow_mut()(ProgressEvent::Remote { text });
            }
            true
        });
        callbacks.push_update_reference(|refname, status| {
            if let Some(msg) = status {
                rejected.push(format!("{refname}: {msg}"));
            }
            Ok(())
        });
        let mut push_opts = git2::PushOptions::new();
        push_opts.remote_callbacks(callbacks);
        remote.push(&[refspec], Some(&mut push_opts)).map_err(|e| e.to_string())?;
    }
    if !rejected.is_empty() {
        return Err(format!("Push rejected: {}", rejected.join(", ")));
    }

    if set_upstream {
        let mut local = repo
            .find_branch(local_branch, BranchType::Local)
            .map_err(|e| e.to_string())?;
        local
            .set_upstream(Some(&format!("{remote_name}/{remote_branch}")))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Fetches one remote (or every remote when `remote_name` is `None`, PRD §12.2's "All remotes"
/// default). Submodule updates are best-effort and never fail the overall fetch.
fn fetch_remote(
    repo: &Repository,
    on_progress: impl FnMut(ProgressEvent),
    remote_name: Option<&str>,
    prune: bool,
    tags: bool,
    submodules: bool,
) -> Result<FetchOutcome, String> {
    let names: Vec<String> = match remote_name {
        Some(n) => vec![n.to_string()],
        None => list_remotes(repo)?,
    };
    // See `push_branch`'s matching comment — shared across `transfer_progress`/
    // `sideband_progress`, which fire independently but never concurrently.
    let on_progress = std::cell::RefCell::new(on_progress);
    for name in &names {
        let mut remote = repo.find_remote(name).map_err(|e| e.to_string())?;
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(credentials_callback(repo));
        callbacks.transfer_progress(|progress: git2::Progress<'_>| {
            let mut on_progress = on_progress.borrow_mut();
            for ev in transfer_progress_events(&progress) {
                on_progress(ev);
            }
            true
        });
        callbacks.sideband_progress(|data| {
            for text in sideband_lines(data) {
                on_progress.borrow_mut()(ProgressEvent::Remote { text });
            }
            true
        });
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        fetch_opts.prune(if prune { git2::FetchPrune::On } else { git2::FetchPrune::Unspecified });
        fetch_opts.download_tags(if tags { git2::AutotagOption::All } else { git2::AutotagOption::Auto });
        remote
            .fetch(&Vec::<String>::new(), Some(&mut fetch_opts), None)
            .map_err(|e| e.to_string())?;
    }

    let mut submodule_warnings = Vec::new();
    if submodules {
        if let Ok(subs) = repo.submodules() {
            for mut sub in subs {
                if let Err(e) = sub.update(true, None) {
                    submodule_warnings.push(format!("{}: {e}", sub.name().unwrap_or("submodule")));
                }
            }
        }
    }
    Ok(FetchOutcome { submodule_warnings })
}

/// Drives a started (or reopened) rebase to completion: applies + auto-commits every
/// non-conflicting step, stopping (returning `Conflict`, leaving `repo.state()` as `Rebase`) the
/// moment a step's index has conflicts. The conflict resolver's existing operation-agnostic
/// `conflict_status`/`conflict_file` already handle that state; resuming after resolution is
/// `finish_conflict_resolution`'s job (see its `Rebase` arm below), which re-enters this same
/// loop after committing the just-resolved step.
fn drive_rebase_to_completion(repo: &Repository, mut rebase: git2::Rebase<'_>) -> Result<ConflictableOutcome, String> {
    let committer = repo.signature().map_err(|e| e.to_string())?;
    loop {
        match rebase.next() {
            None => break,
            // A step whose change-set is already present in the new base (e.g. the upstream
            // side independently picked up an equivalent change) — libgit2 has already skipped
            // it internally; there's nothing to commit, just move on to the next step.
            Some(Err(e)) if e.code() == git2::ErrorCode::Applied => continue,
            Some(Err(e)) => return Err(e.to_string()),
            Some(Ok(_)) => {
                let index = repo.index().map_err(|e| e.to_string())?;
                if index.has_conflicts() {
                    return Ok(ConflictableOutcome::Conflict);
                }
                rebase.commit(None, &committer, None).map_err(|e| e.to_string())?;
            }
        }
    }
    rebase.finish(None).map_err(|e| e.to_string())?;
    let head_sha = repo
        .head()
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?
        .id()
        .to_string();
    Ok(ConflictableOutcome::Completed { sha: head_sha })
}

/// Pulls `remote_name`/`remote_branch` into `local_branch` (PRD §12.3): always fetches first,
/// then integrates per `strategy`. `Merge` conflicts reuse `finish_conflict_resolution`'s
/// existing `RepositoryState::Merge` handling unchanged (it already reads `MERGE_HEAD`);
/// `Rebase` conflicts are the one genuinely new control-flow path, handled by
/// `drive_rebase_to_completion` above plus this function's `Rebase` arm of
/// `finish_conflict_resolution`.
fn pull_branch(
    repo: &Repository,
    on_progress: impl FnMut(ProgressEvent),
    local_branch: &str,
    remote_name: &str,
    remote_branch: &str,
    strategy: PullStrategy,
) -> Result<ConflictableOutcome, String> {
    if !is_working_tree_clean(repo).map_err(|e| e.to_string())? {
        return Err("Commit or stash your changes before pulling.".to_string());
    }
    fetch_remote(repo, on_progress, Some(remote_name), false, false, false)?;

    let tracking_ref_name = format!("refs/remotes/{remote_name}/{remote_branch}");
    let upstream_oid = repo.refname_to_id(&tracking_ref_name).map_err(|e| e.to_string())?;
    let upstream_ac = repo.find_annotated_commit(upstream_oid).map_err(|e| e.to_string())?;
    let local_branch_ref = repo
        .find_branch(local_branch, BranchType::Local)
        .map_err(|e| e.to_string())?;
    let local_oid = local_branch_ref
        .get()
        .target()
        .ok_or_else(|| "Local branch has no commits yet.".to_string())?;
    let local_ac = repo.find_annotated_commit(local_oid).map_err(|e| e.to_string())?;

    let (analysis, _) = repo
        .merge_analysis_for_ref(local_branch_ref.get(), &[&upstream_ac])
        .map_err(|e| e.to_string())?;

    if analysis.is_up_to_date() {
        return Ok(ConflictableOutcome::Completed { sha: local_oid.to_string() });
    }

    match strategy {
        PullStrategy::FfOnly => {
            if !analysis.is_fast_forward() {
                return Err("Can't fast-forward — local and remote have diverged.".to_string());
            }
            let refname = local_branch_ref
                .get()
                .name()
                .ok_or_else(|| "Local branch ref has no name.".to_string())?
                .to_string();
            repo.reference(&refname, upstream_oid, true, "pull: fast-forward")
                .map_err(|e| e.to_string())?;
            repo.set_head(&refname).map_err(|e| e.to_string())?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
                .map_err(|e| e.to_string())?;
            Ok(ConflictableOutcome::Completed { sha: upstream_oid.to_string() })
        }
        PullStrategy::Merge => {
            if analysis.is_fast_forward() {
                let refname = local_branch_ref
                    .get()
                    .name()
                    .ok_or_else(|| "Local branch ref has no name.".to_string())?
                    .to_string();
                repo.reference(&refname, upstream_oid, true, "pull: fast-forward")
                    .map_err(|e| e.to_string())?;
                repo.set_head(&refname).map_err(|e| e.to_string())?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
                    .map_err(|e| e.to_string())?;
                return Ok(ConflictableOutcome::Completed { sha: upstream_oid.to_string() });
            }
            let mut checkout_builder = git2::build::CheckoutBuilder::new();
            checkout_builder.conflict_style_diff3(true);
            let mut merge_opts = git2::MergeOptions::new();
            repo.merge(&[&upstream_ac], Some(&mut merge_opts), Some(&mut checkout_builder))
                .map_err(|e| e.to_string())?;
            let mut index = repo.index().map_err(|e| e.to_string())?;
            if index.has_conflicts() {
                return Ok(ConflictableOutcome::Conflict);
            }
            let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
            let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
            let local_commit = repo.find_commit(local_oid).map_err(|e| e.to_string())?;
            let upstream_commit = repo.find_commit(upstream_oid).map_err(|e| e.to_string())?;
            let sig = repo.signature().map_err(|e| e.to_string())?;
            let message = format!("Merge {remote_name}/{remote_branch} into {local_branch}");
            let new_oid = repo
                .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&local_commit, &upstream_commit])
                .map_err(|e| e.to_string())?;
            repo.cleanup_state().map_err(|e| e.to_string())?;
            Ok(ConflictableOutcome::Completed { sha: new_oid.to_string() })
        }
        PullStrategy::Rebase => {
            if analysis.is_fast_forward() {
                let refname = local_branch_ref
                    .get()
                    .name()
                    .ok_or_else(|| "Local branch ref has no name.".to_string())?
                    .to_string();
                repo.reference(&refname, upstream_oid, true, "pull: fast-forward")
                    .map_err(|e| e.to_string())?;
                repo.set_head(&refname).map_err(|e| e.to_string())?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
                    .map_err(|e| e.to_string())?;
                return Ok(ConflictableOutcome::Completed { sha: upstream_oid.to_string() });
            }
            let rebase = repo
                .rebase(Some(&local_ac), Some(&upstream_ac), None, None)
                .map_err(|e| e.to_string())?;
            drive_rebase_to_completion(repo, rebase)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> (std::path::PathBuf, Repository) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "trunk-graph-test-{n}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        (dir, repo)
    }

    fn commit(repo: &Repository, message: &str, parents: &[&git2::Commit]) -> Oid {
        let sig = repo.signature().unwrap();
        let tree_oid = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, parents)
            .unwrap()
    }

    /// Like `commit()` above, but actually writes `file_name` to the working directory first —
    /// needed for commit-detail/diff tests, which (unlike the graph-walk tests) need real file
    /// content to diff against, not just empty-tree commits.
    fn commit_with_file(
        repo: &Repository,
        dir: &std::path::Path,
        message: &str,
        parents: &[&git2::Commit],
        file_name: &str,
        content: &str,
    ) -> Oid {
        std::fs::write(dir.join(file_name), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(file_name)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, parents)
            .unwrap()
    }

    #[test]
    fn build_graph_on_linear_history_marks_head_and_colors() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit(&repo, "second", &[&c1_commit]);

        let cache = build_graph(&repo).unwrap();
        assert_eq!(cache.rows.len(), 2);
        assert!(cache.rows[0].is_head, "newest commit should be HEAD");
        assert_eq!(cache.rows[0].lane_color_index, 1, "HEAD pinned to lane colour 1");
        for row in &cache.rows {
            assert!((1..=7).contains(&row.lane_color_index));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn build_graph_on_merge_produces_a_connector_and_branch_pill() {
        let (dir, repo) = temp_repo();
        let base = commit(&repo, "base", &[]);
        let base_commit = repo.find_commit(base).unwrap();
        let main_tip = commit(&repo, "main work", &[&base_commit]);
        let main_tip_commit = repo.find_commit(main_tip).unwrap();

        repo.branch("feature", &base_commit, false).unwrap();
        let feature_tip = {
            let sig = repo.signature().unwrap();
            let tree_oid = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            repo.commit(
                Some("refs/heads/feature"),
                &sig,
                &sig,
                "feature work",
                &tree,
                &[&base_commit],
            )
            .unwrap()
        };
        let feature_tip_commit = repo.find_commit(feature_tip).unwrap();

        let merge_oid = commit(&repo, "merge feature", &[&main_tip_commit, &feature_tip_commit]);

        let cache = build_graph(&repo).unwrap();
        let merge_row = cache.rows.iter().find(|r| r.sha == merge_oid.to_string()).unwrap();
        assert_eq!(merge_row.parents.len(), 2);
        assert!(
            !merge_row.connectors.is_empty(),
            "merge commit should carry at least one lane connector"
        );

        let feature_row = cache
            .rows
            .iter()
            .find(|r| r.sha == feature_tip.to_string())
            .unwrap();
        assert!(
            feature_row.refs.iter().any(|b| b.name == "feature" && b.kind == RefKind::Local),
            "feature branch tip should carry a Local ref badge"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn branch_ancestors_walks_full_parent_chain() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        let c2 = commit(&repo, "second", &[&c1_commit]);

        let cache = build_graph(&repo).unwrap();
        let branch_name = repo.head().unwrap().shorthand().unwrap().to_string();
        let ancestors = cache.branch_ancestors(&branch_name);
        assert!(ancestors.contains(&c1.to_string()));
        assert!(ancestors.contains(&c2.to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn commit_detail_reports_per_file_add_delete_counts() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "add a", &[], "a.txt", "one\ntwo\nthree\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let c2 = commit_with_file(&repo, &dir, "change a", &[&c1_commit], "a.txt", "one\nTWO\nthree\nfour\n");

        let root_detail = commit_detail(&repo, &c1.to_string()).unwrap();
        assert_eq!(root_detail.files.len(), 1);
        assert_eq!(root_detail.files[0].path, "a.txt");
        assert_eq!(root_detail.files[0].additions, 3, "root commit: every line is an addition");
        assert_eq!(root_detail.files[0].deletions, 0);

        let child_detail = commit_detail(&repo, &c2.to_string()).unwrap();
        assert_eq!(child_detail.files.len(), 1);
        assert_eq!(child_detail.files[0].additions, 2, "\"TWO\" + \"four\"");
        assert_eq!(child_detail.files[0].deletions, 1, "\"two\"");

        let diff_lines = commit_file_diff(&repo, &c2.to_string(), "a.txt").unwrap();
        assert!(diff_lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Addition && l.content == "TWO"));
        assert!(diff_lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Deletion && l.content == "two"));
        assert!(diff_lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Context && l.content == "one"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cherry_pick_and_revert_reject_merge_commits() {
        let (dir, repo) = temp_repo();
        let base = commit(&repo, "base", &[]);
        let base_commit = repo.find_commit(base).unwrap();
        let main_tip = commit(&repo, "main work", &[&base_commit]);
        let main_tip_commit = repo.find_commit(main_tip).unwrap();

        // Committed onto a separate ref, not HEAD — `repo.commit(Some("HEAD"), ...)` requires
        // the new commit's first parent to match HEAD's *current* tip (same constraint the
        // existing `build_graph_on_merge_...` test above already works around the same way),
        // and `base_commit` isn't `main_tip`.
        let feature_tip = {
            let sig = repo.signature().unwrap();
            let tree_oid = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            repo.commit(Some("refs/heads/feature"), &sig, &sig, "feature work", &tree, &[&base_commit])
                .unwrap()
        };
        let feature_tip_commit = repo.find_commit(feature_tip).unwrap();
        let merge_oid = commit(&repo, "merge feature", &[&main_tip_commit, &feature_tip_commit]);
        let merge_sha = merge_oid.to_string();

        assert!(cherry_pick(&repo, &merge_sha, false).is_err(), "cherry-pick must refuse a merge commit");
        assert!(revert_commit(&repo, &merge_sha, false).is_err(), "revert must refuse a merge commit");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn is_working_tree_clean_detects_untracked_file() {
        let (dir, repo) = temp_repo();
        assert!(is_working_tree_clean(&repo).unwrap());
        std::fs::write(dir.join("untracked.txt"), "stray").unwrap();
        assert!(!is_working_tree_clean(&repo).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // --- Staging & committing (SPEC.md item 5, PRD §4.4/§8) ---------------------------------

    #[test]
    fn working_tree_status_reports_new_modified_and_partially_staged_files() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\ntwo\nthree\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "one\ntwo\nthree\n");

        // Untracked new file.
        std::fs::write(dir.join("b.txt"), "new file\n").unwrap();
        // Modified-but-unstaged file.
        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();

        let status = working_tree_status(&repo).unwrap();
        let a = status.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!(a.status, FileStatusKind::Modified);
        assert!(a.unstaged && !a.staged);

        let b = status.files.iter().find(|f| f.path == "b.txt").unwrap();
        assert_eq!(b.status, FileStatusKind::Added);
        assert!(b.unstaged && !b.staged);

        assert!(status.can_amend);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stage_file_and_unstage_file_roundtrip() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\ntwo\nthree\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "one\ntwo\nthree\n");
        std::fs::write(dir.join("a.txt"), "one\nTWO\nthree\n").unwrap();

        stage_file(&repo, "a.txt").unwrap();
        let status = working_tree_status(&repo).unwrap();
        let a = status.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert!(a.staged && !a.unstaged, "fully staged after stage_file");

        unstage_file(&repo, "a.txt").unwrap();
        let status = working_tree_status(&repo).unwrap();
        let a = status.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert!(!a.staged && a.unstaged, "back to unstaged after unstage_file");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stage_hunk_then_unstage_hunk_is_a_full_roundtrip() {
        let (dir, repo) = temp_repo();
        let base_lines: Vec<String> = (1..=24).map(|n| format!("line{n}")).collect();
        let base_content = format!("{}\n", base_lines.join("\n"));
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", &base_content);
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", &base_content);

        // Two separate hunks — far enough apart (20+ unchanged lines between them) not to be
        // merged into one by libgit2's default 3-line context radius.
        let mut edited_lines = base_lines.clone();
        edited_lines[0] = "ONE".to_string();
        let last = edited_lines.len() - 1;
        edited_lines[last] = "TWENTYFOUR".to_string();
        let edited_content = format!("{}\n", edited_lines.join("\n"));
        std::fs::write(dir.join("a.txt"), &edited_content).unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert!(diff.hunks.len() >= 2, "expected two separate hunks, got {}", diff.hunks.len());
        assert!(diff.hunks.iter().all(|h| !h.fully_staged));

        let first_hunk_old_start = diff.hunks[0].old_start;
        let first_hunk_new_start = diff.hunks[0].new_start;
        stage_hunk(&repo, "a.txt", first_hunk_new_start).unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        let first = diff.hunks.iter().find(|h| h.old_start == first_hunk_old_start).unwrap();
        assert!(first.fully_staged, "first hunk should now be fully staged");
        assert!(diff.hunks.iter().any(|h| !h.fully_staged), "second hunk still unstaged");

        unstage_hunk(&repo, "a.txt", first_hunk_old_start).unwrap();
        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert!(
            diff.hunks.iter().all(|h| !h.fully_staged),
            "unstage_hunk should fully revert the hunk back to unstaged"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Regression test for the reported "hunk did not apply" bug: staging a file's first hunk
    /// whose added/removed line counts are *unequal* shifts every later hunk's position in the
    /// index relative to HEAD — `old_start` (HEAD-relative) no longer matches that later hunk's
    /// position in the index→workdir diff `stage_hunk` actually operates against, but
    /// `new_start` (workdir-relative) still does, since staging never touches the workdir.
    #[test]
    fn staging_an_earlier_unequal_length_hunk_does_not_break_staging_a_later_one() {
        let (dir, repo) = temp_repo();
        let base_lines: Vec<String> = (1..=30).map(|n| format!("line{n}")).collect();
        let base_content = format!("{}\n", base_lines.join("\n"));
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", &base_content);
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", &base_content);

        // First hunk: replace one line with three (net +2 lines) — an unequal-length change.
        // Second hunk: a simple one-line replace near the end, far enough away to stay separate.
        let mut edited_lines = base_lines.clone();
        edited_lines.splice(0..1, ["ONE-A".to_string(), "ONE-B".to_string(), "ONE-C".to_string()]);
        let last = edited_lines.len() - 1;
        edited_lines[last] = "THIRTY".to_string();
        let edited_content = format!("{}\n", edited_lines.join("\n"));
        std::fs::write(dir.join("a.txt"), &edited_content).unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert_eq!(diff.hunks.len(), 2, "expected two separate hunks");
        let first_new_start = diff.hunks[0].new_start;

        stage_hunk(&repo, "a.txt", first_new_start).unwrap();

        // Re-fetch fresh (as the real UI always does) and stage the second hunk by ITS
        // freshly-recomputed `new_start` — this used to fail with libgit2's ApplyFailed because
        // the old code matched on `old_start` against the index→workdir diff, whose "old" side
        // (the index) had already shifted by +2 lines relative to HEAD once the first hunk was
        // staged.
        let diff = working_file_diff(&repo, "a.txt").unwrap();
        let second = diff.hunks.iter().find(|h| !h.fully_staged).expect("second hunk still unstaged");
        let second_new_start = second.new_start;

        stage_hunk(&repo, "a.txt", second_new_start).unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert!(diff.hunks.iter().all(|h| h.fully_staged), "both hunks should now be staged");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stage_line_stages_only_the_selected_line() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\ntwo\nthree\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "one\ntwo\nthree\n");
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\nFOUR\nFIVE\n").unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert_eq!(diff.hunks.len(), 1);
        let hunk = &diff.hunks[0];
        let added_four = hunk
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Addition && l.content == "FOUR")
            .unwrap();

        stage_line(&repo, "a.txt", hunk.new_start, added_four.line_index_in_hunk).unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        let hunk = &diff.hunks[0];
        assert!(!hunk.fully_staged, "only one of two added lines was staged");
        let four_line = hunk.lines.iter().find(|l| l.content == "FOUR").unwrap();
        assert_eq!(four_line.staged, StagedState::Staged);
        let five_line = hunk.lines.iter().find(|l| l.content == "FIVE").unwrap();
        assert_eq!(five_line.staged, StagedState::Unstaged);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Regression test for the EOF-newline patch bug: a file with no trailing newline, where the
    /// hunk being staged includes that final, newline-less line as unchanged context. The old
    /// code fabricated a `\n` in the patch text with no `\ No newline at end of file` marker,
    /// which made libgit2 reject the patch as not matching the actual on-disk content.
    #[test]
    fn stage_hunk_succeeds_on_a_file_with_no_trailing_newline() {
        let (dir, repo) = temp_repo();
        // No trailing newline after "three" — note the lack of `\n` here.
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\ntwo\nthree");
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "one\ntwo\nthree");
        // Change the first line; "three" (still the last line, still newline-less) stays as
        // unchanged context within the same hunk.
        std::fs::write(dir.join("a.txt"), "ONE\ntwo\nthree").unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert_eq!(diff.hunks.len(), 1);
        let new_start = diff.hunks[0].new_start;

        stage_hunk(&repo, "a.txt", new_start).unwrap();

        let diff = working_file_diff(&repo, "a.txt").unwrap();
        assert!(diff.hunks[0].fully_staged);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn commit_changes_writes_a_new_commit_and_refuses_when_nothing_staged() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let _c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "one\n");

        assert!(
            commit_changes(&repo, "nothing to commit", false, false).is_err(),
            "must refuse when the index matches HEAD's tree"
        );

        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        stage_file(&repo, "a.txt").unwrap();
        let new_sha = commit_changes(&repo, "add a second line", false, false).unwrap();

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.id().to_string(), new_sha);
        assert_eq!(head.message().unwrap(), "add a second line");
        assert_eq!(head.parent_count(), 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn commit_changes_amend_replaces_head_keeping_its_parents() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let original_head = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "one\ntwo\n");

        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        stage_file(&repo, "a.txt").unwrap();
        let amended_sha = commit_changes(&repo, "amended message", true, false).unwrap();

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.id().to_string(), amended_sha);
        assert_ne!(head.id(), original_head, "amend must produce a new commit object");
        assert_eq!(head.message().unwrap(), "amended message");
        assert_eq!(head.parent_count(), 1);
        assert_eq!(head.parent_id(0).unwrap(), c1, "amend keeps HEAD's own parent, not HEAD itself");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn last_commit_message_is_none_on_unborn_head() {
        let (dir, repo) = temp_repo();
        assert_eq!(last_commit_message(&repo).unwrap(), None);
        let c1 = commit_with_file(&repo, &dir, "first message", &[], "a.txt", "x\n");
        let _ = repo.find_commit(c1).unwrap();
        assert_eq!(last_commit_message(&repo).unwrap(), Some("first message".to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // --- Conflict resolver (SPEC.md item 6, PRD §4.6/§9) ------------------------------------

    /// Builds two branches that both edit `a.txt`'s middle line, diverging from a common base —
    /// guarantees a same-line content conflict. `main` (HEAD) ends up at the tip carrying "MAIN";
    /// `feature` carries "FEATURE", never checked out. Returns `(dir, repo, main_tip, feature_oid)`.
    fn setup_conflicting_branches() -> (std::path::PathBuf, Repository, Oid, Oid) {
        let (dir, repo) = temp_repo();
        let base = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\ntwo\nthree\n");

        std::fs::write(dir.join("a.txt"), "one\nFEATURE\nthree\n").unwrap();
        let feature_oid = {
            let base_commit = repo.find_commit(base).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("a.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("refs/heads/feature"), &sig, &sig, "feature change", &tree, &[&base_commit])
                .unwrap()
        };

        let main_tip = {
            let base_commit = repo.find_commit(base).unwrap();
            commit_with_file(&repo, &dir, "main change", &[&base_commit], "a.txt", "one\nMAIN\nthree\n")
        };
        (dir, repo, main_tip, feature_oid)
    }

    #[test]
    fn cherry_pick_no_commit_applies_to_index_without_creating_a_commit() {
        let (dir, repo) = temp_repo();
        let base = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\n");
        let base_commit = repo.find_commit(base).unwrap();

        // A non-conflicting commit on a separate branch — adds a new file, nothing overlapping
        // with `a.txt`, so this cherry-pick always applies cleanly.
        std::fs::write(dir.join("b.txt"), "new file\n").unwrap();
        let feature_oid = {
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("b.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("refs/heads/feature"), &sig, &sig, "add b", &tree, &[&base_commit])
                .unwrap()
        };
        // The index/workdir still carry `b.txt` from building the feature commit above (it was
        // never committed to HEAD's own branch) — reset back to a clean tree matching HEAD so
        // `cherry_pick`'s dirty-tree precheck doesn't reject the call below.
        repo.reset(base_commit.as_object(), git2::ResetType::Hard, None).unwrap();

        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();
        let outcome = cherry_pick(&repo, &feature_oid.to_string(), true).unwrap();
        assert!(matches!(outcome, ConflictableOutcome::AppliedNoCommit));

        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();
        assert_eq!(head_before, head_after, "no-commit cherry-pick must not move HEAD");
        assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap(), "new file\n");

        let status = working_tree_status(&repo).unwrap();
        let b = status.files.iter().find(|f| f.path == "b.txt").unwrap();
        assert!(b.staged, "no-commit cherry-pick should leave the change staged");

        // Regression: libgit2's `cherrypick()` always writes `CHERRY_PICK_HEAD`/sets the repo
        // state to "in progress", unlike plain `git cherry-pick -n` on a single non-conflicting
        // commit, which leaves no lingering sequencer state. Without `cleanup_state()`, this
        // state was wrongly read back as a real conflict (`has_conflict()` checks `state() !=
        // Clean`), surfacing a phantom "Resolve conflicts" button whose Abort then discarded
        // these intentionally-uncommitted changes.
        assert_eq!(repo.state(), git2::RepositoryState::Clean);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn revert_no_commit_applies_to_index_without_creating_a_commit() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "first", &[], "a.txt", "one\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        let c2 = commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "two\n");

        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();
        let outcome = revert_commit(&repo, &c2.to_string(), true).unwrap();
        assert!(matches!(outcome, ConflictableOutcome::AppliedNoCommit));

        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();
        assert_eq!(head_before, head_after, "no-commit revert must not move HEAD");
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");

        let status = working_tree_status(&repo).unwrap();
        let a = status.files.iter().find(|f| f.path == "a.txt").unwrap();
        assert!(a.staged, "no-commit revert should leave the change staged");

        // Same regression as `cherry_pick_no_commit_applies_to_index_without_creating_a_commit`
        // — `REVERT_HEAD`/state must be cleaned up so this doesn't read back as a phantom conflict.
        assert_eq!(repo.state(), git2::RepositoryState::Clean);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cherry_pick_conflict_is_reported_not_aborted() {
        let (dir, repo, _main_tip, feature_oid) = setup_conflicting_branches();

        let outcome = cherry_pick(&repo, &feature_oid.to_string(), false).unwrap();
        assert!(matches!(outcome, ConflictableOutcome::Conflict));
        assert!(repo.index().unwrap().has_conflicts(), "conflict must be left in place, not aborted");
        assert_eq!(repo.state(), git2::RepositoryState::CherryPick);

        let session = conflict_status(&repo).unwrap().expect("a conflict session should be reported");
        assert_eq!(session.operation, "cherry-pick");
        assert_eq!(session.files, vec!["a.txt".to_string()]);

        let segments = conflict_file(&repo, "a.txt").unwrap();
        let (ours, base, theirs) = segments
            .iter()
            .find_map(|s| match s {
                ConflictSegment::Conflict { ours, base, theirs } => Some((ours.clone(), base.clone(), theirs.clone())),
                _ => None,
            })
            .expect("file should contain exactly one conflict segment");
        assert_eq!(ours, vec!["MAIN".to_string()]);
        assert_eq!(base, vec!["two".to_string()]);
        assert_eq!(theirs, vec!["FEATURE".to_string()]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finish_conflict_resolution_commits_resolved_content_and_clears_state() {
        let (dir, repo, _main_tip, feature_oid) = setup_conflicting_branches();
        cherry_pick(&repo, &feature_oid.to_string(), false).unwrap();

        let resolved = vec![ResolvedFile { path: "a.txt".to_string(), content: "one\nFEATURE\nthree\n".to_string() }];
        let outcome = finish_conflict_resolution(&repo, resolved).unwrap();
        let ConflictableOutcome::Completed { sha: new_sha } = outcome else {
            panic!("expected Completed, got {outcome:?}");
        };

        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(!repo.index().unwrap().has_conflicts());
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.id().to_string(), new_sha);
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\nFEATURE\nthree\n");
        assert_eq!(head.message().unwrap(), "feature change\n", "cherry-pick keeps the original commit message");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn abort_conflict_resolution_restores_pre_cherry_pick_tree() {
        let (dir, repo, main_tip, feature_oid) = setup_conflicting_branches();
        cherry_pick(&repo, &feature_oid.to_string(), false).unwrap();

        abort_in_progress_operation(&repo).unwrap();

        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(!repo.index().unwrap().has_conflicts());
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.id(), main_tip, "HEAD unchanged after abort");
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "one\nMAIN\nthree\n",
            "working tree restored to pre-cherry-pick content"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    // --- Push/Fetch/Pull (SPEC.md item 7, PRD §12) -----------------------------------------

    /// Creates a bare "remote" repo with one commit on `refs/heads/main`, writing objects
    /// directly (no working tree on a bare repo to write through) — and a local clone of it with
    /// "origin" already wired up exactly as `git clone` would. Returns `(remote_dir, local_dir,
    /// local_repo, initial_oid)`.
    fn remote_and_clone_pair() -> (std::path::PathBuf, std::path::PathBuf, Repository, Oid) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        // Nanosecond suffix (matching `temp_repo()`'s convention) — without it, a leftover
        // directory from a previous *panicked* run (which skips `cleanup_pair`) would collide
        // with this run's `COUNTER`, which always restarts at 0, and its stale refs/objects
        // would make these tests fail in baffling, run-order-dependent ways.
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("trunk-remote-test-{n}-{nanos}"));
        let remote_dir = base.join("remote");
        let local_dir = base.join("local");
        std::fs::create_dir_all(&remote_dir).unwrap();

        let remote_repo = Repository::init_bare(&remote_dir).unwrap();
        {
            let mut config = remote_repo.config().unwrap();
            config.set_str("user.name", "Test").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }
        let initial_oid = commit_blob(&remote_repo, "initial", &[], "a.txt", "one\ntwo\nthree\n");
        remote_repo.set_head("refs/heads/main").unwrap();

        let local_repo = Repository::clone(remote_dir.to_str().unwrap(), &local_dir).unwrap();
        {
            let mut config = local_repo.config().unwrap();
            config.set_str("user.name", "Test").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }
        {
            let mut local_main = local_repo.find_branch("main", BranchType::Local).unwrap();
            local_main.set_upstream(Some("origin/main")).unwrap();
        }

        (remote_dir, local_dir, local_repo, initial_oid)
    }

    /// Like `commit_with_file`, but writes the blob/tree directly through the object database
    /// instead of through a working tree + index — the only way to add a commit to a bare repo
    /// (no working tree to write to) or to simulate "someone else pushed" to the remote side of
    /// a push/fetch/pull test without a second working copy.
    fn commit_blob(repo: &Repository, message: &str, parents: &[&git2::Commit], file_name: &str, content: &str) -> Oid {
        let blob_oid = repo.blob(content.as_bytes()).unwrap();
        // Seeded from the first parent's tree (not `None`, an empty tree) so this commit's tree
        // carries forward every other file the parent had — otherwise each call would silently
        // drop every file but the one it's touching, which only happened to go unnoticed by
        // earlier (single-file) tests.
        let parent_tree = parents.first().and_then(|p| p.tree().ok());
        let mut builder = repo.treebuilder(parent_tree.as_ref()).unwrap();
        builder.insert(file_name, blob_oid, 0o100644).unwrap();
        let tree_oid = builder.write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("refs/heads/main"), &sig, &sig, message, &tree, parents).unwrap()
    }

    fn cleanup_pair(remote_dir: &std::path::Path, local_dir: &std::path::Path) {
        std::fs::remove_dir_all(remote_dir.parent().unwrap()).unwrap();
        let _ = local_dir;
    }

    #[test]
    fn list_local_branches_with_tracking_reports_ahead_and_behind() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();

        // Remote gains a commit the local clone hasn't fetched yet.
        let remote_repo = Repository::open(&remote_dir).unwrap();
        let initial_commit = remote_repo.find_commit(initial_oid).unwrap();
        commit_blob(&remote_repo, "remote-only", &[&initial_commit], "b.txt", "remote change\n");

        // Local gains a commit of its own the remote doesn't have.
        let local_initial = local_repo.find_commit(initial_oid).unwrap();
        commit_blob(&local_repo, "local-only", &[&local_initial], "c.txt", "local change\n");

        fetch_remote(&local_repo, |_| {}, Some("origin"), false, false, false).unwrap();

        let branches = list_local_branches_with_tracking(&local_repo).unwrap();
        let main = branches.iter().find(|b| b.name == "main").unwrap();
        assert_eq!(main.ahead, 1, "local has one commit not on origin/main");
        assert_eq!(main.behind, 1, "origin/main has one commit not yet merged locally");

        cleanup_pair(&remote_dir, &local_dir);
    }

    #[test]
    fn push_branch_updates_the_remote_tip() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();
        let initial_commit = local_repo.find_commit(initial_oid).unwrap();
        let new_oid = commit_blob(&local_repo, "local change", &[&initial_commit], "b.txt", "hello\n");

        push_branch(&local_repo, |_| {}, "main", "origin", "main", false, false, false).unwrap();

        let remote_repo = Repository::open(&remote_dir).unwrap();
        let remote_tip = remote_repo.find_reference("refs/heads/main").unwrap().target().unwrap();
        assert_eq!(remote_tip, new_oid, "remote main should now point at the pushed commit");

        cleanup_pair(&remote_dir, &local_dir);
    }

    #[test]
    fn push_branch_with_force_with_lease_rejects_a_stale_remote() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();

        // Someone else pushes to the remote without this clone knowing about it.
        let remote_repo = Repository::open(&remote_dir).unwrap();
        let initial_commit = remote_repo.find_commit(initial_oid).unwrap();
        commit_blob(&remote_repo, "someone else's push", &[&initial_commit], "b.txt", "surprise\n");

        // This clone, unaware, tries to force-with-lease its own (divergent) local commit.
        let local_initial = local_repo.find_commit(initial_oid).unwrap();
        commit_blob(&local_repo, "local change", &[&local_initial], "c.txt", "mine\n");

        let result = push_branch(&local_repo, |_| {}, "main", "origin", "main", false, true, true);
        assert!(result.is_err(), "force-with-lease should refuse a remote that moved since the last known state");

        cleanup_pair(&remote_dir, &local_dir);
    }

    #[test]
    fn pull_ff_only_fast_forwards_when_remote_is_strictly_ahead() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();
        let remote_repo = Repository::open(&remote_dir).unwrap();
        let initial_commit = remote_repo.find_commit(initial_oid).unwrap();
        let remote_tip = commit_blob(&remote_repo, "remote-only", &[&initial_commit], "b.txt", "ff me\n");

        let outcome =
            pull_branch(&local_repo, |_| {}, "main", "origin", "main", PullStrategy::FfOnly).unwrap();
        let ConflictableOutcome::Completed { sha } = outcome else {
            panic!("expected a clean fast-forward, got {outcome:?}");
        };
        assert_eq!(sha, remote_tip.to_string());
        assert_eq!(local_repo.head().unwrap().peel_to_commit().unwrap().id(), remote_tip);

        cleanup_pair(&remote_dir, &local_dir);
    }

    #[test]
    fn pull_merge_strategy_hands_off_a_real_conflict_to_the_resolver() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();
        let remote_repo = Repository::open(&remote_dir).unwrap();
        let initial_commit = remote_repo.find_commit(initial_oid).unwrap();
        commit_blob(&remote_repo, "remote edits a.txt", &[&initial_commit], "a.txt", "one\nREMOTE\nthree\n");

        let local_initial = local_repo.find_commit(initial_oid).unwrap();
        commit_with_file(&local_repo, &local_dir, "local edits a.txt", &[&local_initial], "a.txt", "one\nLOCAL\nthree\n");

        let outcome =
            pull_branch(&local_repo, |_| {}, "main", "origin", "main", PullStrategy::Merge).unwrap();
        assert!(matches!(outcome, ConflictableOutcome::Conflict));
        assert_eq!(local_repo.state(), git2::RepositoryState::Merge);

        let resolved = vec![ResolvedFile { path: "a.txt".to_string(), content: "one\nBOTH\nthree\n".to_string() }];
        let finish = finish_conflict_resolution(&local_repo, resolved).unwrap();
        assert!(matches!(finish, ConflictableOutcome::Completed { .. }));
        assert_eq!(local_repo.state(), git2::RepositoryState::Clean);
        assert_eq!(
            std::fs::read_to_string(local_dir.join("a.txt")).unwrap(),
            "one\nBOTH\nthree\n"
        );

        cleanup_pair(&remote_dir, &local_dir);
    }

    #[test]
    fn pull_rebase_strategy_pauses_and_resumes_across_two_separate_conflicts() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();
        // A second file, present from the common ancestor onward, so each local commit below can
        // conflict on its *own* file independently of the other — avoiding any ambiguity from a
        // single shared file where one step's resolution could incidentally satisfy the next.
        let remote_repo = Repository::open(&remote_dir).unwrap();
        let initial_commit = remote_repo.find_commit(initial_oid).unwrap();
        let common_oid = commit_blob(&remote_repo, "add b.txt", &[&initial_commit], "b.txt", "orig-b\n");
        let common_commit = remote_repo.find_commit(common_oid).unwrap();
        // Remote then edits both files, so each of the two local commits below (one per file)
        // conflicts with a *different* rebase step — exercising the multi-step continuation loop
        // (`drive_rebase_to_completion`), not just a single paused step.
        commit_blob(&remote_repo, "remote edits both files", &[&common_commit], "a.txt", "remote-a\n");
        let remote_tip = remote_repo.find_reference("refs/heads/main").unwrap().peel_to_commit().unwrap();
        let blob_oid = remote_repo.blob(b"remote-b\n").unwrap();
        let mut builder = remote_repo.treebuilder(Some(&remote_tip.tree().unwrap())).unwrap();
        builder.insert("b.txt", blob_oid, 0o100644).unwrap();
        let tree = remote_repo.find_tree(builder.write().unwrap()).unwrap();
        let sig = remote_repo.signature().unwrap();
        remote_repo
            .commit(Some("refs/heads/main"), &sig, &sig, "remote edits b.txt too", &tree, &[&remote_tip])
            .unwrap();

        fetch_remote(&local_repo, |_| {}, Some("origin"), false, false, false).unwrap();
        let common_local_commit = local_repo.find_commit(common_oid).unwrap();
        // Sync HEAD/workdir/index to the common ancestor (which already has b.txt) before
        // building local commits on top via `commit_with_file` — otherwise the still-checked-out
        // initial-commit tree (no b.txt) would make the first local commit look like it deletes
        // b.txt instead of cleanly adding only a.txt.
        local_repo
            .reset(common_local_commit.as_object(), git2::ResetType::Hard, None)
            .unwrap();
        let local_first =
            commit_with_file(&local_repo, &local_dir, "local edits a.txt", &[&common_local_commit], "a.txt", "local-a\n");
        {
            let local_first_commit = local_repo.find_commit(local_first).unwrap();
            commit_with_file(&local_repo, &local_dir, "local edits b.txt", &[&local_first_commit], "b.txt", "local-b\n");
        }

        let outcome =
            pull_branch(&local_repo, |_| {}, "main", "origin", "main", PullStrategy::Rebase).unwrap();
        assert!(matches!(outcome, ConflictableOutcome::Conflict), "first rebased commit should conflict on a.txt");
        assert!(matches!(
            local_repo.state(),
            git2::RepositoryState::Rebase | git2::RepositoryState::RebaseInteractive | git2::RepositoryState::RebaseMerge
        ));

        let resolved_step_1 = vec![ResolvedFile { path: "a.txt".to_string(), content: "both-a\n".to_string() }];
        let after_step_1 = finish_conflict_resolution(&local_repo, resolved_step_1).unwrap();
        assert!(
            matches!(after_step_1, ConflictableOutcome::Conflict),
            "second rebased commit should independently conflict on b.txt, re-pausing instead of finishing"
        );

        let resolved_step_2 = vec![ResolvedFile { path: "b.txt".to_string(), content: "both-b\n".to_string() }];
        let after_step_2 = finish_conflict_resolution(&local_repo, resolved_step_2).unwrap();
        assert!(matches!(after_step_2, ConflictableOutcome::Completed { .. }), "rebase should finish after the second conflict resolves");
        assert_eq!(local_repo.state(), git2::RepositoryState::Clean);
        assert_eq!(std::fs::read_to_string(local_dir.join("a.txt")).unwrap(), "both-a\n");
        assert_eq!(std::fs::read_to_string(local_dir.join("b.txt")).unwrap(), "both-b\n");

        cleanup_pair(&remote_dir, &local_dir);
    }

    // --- Branch CRUD (SPEC.md item 8, PRD §13) ----------------------------------------------

    #[test]
    fn checkout_branch_switches_head_on_a_clean_tree() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        repo.branch("feature", &c1_commit, false).unwrap();

        checkout_branch(&repo, "feature", None, None).unwrap();
        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feature");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    // Regression test for a real bug: `checkout_head(None)` (libgit2's non-forced
    // `GIT_CHECKOUT_SAFE` default) can leave a working-directory file's on-disk content stale
    // — not matching the tree it just checked out into the index — even on a verified-clean
    // tree with nothing at risk. That showed up as the "Stage changes" toolbar button reporting
    // phantom changes immediately after switching branches in the UI, even though a real `git
    // switch` of the same move showed nothing. This test creates a branch at an *older* commit
    // (different file content from HEAD) and switches to it, then asserts both the on-disk file
    // content and `working_tree_status` end up clean — not just that HEAD moved.
    #[test]
    fn checkout_branch_rewrites_workdir_content_when_switching_to_an_older_commit() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "first", &[], "a.txt", "older content\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        repo.branch("feature", &c1_commit, false).unwrap();
        commit_with_file(&repo, &dir, "second", &[&c1_commit], "a.txt", "newer content\n");
        // HEAD (main) is now at "second"; "feature" still points at "first" with different
        // content for the same file — exactly the shape of "branch from here at an earlier
        // commit, then switch to it" that exposed the bug.

        checkout_branch(&repo, "feature", None, None).unwrap();

        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feature");
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "older content\n",
            "working directory should actually contain the checked-out branch's content"
        );
        let status = working_tree_status(&repo).unwrap();
        assert!(status.files.is_empty(), "tree should be clean immediately after switching, found: {:?}", status.files);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    // `create_branch_at` is ref-creation only — never touches HEAD or the working tree.
    // "Checkout after creating" is the frontend separately calling `checkout_branch` once this
    // succeeds (see that function's own dirty-tree-handling tests above for the checkout half of
    // what used to be exercised here), so a failed checkout can never be mistaken for a failed
    // creation — the branch this test creates must exist regardless of what happens next.
    #[test]
    fn create_branch_at_only_creates_the_ref_without_touching_head_or_workdir() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "first", &[], "a.txt", "older content\n");
        commit_with_file(&repo, &dir, "second", &[&repo.find_commit(c1).unwrap()], "a.txt", "newer content\n");
        let head_before = repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch_at(&repo, &c1.to_string(), "feature").unwrap();

        assert_eq!(repo.head().unwrap().shorthand().unwrap(), head_before, "create must not move HEAD");
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "newer content\n",
            "create must not touch the working directory"
        );
        let feature_tip = repo.find_branch("feature", BranchType::Local).unwrap().get().target().unwrap();
        assert_eq!(feature_tip.to_string(), c1.to_string(), "new branch should point at the requested starting commit");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn checkout_branch_requires_a_dirty_tree_strategy() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        repo.branch("feature", &c1_commit, false).unwrap();
        std::fs::write(dir.join("untracked.txt"), "dirty\n").unwrap();

        let result = checkout_branch(&repo, "feature", None, None);
        assert!(result.is_err(), "a dirty tree with no chosen strategy should refuse rather than guess");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn checkout_branch_stashes_and_reapplies_dirty_changes() {
        let (dir, repo) = temp_repo();
        let c1 = commit_with_file(&repo, &dir, "first", &[], "a.txt", "one\n");
        let c1_commit = repo.find_commit(c1).unwrap();
        repo.branch("feature", &c1_commit, false).unwrap();
        std::fs::write(dir.join("untracked.txt"), "dirty\n").unwrap();

        checkout_branch(&repo, "feature", None, Some(DirtyTreeStrategy::Stash)).unwrap();

        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feature");
        assert_eq!(
            std::fs::read_to_string(dir.join("untracked.txt")).unwrap(),
            "dirty\n",
            "stashed change should be reapplied after the switch"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    // Regression: "carry over" used to rely on libgit2's non-forced `GIT_CHECKOUT_SAFE` strategy
    // to decide what was safe to touch on a dirty tree — but that left every file the target
    // branch changes (not just the user's actual edit) as a phantom uncommitted diff, because
    // SAFE's stat-based detection updated the index without always rewriting the workdir to
    // match. With an explicit tree-diff-based path restriction, only the user's real edit should
    // remain pending after carrying over, and `a.txt` (which the user never touched) must come
    // out cleanly matching the target branch.
    #[test]
    fn checkout_branch_carry_over_only_leaves_the_users_real_edit_pending() {
        let (dir, repo) = temp_repo();
        let base = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\n");
        let base_commit = repo.find_commit(base).unwrap();
        repo.branch("target", &base_commit, false).unwrap();
        // Advance "target" without moving HEAD (still on main/base) — a.txt differs between the
        // two branches, exactly like the bug report's "diff between the previous and new branch".
        let sig = repo.signature().unwrap();
        let tree_oid = {
            let blob = repo.blob(b"two\n").unwrap();
            let mut builder = repo.treebuilder(Some(&base_commit.tree().unwrap())).unwrap();
            builder.insert("a.txt", blob, 0o100644).unwrap();
            builder.write().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("refs/heads/target"), &sig, &sig, "advance target", &tree, &[&base_commit]).unwrap();

        // The user's one real uncommitted change: a staged new file, unrelated to a.txt.
        std::fs::write(dir.join("new.txt"), "mine\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("new.txt")).unwrap();
        index.write().unwrap();

        checkout_branch(&repo, "target", None, Some(DirtyTreeStrategy::Carry)).unwrap();

        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "target");
        assert_eq!(
            std::fs::read_to_string(dir.join("a.txt")).unwrap(),
            "two\n",
            "a.txt should be cleanly checked out to the target branch's content, not left stale"
        );
        let status = working_tree_status(&repo).unwrap();
        assert_eq!(
            status.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            vec!["new.txt"],
            "only the user's real edit should remain pending, found: {:?}",
            status.files
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn checkout_branch_carry_over_refuses_a_real_conflict() {
        let (dir, repo) = temp_repo();
        let base = commit_with_file(&repo, &dir, "base", &[], "a.txt", "one\n");
        let base_commit = repo.find_commit(base).unwrap();
        let original_branch = repo.head().unwrap().shorthand().unwrap().to_string();
        repo.branch("target", &base_commit, false).unwrap();
        let sig = repo.signature().unwrap();
        let tree_oid = {
            let blob = repo.blob(b"two\n").unwrap();
            let mut builder = repo.treebuilder(Some(&base_commit.tree().unwrap())).unwrap();
            builder.insert("a.txt", blob, 0o100644).unwrap();
            builder.write().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("refs/heads/target"), &sig, &sig, "advance target", &tree, &[&base_commit]).unwrap();

        // This time the user's uncommitted edit is to the *same* path the target branch changes.
        std::fs::write(dir.join("a.txt"), "my edit\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();

        let result = checkout_branch(&repo, "target", None, Some(DirtyTreeStrategy::Carry));
        assert!(result.is_err(), "carrying over a real path collision should refuse, not silently overwrite");
        assert_eq!(repo.head().unwrap().shorthand().unwrap(), original_branch, "HEAD must not move on refusal");
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "my edit\n", "the user's edit must be untouched");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn get_branch_delete_info_reports_merged_branch_as_safe() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        repo.branch("feature", &c1_commit, false).unwrap();
        // HEAD (main) and "feature" point at the same commit — feature is trivially merged.

        let info = get_branch_delete_info(&repo, "feature").unwrap();
        assert!(info.merged);
        assert_eq!(info.commit_loss_count, 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn get_branch_delete_info_counts_unmerged_commits() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        let feature_branch = repo.branch("feature", &c1_commit, false).unwrap();
        let feature_tip = feature_branch.get().target().unwrap();
        let feature_commit = repo.find_commit(feature_tip).unwrap();
        // `commit()` writes through `HEAD`, so this needs `Some("refs/heads/feature")` explicitly
        // — HEAD (main) must stay put for "feature" to end up with unique work main doesn't have.
        let sig = repo.signature().unwrap();
        let tree_oid = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("refs/heads/feature"), &sig, &sig, "feature work", &tree, &[&feature_commit])
            .unwrap();

        let info = get_branch_delete_info(&repo, "feature").unwrap();
        assert!(!info.merged);
        assert_eq!(info.commit_loss_count, 1);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rename_branch_updates_the_ref_name_only() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        repo.branch("old-name", &c1_commit, false).unwrap();

        rename_branch(&repo, "old-name", "new-name").unwrap();

        assert!(repo.find_branch("old-name", BranchType::Local).is_err());
        assert!(repo.find_branch("new-name", BranchType::Local).is_ok());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn delete_branch_refuses_unmerged_without_force() {
        let (dir, repo) = temp_repo();
        let c1 = commit(&repo, "first", &[]);
        let c1_commit = repo.find_commit(c1).unwrap();
        let feature_branch = repo.branch("feature", &c1_commit, false).unwrap();
        let feature_tip = feature_branch.get().target().unwrap();
        let feature_commit = repo.find_commit(feature_tip).unwrap();
        // `commit()` writes through HEAD (main) — explicitly target "refs/heads/feature" instead
        // so main stays put and "feature" ends up with unique unmerged work.
        let sig = repo.signature().unwrap();
        let tree_oid = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("refs/heads/feature"), &sig, &sig, "feature work", &tree, &[&feature_commit])
            .unwrap();

        let result = delete_branch(&repo, "feature", false, false);
        assert!(result.is_err(), "deleting an unmerged branch without force should be refused");
        assert!(repo.find_branch("feature", BranchType::Local).is_ok());

        delete_branch(&repo, "feature", true, false).unwrap();
        assert!(repo.find_branch("feature", BranchType::Local).is_err());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn delete_branch_also_deletes_the_remote_branch() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();
        let initial_commit = local_repo.find_commit(initial_oid).unwrap();
        local_repo.branch("feature", &initial_commit, false).unwrap();
        push_branch(&local_repo, |_| {}, "feature", "origin", "feature", true, false, false).unwrap();

        delete_branch(&local_repo, "feature", true, true).unwrap();

        let remote_repo = Repository::open(&remote_dir).unwrap();
        assert!(
            remote_repo.find_reference("refs/heads/feature").is_err(),
            "remote feature branch should have been deleted too"
        );

        cleanup_pair(&remote_dir, &local_dir);
    }

    #[test]
    fn list_branches_for_switch_lists_remote_only_branches_below_locals() {
        let (remote_dir, local_dir, local_repo, initial_oid) = remote_and_clone_pair();
        let remote_repo = Repository::open(&remote_dir).unwrap();
        let initial_commit = remote_repo.find_commit(initial_oid).unwrap();
        commit_blob(&remote_repo, "remote-only branch work", &[&initial_commit], "b.txt", "remote\n");
        remote_repo.reference("refs/heads/other", initial_oid, false, "").unwrap();
        fetch_remote(&local_repo, |_| {}, Some("origin"), false, false, false).unwrap();

        let entries = list_branches_for_switch(&local_repo).unwrap();
        let main = entries.iter().find(|e| e.name == "main").unwrap();
        assert!(!main.is_remote_only);
        let other = entries.iter().find(|e| e.name == "other").unwrap();
        assert!(other.is_remote_only, "branch with no local counterpart should be listed as remote-only");
        assert_eq!(other.remote_label.as_deref(), Some("origin/other"));

        cleanup_pair(&remote_dir, &local_dir);
    }
}
