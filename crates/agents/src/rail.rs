//! The Rail: Workstreams, Agents and Terminals as a tree in SQLite (WAL).
//! Sibling `order` values are contiguous from 0. Workstreams sit at the root and never nest (A26);
//! an Agent sits under a Workstream or under the Agent that started it, to any depth.

use std::path::Path;

use contracts::agent::{NodeKind, Order, RailNode, Worktree};
use contracts::project::Worktrees;
use rpc::{OpenError, RpcError};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

/// Structure only: a node's `status` is the Agents module's to fill in.
pub struct Rail {
    db: Connection,
}

impl Rail {
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let mut db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                parent INTEGER REFERENCES nodes(id),
                ord INTEGER NOT NULL,
                meta INTEGER NOT NULL DEFAULT 0,
                terminal_id TEXT
            );
            CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK (id = 0),
                worktrees_on INTEGER NOT NULL DEFAULT 0,
                worktrees_check TEXT
            );",
        )?;
        // SQLite's `ALTER TABLE` has no `ADD COLUMN IF NOT EXISTS`, so a column a Project's
        // `agents.db` already has (from before G2) is skipped by hand.
        for (column, decl) in [
            ("attempt", "INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0)"),
            ("worktree_owner", "TEXT"),
            ("worktree_commit", "TEXT"),
            ("worktree_path", "TEXT"),
            ("worktree_branch", "TEXT"),
            ("worktree_base", "TEXT"),
            // 'provisioning' from before `git worktree add` runs until the Terminal is about to
            // start, then 'ready'; no RPC returns it (G2, G6).
            ("worktree_state", "TEXT"),
            // A21: the vendor conversation id and the effective cwd it was launched in, from the
            // latest Signal of the current Attempt that named one. Never read outside this module
            // and `agent.resume`; no vendor id reaches a public name or the webview.
            ("conversation_id", "TEXT"),
            ("conversation_cwd", "TEXT"),
            // O1: the node's order as JSON; NULL (a node from before O1) reads as the default
            // Clarification order of its kind.
            ("work", "TEXT"),
        ] {
            add_column_if_missing(&db, "nodes", column, decl)?;
        }
        // Terminal ids restart at 1 with the Daemon, so a stored id could name an unrelated Terminal.
        db.execute("UPDATE nodes SET terminal_id = NULL", [])?;
        // A25: a Rail stored by an earlier Daemon holds `room` where this one writes `workstream`.
        db.execute(
            "UPDATE nodes SET kind = 'workstream' WHERE kind = 'room'",
            [],
        )?;
        flatten_workstreams(&mut db)?;
        Ok(Self { db })
    }

    /// Every node, each parent before its children, siblings by `order`.
    pub fn tree(&self) -> Result<Vec<RailNode>, RpcError> {
        let mut flat = load(&self.db)?;
        flat.sort_by_key(|node| node.order);
        let mut out = Vec::with_capacity(flat.len());
        descend(&flat, None, &mut out);
        Ok(out)
    }

    /// Append a node under `parent` (`None` is the root); `check_parent` says which kinds fit where.
    pub fn insert(
        &mut self,
        kind: NodeKind,
        name: &str,
        parent: Option<&str>,
        terminal_id: Option<&str>,
    ) -> Result<RailNode, RpcError> {
        self.insert_ordered(kind, name, parent, terminal_id, None)
    }

    /// `insert` with the node's order (O2, O3); `None` is the default of its kind. A Terminal
    /// holds none.
    pub fn insert_ordered(
        &mut self,
        kind: NodeKind,
        name: &str,
        parent: Option<&str>,
        terminal_id: Option<&str>,
        work: Option<&Order>,
    ) -> Result<RailNode, RpcError> {
        let work = (kind != NodeKind::Terminal)
            .then(|| work.map_or_else(|| default_order(kind), Order::clone))
            .map(|order| serde_json::to_string(&order).map_err(RpcError::internal))
            .transpose()?;
        let tx = self.db.transaction().map_err(sql)?;
        let nodes = load(&tx)?;
        check_parent(&nodes, kind, parent)?;
        let order = siblings(&nodes, parent).len() as i64;
        tx.execute(
            "INSERT INTO nodes (kind, name, parent, ord, terminal_id, work)
                VALUES (?, ?, ?, ?, ?, ?)",
            params![kind_name(kind), name, parent, order, terminal_id, work],
        )
        .map_err(sql)?;
        let id = tx.last_insert_rowid().to_string();
        tx.commit().map_err(sql)?;
        self.node(&id)
    }

    /// O4: replace the order of an Agent or a Workstream. A Terminal holds none (`CONFLICT`).
    pub fn set_order(&mut self, id: &str, order: &Order) -> Result<RailNode, RpcError> {
        let node = self.node(id)?;
        if node.kind == NodeKind::Terminal {
            return Err(RpcError::conflict(format!("{id} is a Terminal")));
        }
        let json = serde_json::to_string(order).map_err(RpcError::internal)?;
        self.db
            .execute(
                "UPDATE nodes SET work = ? WHERE id = ?",
                params![json, node.id],
            )
            .map_err(sql)?;
        self.node(id)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<RailNode, RpcError> {
        // SQLite would match "01" to node 1; only the exact id names a node.
        let node = self.node(id)?;
        self.db
            .execute(
                "UPDATE nodes SET name = ? WHERE id = ?",
                params![name, node.id],
            )
            .map_err(sql)?;
        self.node(id)
    }

    /// Put `id` at `index` among the children of `parent`, closing the gap it leaves behind.
    pub fn move_node(
        &mut self,
        id: &str,
        parent: Option<&str>,
        index: u32,
    ) -> Result<(), RpcError> {
        let tx = self.db.transaction().map_err(sql)?;
        let nodes = load(&tx)?;
        let node = find(&nodes, id)?;
        check_parent(&nodes, node.kind, parent)?;
        // Only a start nests an Agent under an Agent (A6, A26).
        if let Some(under) = parent.map(|id| find(&nodes, id)).transpose()?
            && node.kind == NodeKind::Agent
            && under.kind == NodeKind::Agent
        {
            return Err(RpcError::conflict(format!(
                "{id} cannot move under Agent {}: only a start nests it",
                under.id
            )));
        }
        if parent.is_some_and(|parent| is_within(&nodes, parent, id)) {
            return Err(RpcError::conflict(format!("{id} cannot move into itself")));
        }
        let without = |parent: Option<&str>| {
            let mut ids = siblings(&nodes, parent);
            ids.retain(|sibling| sibling != id);
            ids
        };
        let mut ids = without(parent);
        ids.insert((index as usize).min(ids.len()), id.to_owned());
        place(&tx, parent, &ids)?;
        if node.parent.as_deref() != parent {
            place(
                &tx,
                node.parent.as_deref(),
                &without(node.parent.as_deref()),
            )?;
        }
        tx.commit().map_err(sql)
    }

    pub fn allocate_attempt(&mut self, id: &str) -> Result<String, RpcError> {
        let node = self.node(id)?;
        let previous = node
            .attempt
            .as_deref()
            .unwrap_or("0")
            .parse::<i64>()
            .map_err(sql_ordinal)?;
        let next = previous
            .checked_add(1)
            .ok_or_else(|| RpcError::internal("Attempt exhausted"))?;
        self.db
            .execute(
                "UPDATE nodes SET attempt = ? WHERE id = ?",
                params![next, id],
            )
            .map_err(sql)?;
        Ok(next.to_string())
    }

    pub fn detach_terminal(&mut self, id: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE nodes SET terminal_id = NULL WHERE id = ?",
                params![id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// Record which Terminal runs the Agent `id`.
    pub fn attach_terminal(&mut self, id: &str, terminal_id: &str) -> Result<(), RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE nodes SET terminal_id = ? WHERE id = ?",
                params![terminal_id, id],
            )
            .map_err(sql)?;
        if changed == 0 {
            return Err(RpcError::not_found(format!("node {id}")));
        }
        Ok(())
    }

    /// Delete `id`. Its children (only a Workstream, so a Door, ever has any) move to its own
    /// parent, at its place, in order, first; one transaction, so a failed delete leaves them
    /// still under `id`.
    pub fn remove(&mut self, id: &str) -> Result<(), RpcError> {
        let tx = self.db.transaction().map_err(sql)?;
        let nodes = load(&tx)?;
        let node = find(&nodes, id)?;
        let parent = node.parent.as_deref();
        let mut ids = siblings(&nodes, parent);
        let at = ids
            .iter()
            .position(|sibling| sibling == id)
            .expect("id is among its own parent's siblings");
        let children = siblings(&nodes, Some(id));
        ids.splice(at..=at, children);
        // Reparent before deleting: a child's row still points at `id` until `place` runs, and a
        // delete while one does would fail its foreign key.
        place(&tx, parent, &ids)?;
        tx.execute("DELETE FROM nodes WHERE id = ?", params![id])
            .map_err(sql)?;
        tx.commit().map_err(sql)
    }

    pub fn node(&self, id: &str) -> Result<RailNode, RpcError> {
        find(&load(&self.db)?, id).cloned()
    }

    /// The Project's `worktrees` setting; `{on: false, check: null}` for a Project that never
    /// had it set (G1).
    pub fn get_worktrees(&self) -> Result<Worktrees, RpcError> {
        self.db
            .query_row(
                "SELECT worktrees_on, worktrees_check FROM settings WHERE id = 0",
                [],
                |row| {
                    Ok(Worktrees {
                        on: row.get(0)?,
                        check: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(sql)
            .map(|found| {
                found.unwrap_or(Worktrees {
                    on: false,
                    check: None,
                })
            })
    }

    pub fn set_worktrees(&mut self, worktrees: &Worktrees) -> Result<(), RpcError> {
        self.db
            .execute(
                "INSERT INTO settings (id, worktrees_on, worktrees_check) VALUES (0, ?, ?)
                 ON CONFLICT (id) DO UPDATE SET worktrees_on = excluded.worktrees_on,
                    worktrees_check = excluded.worktrees_check",
                params![worktrees.on, worktrees.check],
            )
            .map_err(sql)?;
        Ok(())
    }

    pub fn mark_worktree_provisioning(
        &mut self,
        id: &str,
        plan: &super::worktree::Provision,
    ) -> Result<(), RpcError> {
        self.db.execute("UPDATE nodes SET worktree_state='provisioning', worktree_path=?, worktree_branch=?, worktree_base=?, worktree_owner=?, worktree_commit=? WHERE id=?", params![plan.worktree.path.to_string_lossy(), plan.worktree.branch, plan.worktree.base, plan.owner, plan.commit, id]).map_err(sql)?;
        Ok(())
    }

    /// Each node with a Worktree record still owned by provisioning, with its plan and whether
    /// it reached `ready`; the plan is `None` for a legacy record with no ownership proof (G6),
    /// which no caller may clean up.
    pub fn provisioning(
        &self,
    ) -> Result<Vec<(RailNode, Option<super::worktree::Provision>, bool)>, RpcError> {
        let mut records = Vec::new();
        for node in self.tree()? {
            let (state, owner, commit): (Option<String>, Option<String>, Option<String>) = self
                .db
                .query_row(
                    "SELECT worktree_state, worktree_owner, worktree_commit FROM nodes WHERE id=?",
                    [&node.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(sql)?;
            if state.as_deref() == Some("provisioning") && owner.is_none() {
                records.push((node.clone(), None, false));
                continue;
            }
            if let (Some(owner), Some(commit), Some(worktree)) = (owner, commit, &node.worktree) {
                records.push((
                    node.clone(),
                    Some(super::worktree::Provision {
                        worktree: super::worktree::Worktree::from(worktree),
                        owner,
                        commit,
                    }),
                    state.as_deref() == Some("ready"),
                ));
            }
        }
        Ok(records)
    }

    pub fn clear_provisioning_owner(&mut self, id: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE nodes SET worktree_owner=NULL, worktree_commit=NULL WHERE id=?",
                [id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// Record a Worktree Provisioning made for `id`, just before its Terminal starts.
    pub fn set_worktree(&mut self, id: &str, worktree: &Worktree) -> Result<(), RpcError> {
        let path = worktree.path.as_str();
        self.db
            .execute(
                "UPDATE nodes SET worktree_path = ?, worktree_branch = ?, worktree_base = ?,
                    worktree_state = 'ready' WHERE id = ?",
                params![path, worktree.branch, worktree.base, id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// Undo `mark_worktree_provisioning`/`set_worktree` for a node that keeps its place on the
    /// Rail (`rail.startDoor`'s own failure path; a failed `agent.spawn` deletes the node outright).
    pub fn clear_worktree(&mut self, id: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE nodes SET worktree_path = NULL, worktree_branch = NULL,
                    worktree_base = NULL, worktree_state = NULL, worktree_owner=NULL, worktree_commit=NULL WHERE id = ?",
                params![id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// A21: save the vendor conversation id and the effective cwd it was launched in, replacing
    /// any earlier save. `id` must already name a node.
    pub fn save_conversation(
        &mut self,
        id: &str,
        conversation_id: &str,
        cwd: &str,
    ) -> Result<(), RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE nodes SET conversation_id = ?, conversation_cwd = ? WHERE id = ?",
                params![conversation_id, cwd, id],
            )
            .map_err(sql)?;
        if changed == 0 {
            return Err(RpcError::not_found(format!("node {id}")));
        }
        Ok(())
    }

    /// The saved conversation id and cwd (A21), if any.
    pub fn conversation(&self, id: &str) -> Result<Option<(String, String)>, RpcError> {
        let found: (Option<String>, Option<String>) = self
            .db
            .query_row(
                "SELECT conversation_id, conversation_cwd FROM nodes WHERE id = ?",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sql)?
            .ok_or_else(|| RpcError::not_found(format!("node {id}")))?;
        Ok(match found {
            (Some(conversation_id), Some(cwd)) => Some((conversation_id, cwd)),
            _ => None,
        })
    }

    /// Whether `id` has A21 data; used to compute `can_resume` without exposing the id itself.
    pub fn has_conversation(&self, id: &str) -> Result<bool, RpcError> {
        Ok(self.conversation(id)?.is_some())
    }
}

fn sql_ordinal(err: std::num::ParseIntError) -> RpcError {
    RpcError::internal(err)
}

fn sql(err: rusqlite::Error) -> RpcError {
    RpcError::internal(err)
}

/// Add `column` to `table` unless it is already there (a migration run against a Project's
/// existing `agents.db`).
fn add_column_if_missing(
    db: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> rusqlite::Result<()> {
    let has_column = db
        .prepare("SELECT 1 FROM pragma_table_info(?) WHERE name = ?")?
        .query_row(params![table, column], |_| Ok(()))
        .optional()?
        .is_some();
    if !has_column {
        db.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
    }
    Ok(())
}

/// O2, O3: the order a node holds when none was given.
pub fn default_order(kind: NodeKind) -> Order {
    Order::clarification(match kind {
        NodeKind::Workstream => "What is this Workstream for?",
        _ => "What should this Agent do?",
    })
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Workstream => "workstream",
        NodeKind::Agent => "agent",
        NodeKind::Terminal => "terminal",
    }
}

fn load(db: &Connection) -> Result<Vec<RailNode>, RpcError> {
    let mut query = db
        .prepare(
            "SELECT id, kind, name, parent, ord, attempt, terminal_id,
                worktree_path, worktree_branch, worktree_base, work FROM nodes",
        )
        .map_err(sql)?;
    let rows = query
        .query_map([], |row| {
            let kind = match row.get::<_, String>(1)?.as_str() {
                "group" | "workstream" => NodeKind::Workstream,
                "agent" => NodeKind::Agent,
                _ => NodeKind::Terminal,
            };
            let worktree = match (
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ) {
                (Some(path), Some(branch), Some(base)) => Some(Worktree { path, branch, base }),
                _ => None,
            };
            let work = match kind {
                NodeKind::Terminal => None,
                _ => Some(
                    row.get::<_, Option<String>>(10)?
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_else(|| default_order(kind)),
                ),
            };
            Ok(RailNode {
                id: row.get::<_, i64>(0)?.to_string(),
                kind,
                name: row.get(2)?,
                parent: row.get::<_, Option<i64>>(3)?.map(|id| id.to_string()),
                order: row.get(4)?,
                status: None,
                status_revision: None,
                attempt: match row.get::<_, i64>(5)? {
                    0 => None,
                    n => Some(n.to_string()),
                },
                terminal_id: row.get(6)?,
                worktree,
                can_resume: false,
                channel: None,
                stray: Vec::new(),
                work,
            })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

fn find<'a>(nodes: &'a [RailNode], id: &str) -> Result<&'a RailNode, RpcError> {
    nodes
        .iter()
        .find(|node| node.id == id)
        .ok_or_else(|| RpcError::not_found(format!("node {id}")))
}

/// Ids of the children of `parent`, by `order`.
fn siblings(nodes: &[RailNode], parent: Option<&str>) -> Vec<String> {
    let mut children: Vec<_> = nodes
        .iter()
        .filter(|node| node.parent.as_deref() == parent)
        .collect();
    children.sort_by_key(|node| node.order);
    children.into_iter().map(|node| node.id.clone()).collect()
}

fn descend(nodes: &[RailNode], parent: Option<&str>, out: &mut Vec<RailNode>) {
    for node in nodes.iter().filter(|node| node.parent.as_deref() == parent) {
        out.push(node.clone());
        descend(nodes, Some(&node.id), out);
    }
}

/// A26: a Workstream sits at the root. An Agent sits under a Workstream or under the Agent that
/// started it; a Terminal sits under a Workstream.
fn check_parent(nodes: &[RailNode], kind: NodeKind, parent: Option<&str>) -> Result<(), RpcError> {
    let Some(parent) = parent.map(|id| find(nodes, id)).transpose()? else {
        return Ok(());
    };
    match (kind, parent.kind) {
        (NodeKind::Workstream, _) => Err(RpcError::conflict(format!(
            "Workstreams never nest: a Workstream cannot sit under {}",
            parent.id
        ))),
        (NodeKind::Agent, NodeKind::Agent) | (_, NodeKind::Workstream) => Ok(()),
        _ => Err(RpcError::conflict(format!(
            "{} is not a Workstream: only Workstreams hold children",
            parent.id
        ))),
    }
}

/// A26: a Rail stored by an earlier Daemon may hold a Workstream under another. Each moves to
/// the root, after the last root node, in the order of depth then `order`; the siblings it leaves
/// stay contiguous from 0. Nothing else changes.
fn flatten_workstreams(db: &mut Connection) -> Result<(), RpcError> {
    let tx = db.transaction().map_err(sql)?;
    let nodes = load(&tx)?;
    let depth = |node: &RailNode| {
        let mut depth = 0;
        let mut at = node.parent.as_deref();
        while let Some(parent) = at.filter(|_| depth <= nodes.len()) {
            depth += 1;
            at = nodes
                .iter()
                .find(|candidate| candidate.id == parent)
                .and_then(|candidate| candidate.parent.as_deref());
        }
        depth
    };
    let mut nested: Vec<&RailNode> = nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Workstream && node.parent.is_some())
        .collect();
    if nested.is_empty() {
        return Ok(());
    }
    nested.sort_by_key(|node| (depth(node), node.order, node.id.parse::<i64>().unwrap_or(0)));
    let mut roots = siblings(&nodes, None);
    let left: Vec<String> = nested
        .iter()
        .filter_map(|node| node.parent.clone())
        .collect();
    roots.extend(nested.iter().map(|node| node.id.clone()));
    place(&tx, None, &roots)?;
    for parent in left {
        let rest: Vec<String> = siblings(&nodes, Some(&parent))
            .into_iter()
            .filter(|id| nested.iter().all(|node| &node.id != id))
            .collect();
        place(&tx, Some(&parent), &rest)?;
    }
    tx.commit().map_err(sql)
}

/// Is `node` the ancestor `id` or a descendant of it?
fn is_within(nodes: &[RailNode], node: &str, id: &str) -> bool {
    let mut at = Some(node);
    while let Some(current) = at {
        if current == id {
            return true;
        }
        at = nodes
            .iter()
            .find(|candidate| candidate.id == current)
            .and_then(|candidate| candidate.parent.as_deref());
    }
    false
}

/// Make `ids` the children of `parent`, in that order, numbered from 0.
fn place(tx: &Transaction, parent: Option<&str>, ids: &[String]) -> Result<(), RpcError> {
    for (order, id) in (0_i64..).zip(ids) {
        tx.execute(
            "UPDATE nodes SET parent = ?, ord = ? WHERE id = ?",
            params![parent, order, id],
        )
        .map_err(sql)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use contracts::agent::NodeKind;
    use rpc::code;

    use super::Rail;

    fn rail() -> (tempfile::TempDir, Rail) {
        let dir = tempfile::tempdir().unwrap();
        let rail = Rail::open(&dir.path().join("agents.db")).unwrap();
        (dir, rail)
    }

    #[test]
    fn a8_legacy_plain_and_live_groups_keep_ids_order_and_children_as_workstreams() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agents.db");
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE nodes (id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, name TEXT NOT NULL, parent INTEGER, ord INTEGER NOT NULL, meta INTEGER NOT NULL DEFAULT 0, terminal_id TEXT); INSERT INTO nodes VALUES (1,'group','plain',NULL,0,0,NULL),(2,'group','coordinator',NULL,1,1,'99'),(3,'agent','child',2,0,0,'100');").unwrap();
        drop(db);
        let mut rail = Rail::open(&path).unwrap();
        let tree = rail.tree().unwrap();
        assert_eq!(
            tree.iter()
                .map(|n| (n.id.as_str(), n.name.as_str(), n.order))
                .collect::<Vec<_>>(),
            [
                ("1", "plain", 0),
                ("2", "coordinator", 1),
                ("3", "child", 0)
            ]
        );
        assert_eq!(tree[0].kind, NodeKind::Workstream);
        assert_eq!(tree[1].kind, NodeKind::Workstream);
        assert_eq!(tree[2].parent.as_deref(), Some("2"));
        assert!(
            tree.iter()
                .all(|n| n.attempt.is_none() && n.terminal_id.is_none())
        );
        assert_eq!(rail.allocate_attempt("2").unwrap(), "1");
        drop(rail);
        let mut rail = Rail::open(&path).unwrap();
        assert_eq!(rail.allocate_attempt("2").unwrap(), "2");
        assert_eq!(rail.node("3").unwrap().parent.as_deref(), Some("2"));
    }

    #[test]
    fn a7_attempt_exhaustion_does_not_wrap_or_change_the_node() {
        let (_dir, mut rail) = rail();
        let node = rail
            .insert(NodeKind::Workstream, "workstream", None, None)
            .unwrap();
        rail.db
            .execute("UPDATE nodes SET attempt = ?", [i64::MAX])
            .unwrap();
        let before = rail.node(&node.id).unwrap();
        assert_eq!(
            rail.allocate_attempt(&node.id).unwrap_err().code,
            code::INTERNAL
        );
        assert_eq!(rail.node(&node.id).unwrap(), before);
    }

    #[test]
    fn a6_nothing_nests_under_an_agent_or_a_terminal() {
        let (_dir, mut rail) = rail();
        let group = rail.insert(NodeKind::Workstream, "g", None, None).unwrap();
        for kind in [NodeKind::Agent, NodeKind::Terminal] {
            let leaf = rail.insert(kind, "leaf", None, Some("1")).unwrap();
            let inserted = rail
                .insert(NodeKind::Workstream, "x", Some(&leaf.id), None)
                .unwrap_err();
            let moved = rail.move_node(&group.id, Some(&leaf.id), 0).unwrap_err();
            assert_eq!(
                (inserted.code, moved.code),
                (code::CONFLICT, code::CONFLICT),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn a6_rename_needs_the_exact_id() {
        let (_dir, mut rail) = rail();
        let group = rail.insert(NodeKind::Workstream, "g", None, None).unwrap();
        let err = rail.rename(&format!("0{}", group.id), "other").unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
        assert_eq!(rail.tree().unwrap()[0].name, "g");
    }

    #[test]
    fn a8_reopening_unlinks_every_terminal_id() {
        let (dir, mut rail) = rail();
        rail.insert(NodeKind::Agent, "claude", None, Some("1"))
            .unwrap();
        rail.insert(NodeKind::Terminal, "shell", None, Some("2"))
            .unwrap();
        drop(rail);

        let reopened = Rail::open(&dir.path().join("agents.db")).unwrap();
        let ids: Vec<_> = reopened
            .tree()
            .unwrap()
            .into_iter()
            .map(|n| n.terminal_id)
            .collect();
        assert_eq!(ids, [None, None]);
    }

    #[test]
    fn a6_removing_a_node_closes_the_gap_among_its_siblings() {
        let (_dir, mut rail) = rail();
        let ids: Vec<_> = ["a", "b", "c"]
            .map(|name| {
                rail.insert(NodeKind::Workstream, name, None, None)
                    .unwrap()
                    .id
            })
            .into();
        rail.remove(&ids[1]).unwrap();
        let tree = rail.tree().unwrap();
        let rest: Vec<_> = tree.iter().map(|n| (n.name.as_str(), n.order)).collect();
        assert_eq!(rest, [("a", 0), ("c", 1)]);
    }

    /// A16: moving a Workstream's children and deleting the Workstream are one transaction. A trigger
    /// that only rejects the DELETE (the UPDATEs `place` runs are untouched) tells this apart
    /// from a mutant that commits the move before deleting in a second transaction: there, the
    /// move would survive even though the delete failed.
    #[test]
    fn a16_a_groups_delete_failing_after_a_successful_move_rolls_both_back() {
        let (_dir, mut rail) = rail();
        let group = rail.insert(NodeKind::Workstream, "g", None, None).unwrap();
        let child = rail
            .insert(NodeKind::Agent, "child", Some(&group.id), None)
            .unwrap();
        rail.db
            .execute_batch(
                "CREATE TRIGGER forbid_delete BEFORE DELETE ON nodes
                 BEGIN SELECT RAISE(ABORT, 'delete forbidden'); END;",
            )
            .unwrap();

        let err = rail.remove(&group.id).unwrap_err();

        assert_eq!(err.code, code::INTERNAL);
        let found = rail.node(&child.id).unwrap();
        assert_eq!(found.parent.as_deref(), Some(group.id.as_str()));
    }
}
