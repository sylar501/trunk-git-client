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
}
