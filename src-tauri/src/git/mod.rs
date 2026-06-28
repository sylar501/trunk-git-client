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

    pub fn cherry_pick(&self, sha: &str) -> Result<ConflictableOutcome, String> {
        cherry_pick(&self.inner, sha)
    }

    pub fn revert_commit(&self, sha: &str) -> Result<ConflictableOutcome, String> {
        revert_commit(&self.inner, sha)
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

    pub fn finish_conflict_resolution(&self, files: Vec<ResolvedFile>) -> Result<String, String> {
        finish_conflict_resolution(&self.inner, files)
    }

    pub fn abort_conflict_resolution(&self) -> Result<(), String> {
        abort_in_progress_operation(&self.inner).map_err(|e| e.to_string())
    }

    pub fn create_branch_at(&self, sha: &str, name: &str) -> Result<(), String> {
        create_branch_at(&self.inner, sha, name)
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
            out.push(BranchInfo {
                name: name.to_string(),
                is_head: branch.is_head(),
                color_index: branch_color_index(name),
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
    Conflict,
}

/// Cherry-picks a single commit onto HEAD. Refuses merge commits outright (matching plain `git
/// cherry-pick`'s own default refusal without `-m` — libgit2 hard-errors rather than defaulting
/// to a mainline if one isn't specified) and refuses a dirty working tree (see
/// `is_working_tree_clean`) before calling into libgit2 at all.
///
/// On conflicts, leaves the index/working tree exactly as `repo.cherrypick()` produced them
/// (same as plain `git cherry-pick` would) instead of aborting — `conflict_style_diff3` on the
/// checkout builder (not the merge options; libgit2's checkout step recomputes each conflicting
/// file's on-disk content from scratch and only reads this style flag, ignoring whatever the
/// merge step itself used) makes libgit2 write `<<<<<<<`/`|||||||`/`=======`/`>>>>>>>` markers
/// straight into the conflicting files, which the conflict resolver reads back via
/// `get_conflict_file`.
fn cherry_pick(repo: &Repository, sha: &str) -> Result<ConflictableOutcome, String> {
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
/// upfront refusals, and the same conflict-handoff shape, as `cherry_pick` above.
fn revert_commit(repo: &Repository, sha: &str) -> Result<ConflictableOutcome, String> {
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
fn finish_conflict_resolution(repo: &Repository, files: Vec<ResolvedFile>) -> Result<String, String> {
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
    Ok(new_oid.to_string())
}

/// Creates a branch at `sha` and checks it out — deliberately a minimal placeholder (no
/// real-time name validation, no starting-point picker) ahead of SPEC.md item 8's full "Create
/// Branch" dialog. `repo.branch(..., force: false)` surfaces git2's natural duplicate-name error
/// rather than silently overwriting; `checkout_head(None)` uses libgit2's default *non-forced*
/// safety checks, so a conflicting dirty working tree fails loudly instead of being clobbered.
fn create_branch_at(repo: &Repository, sha: &str, name: &str) -> Result<(), String> {
    let oid = Oid::from_str(sha).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    let branch = repo.branch(name, &commit, false).map_err(|e| e.to_string())?;
    let refname = branch
        .get()
        .name()
        .ok_or_else(|| "Created branch has an invalid reference name.".to_string())?
        .to_string();
    repo.set_head(&refname).map_err(|e| e.to_string())?;
    repo.checkout_head(None).map_err(|e| e.to_string())?;
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

        assert!(cherry_pick(&repo, &merge_sha).is_err(), "cherry-pick must refuse a merge commit");
        assert!(revert_commit(&repo, &merge_sha).is_err(), "revert must refuse a merge commit");

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
    fn cherry_pick_conflict_is_reported_not_aborted() {
        let (dir, repo, _main_tip, feature_oid) = setup_conflicting_branches();

        let outcome = cherry_pick(&repo, &feature_oid.to_string()).unwrap();
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
        cherry_pick(&repo, &feature_oid.to_string()).unwrap();

        let resolved = vec![ResolvedFile { path: "a.txt".to_string(), content: "one\nFEATURE\nthree\n".to_string() }];
        let new_sha = finish_conflict_resolution(&repo, resolved).unwrap();

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
        cherry_pick(&repo, &feature_oid.to_string()).unwrap();

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
}
