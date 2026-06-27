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

    pub fn cherry_pick(&self, sha: &str) -> Result<String, String> {
        cherry_pick(&self.inner, sha)
    }

    pub fn revert_commit(&self, sha: &str) -> Result<String, String> {
        revert_commit(&self.inner, sha)
    }

    pub fn create_branch_at(&self, sha: &str, name: &str) -> Result<(), String> {
        create_branch_at(&self.inner, sha, name)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// Cherry-picks a single commit onto HEAD. Refuses merge commits outright (matching plain `git
/// cherry-pick`'s own default refusal without `-m` — libgit2 hard-errors rather than defaulting
/// to a mainline if one isn't specified) and refuses a dirty working tree (see
/// `is_working_tree_clean`) before calling into libgit2 at all. Returns the new commit's SHA.
fn cherry_pick(repo: &Repository, sha: &str) -> Result<String, String> {
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

    repo.cherrypick(&commit, None).map_err(|e| e.to_string())?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        abort_in_progress_operation(repo).map_err(|e| e.to_string())?;
        return Err(
            "This commit can't be cherry-picked without conflicts. The operation was cancelled and your working tree is unchanged."
                .to_string(),
        );
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
    Ok(new_oid.to_string())
}

/// Reverts a single commit on top of HEAD with a new commit (author = committer = current user,
/// message matches plain `git revert`'s default format). Same merge-commit and dirty-tree
/// upfront refusals, and the same conflict-abort shape, as `cherry_pick` above.
fn revert_commit(repo: &Repository, sha: &str) -> Result<String, String> {
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

    repo.revert(&commit, None).map_err(|e| e.to_string())?;

    let mut index = repo.index().map_err(|e| e.to_string())?;
    if index.has_conflicts() {
        abort_in_progress_operation(repo).map_err(|e| e.to_string())?;
        return Err(
            "Reverting this commit causes conflicts. The operation was cancelled and your working tree is unchanged."
                .to_string(),
        );
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
}
