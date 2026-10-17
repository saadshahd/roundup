//! The Rail: Workstreams, Agents and Terminals as a tree in SQLite (WAL).
//! Sibling `order` values are contiguous from 0. Workstreams sit at the root and never nest (A26);
//! an Agent sits under a Workstream or under the Agent that started it, to any depth.

use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use contracts::agent::{NodeKind, Order, RailNode, Worktree};
use contracts::project::{Mcp, ProjectModels, RoleModels, Worktrees};
use rpc::{OpenError, RpcError};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

/// Structure only: a node's `status` is the Agents module's to fill in.
pub struct Rail {
    owner_reads: OwnerReads,
    db: Connection,
}

// R16 attribution (#1901, r16/owner-transaction): a fresh WAL reader's lock/unlock
// syscalls dominate the owner step. A schema-only peer lets SQLite share its local
// read lock. It must never supply owner facts: the authoritative query stays fresh.
struct OwnerReads {
    wake: Option<std::sync::mpsc::SyncSender<()>>,
    pending: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl OwnerReads {
    fn open(path: &Path) -> Self {
        let (wake, receive) = std::sync::mpsc::sync_channel(1);
        let pending = Arc::new(AtomicBool::new(false));
        let peer_pending = Arc::clone(&pending);
        let path = path.to_owned();
        let thread = std::thread::Builder::new()
            .name("Terminal owner reads".into())
            .spawn(move || {
                if let Err(error) = read_leases(&path, receive, &peer_pending) {
                    eprintln!("Terminal owner read lease stopped: {error}");
                }
            });
        let thread = match thread {
            Ok(thread) => Some(thread),
            Err(error) => {
                eprintln!("Terminal owner read lease unavailable: {error}");
                None
            }
        };
        Self {
            wake: Some(wake),
            pending,
            thread,
        }
    }

    fn wake(&self) {
        // R16 attribution (#1913): each unnecessary channel submission costs
        // 130–160 ns in the release Daemon. A pending attempt needs no further input.
        // This flag carries no owner facts and never renews the peer's deadline.
        if self.pending.load(Ordering::Relaxed) {
            return;
        }
        if !self.pending.swap(true, Ordering::Relaxed)
            && let Some(wake) = &self.wake
        {
            // A disconnected peer disables only the optimization, not the query.
            let _ = wake.try_send(());
        }
    }
}

fn read_leases(
    path: &Path,
    receive: std::sync::mpsc::Receiver<()>,
    pending: &AtomicBool,
) -> rusqlite::Result<()> {
    if receive.recv().is_err() {
        return Ok(());
    }
    let db = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::default() & !rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    db.busy_timeout(std::time::Duration::ZERO)?;
    let mut checkpoint = db.prepare("PRAGMA wal_checkpoint(PASSIVE)")?;
    let mut begin = db.prepare("BEGIN DEFERRED")?;
    let mut schema = db.prepare("SELECT rootpage FROM sqlite_schema LIMIT 1")?;
    let mut rollback = db.prepare("ROLLBACK")?;
    loop {
        let (busy, frames, copied): (i64, i64, i64) =
            checkpoint.query_row([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        // Do not overlap leases with unfinished backfill: renewing old snapshots
        // indefinitely would starve checkpoints under sustained input and writes.
        let leased = busy == 0 && frames >= 0 && frames == copied;
        if leased {
            begin.execute([])?;
            schema.exists([])?;
        }
        // Input cannot renew this deadline. Release even if no call ever follows;
        // an unsuccessful checkpoint must not start a retry for every keystroke either.
        std::thread::sleep(std::time::Duration::from_millis(5));
        if leased {
            rollback.execute([])?;
        }
        // Rearm only after rollback (or a skipped attempt's sleep), before waiting.
        // The flag synchronizes only wake admission; the channel publishes work.
        pending.store(false, Ordering::Relaxed);
        if receive.recv().is_err() {
            return Ok(());
        }
    }
}

impl Drop for OwnerReads {
    fn drop(&mut self) {
        self.wake.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            eprintln!("Terminal owner read lease thread panicked");
        }
    }
}

impl Rail {
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let mut db = Connection::open(path)?;
        db.set_transaction_behavior(rusqlite::TransactionBehavior::Immediate);
        db.pragma_update(None, "journal_mode", "WAL")?;
        crate::schema::open(&mut db, path)?;
        // Terminal ids restart at 1 with the Daemon, so a stored id could name an unrelated Terminal.
        db.execute("UPDATE nodes SET terminal_id = NULL", [])?;
        // A25: a Rail stored by an earlier Daemon holds `room` where this one writes `workstream`.
        db.execute(
            "UPDATE nodes SET kind = 'workstream' WHERE kind = 'room'",
            [],
        )?;
        drop_terminal_nodes(&db)?;
        flatten_workstreams(&mut db)?;
        Ok(Self {
            owner_reads: OwnerReads::open(path),
            db,
        })
    }

    /// Receiver identity and persisted Attempt, without loading Rail presentation fields.
    pub fn terminal_owner(
        &mut self,
        terminal: &str,
    ) -> Result<Option<(String, Option<String>)>, RpcError> {
        // R16 owner-read attribution: preparation costs 11.0–11.5 µs of 18.7–19.6 µs
        // on the measured 7763 host (r16/owner-read). Reuse compiled SQL, never owner facts:
        // each query still binds and reads under Rail then Attempts, including negative reads.
        self.owner_reads.wake();
        // R16 attribution (#1917, r16/owner-execution): 10 → 8 unowned VM steps.
        // Rust stops at the second row, so SQL needs no limit counter. IS avoids
        // a NULL-parameter branch for bound &str; the explicit predicate keeps
        // the partial covering index and excludes unattached rows.
        self.db
            .with_cached_rows(
                "SELECT id, attempt FROM nodes WHERE terminal_id IS ? AND terminal_id IS NOT NULL",
                [terminal],
                |rows| {
                    let Some(row) = rows.next().map_err(sql)? else {
                        return Ok(None);
                    };
                    let id = row.get::<_, i64>(0).map_err(sql)?.to_string();
                    let attempt = match row.get::<_, i64>(1).map_err(sql)? {
                        0 => None,
                        n => Some(n.to_string()),
                    };
                    if rows.next().map_err(sql)?.is_some() {
                        return Err(RpcError::conflict("Terminal has more than one receiver"));
                    }
                    Ok(Some((id, attempt)))
                },
            )
            .map_err(sql)?
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
    #[cfg(test)]
    pub fn insert(
        &mut self,
        kind: NodeKind,
        name: &str,
        parent: Option<&str>,
        terminal_id: Option<&str>,
    ) -> Result<RailNode, RpcError> {
        self.insert_ordered(kind, name, parent, terminal_id, None)
    }

    /// `insert` with the node's order (O2, O3); `None` is the default of its kind.
    pub fn insert_ordered(
        &mut self,
        kind: NodeKind,
        name: &str,
        parent: Option<&str>,
        terminal_id: Option<&str>,
        work: Option<&Order>,
    ) -> Result<RailNode, RpcError> {
        let work = serde_json::to_string(&work.map_or_else(|| default_order(kind), Order::clone))
            .map_err(RpcError::internal)?;
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

    /// O13: record that Agent `spawner` started node `id`.
    pub fn set_spawner(&mut self, id: &str, spawner: &str) -> Result<(), RpcError> {
        self.node(id)?;
        self.db
            .execute(
                "UPDATE nodes SET spawner = ? WHERE id = ?",
                params![spawner, id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// O13: whether an Agent's `agent.spawn` started `id` and `door` is above it: the `parent`
    /// chain of `id` reaches `door`.
    pub fn spawned_under(&self, id: &str, door: &str) -> Result<bool, RpcError> {
        let started: Option<String> = self
            .db
            .query_row(
                "SELECT spawner FROM nodes WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .map_err(sql)?;
        if started.is_none() {
            return Ok(false);
        }
        let nodes = load(&self.db)?;
        let mut at = find(&nodes, id)?.parent.clone();
        while let Some(parent) = at {
            if parent == door {
                return Ok(true);
            }
            at = find(&nodes, &parent)?.parent.clone();
        }
        Ok(false)
    }

    /// O8: keep the prompt an Agent's spawn gave beside its order.
    pub fn set_spawn_prompt(&mut self, id: &str, prompt: &str) -> Result<(), RpcError> {
        self.node(id)?;
        self.db
            .execute(
                "UPDATE nodes SET spawn_prompt = ? WHERE id = ?",
                params![prompt, id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// O8: the prompt kept for `id`, `None` when it has none; `NOT_FOUND` for an unknown id.
    pub fn spawn_prompt(&self, id: &str) -> Result<Option<String>, RpcError> {
        self.node(id)?;
        self.db
            .query_row(
                "SELECT spawn_prompt FROM nodes WHERE id = ?",
                params![id],
                |row| row.get(0),
            )
            .map_err(sql)
    }

    /// O4: replace the order of an Agent or a Workstream.
    pub fn set_order(&mut self, id: &str, order: &Order) -> Result<RailNode, RpcError> {
        let node = self.node(id)?;
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

    /// A32: the Door of Workstream `id` is now in Iteration `iteration`.
    pub fn set_iteration(&mut self, id: &str, iteration: u32) -> Result<(), RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE nodes SET iteration = ? WHERE id = ?",
                params![iteration, id],
            )
            .map_err(sql)?;
        if changed == 0 {
            return Err(RpcError::not_found(format!("node {id}")));
        }
        Ok(())
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

    /// Delete `id`. Its children (a Workstream or an Agent may have any) move to its own
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

    /// A34: Rail, Todo, Pad and Touch rows share one transaction. The callback stages the
    /// external files and Worktrees before any node leaves; its returned guard survives commit.
    pub fn remove_workstream<T>(
        &mut self,
        id: &str,
        removal: &contracts::agent::Removal,
        apply: impl FnOnce(&Connection) -> Result<T, RpcError>,
    ) -> Result<T, RpcError> {
        self.db
            .pragma_update(None, "foreign_keys", true)
            .map_err(sql)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        let out = apply(&tx)?;
        let nodes = load(&tx)?;
        let node = find(&nodes, id)?;
        let mut ids = siblings(&nodes, node.parent.as_deref());
        let at = ids
            .iter()
            .position(|s| s == id)
            .expect("Workstream is a sibling");
        ids.splice(
            at..=at,
            removal
                .kept
                .iter()
                .filter(|i| i.kind == contracts::agent::RemovalKind::Agent)
                .map(|i| i.id.clone()),
        );
        place(&tx, node.parent.as_deref(), &ids)?;
        for item in removal.removed.iter().rev() {
            if item.kind == contracts::agent::RemovalKind::Agent {
                tx.execute("DELETE FROM nodes WHERE id = ?", [&item.id])
                    .map_err(sql)?;
            }
        }
        tx.execute("DELETE FROM nodes WHERE id = ?", [id])
            .map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(out)
    }

    /// M7: record that Agent `creator` created node `id`; it goes with the node.
    pub fn set_creator(&mut self, id: &str, creator: &str) -> Result<(), RpcError> {
        self.db
            .execute(
                "UPDATE nodes SET creator = ? WHERE id = ?",
                params![creator, id],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// M7: the ids of the Workstreams Agent `creator` created and that still exist.
    pub fn created_by(&self, creator: &str) -> Result<Vec<String>, RpcError> {
        let mut query = self
            .db
            .prepare("SELECT id FROM nodes WHERE creator = ?")
            .map_err(sql)?;
        let rows = query
            .query_map(params![creator], |row| {
                Ok(row.get::<_, i64>(0)?.to_string())
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
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

    /// M2219: the Project's `mcp` setting; `{on: true}` for a Project that never set it.
    pub fn get_mcp(&self) -> Result<Mcp, RpcError> {
        self.db
            .query_row("SELECT mcp_on FROM settings WHERE id = 0", [], |row| {
                Ok(Mcp { on: row.get(0)? })
            })
            .optional()
            .map_err(sql)
            .map(|found| found.unwrap_or(Mcp { on: true }))
    }

    pub fn set_mcp(&mut self, mcp: &Mcp) -> Result<(), RpcError> {
        self.db
            .execute(
                "INSERT INTO settings (id, mcp_on) VALUES (0, ?)
                 ON CONFLICT (id) DO UPDATE SET mcp_on = excluded.mcp_on",
                params![mcp.on],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// A36: the Models and Efforts set per Role, one entry for each of `roles` in order (`None`s
    /// for one never set), and the Models a Door may use.
    pub fn get_models(&self, roles: &[&str]) -> Result<ProjectModels, RpcError> {
        let mut set = self
            .db
            .prepare("SELECT role, model, effort FROM role_models")
            .map_err(sql)?;
        let set: Vec<RoleModels> = set
            .query_map([], |row| {
                Ok(RoleModels {
                    role: row.get(0)?,
                    model: row.get(1)?,
                    effort: row.get(2)?,
                })
            })
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        let roles = roles
            .iter()
            .map(|role| {
                set.iter()
                    .find(|found| found.role == *role)
                    .cloned()
                    .unwrap_or_else(|| RoleModels {
                        role: (*role).to_owned(),
                        model: None,
                        effort: None,
                    })
            })
            .collect();
        Ok(ProjectModels { roles })
    }

    /// A36: replace the Project's Models whole.
    pub fn set_models(&mut self, models: &ProjectModels) -> Result<(), RpcError> {
        let tx = self.db.transaction().map_err(sql)?;
        tx.execute("DELETE FROM role_models", []).map_err(sql)?;
        for role in &models.roles {
            tx.execute(
                "INSERT INTO role_models (role, model, effort) VALUES (?, ?, ?)",
                params![role.role, role.model, role.effort],
            )
            .map_err(sql)?;
        }
        tx.commit().map_err(sql)
    }

    /// A36: store the Model and Effort node `id` runs on.
    pub fn set_pair(
        &mut self,
        id: &str,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> Result<(), RpcError> {
        let changed = self
            .db
            .execute(
                "UPDATE nodes SET model = ?, effort = ? WHERE id = ?",
                params![model, effort, id],
            )
            .map_err(sql)?;
        if changed == 0 {
            return Err(RpcError::not_found(format!("node {id}")));
        }
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

/// X11: a Rail stored by an earlier Daemon may hold nodes of kind `terminal`; their programs are
/// gone, so they go, and each parent's remaining children close the gap.
fn drop_terminal_nodes(db: &Connection) -> rusqlite::Result<()> {
    let parents: Vec<Option<i64>> = db
        .prepare("SELECT DISTINCT parent FROM nodes WHERE kind = 'terminal'")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    db.execute("DELETE FROM nodes WHERE kind = 'terminal'", [])?;
    for parent in parents {
        let ids: Vec<i64> = db
            .prepare("SELECT id FROM nodes WHERE parent IS ? ORDER BY ord, id")?
            .query_map(params![parent], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        for (at, id) in ids.iter().enumerate() {
            db.execute(
                "UPDATE nodes SET ord = ? WHERE id = ?",
                params![at as i64, id],
            )?;
        }
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
    }
}

fn load(db: &Connection) -> Result<Vec<RailNode>, RpcError> {
    let mut query = db
        .prepare(
            "SELECT id, kind, name, parent, ord, attempt, terminal_id,
                worktree_path, worktree_branch, worktree_base, work, model, effort, iteration,
                conversation_id IS NOT NULL AND conversation_cwd IS NOT NULL FROM nodes",
        )
        .map_err(sql)?;
    let rows = query
        .query_map([], |row| {
            let kind = match row.get::<_, String>(1)?.as_str() {
                "group" | "workstream" => NodeKind::Workstream,
                _ => NodeKind::Agent,
            };
            let worktree = match (
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ) {
                (Some(path), Some(branch), Some(base)) => Some(Worktree { path, branch, base }),
                _ => None,
            };
            let work = Some(
                row.get::<_, Option<String>>(10)?
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_else(|| default_order(kind)),
            );
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
                // R16/#1365: 40 per-row conversation queries cost 209 µs in the release
                // profile. Load availability here; presentation gates it on the live Attempt.
                can_resume: row.get(14)?,
                channel: None,
                stray: Vec::new(),
                work,
                model: row.get(11)?,
                effort: row.get(12)?,
                iteration: (kind == NodeKind::Workstream)
                    .then(|| row.get(13))
                    .transpose()?,
                inbox: None,
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
/// started it.
fn check_parent(nodes: &[RailNode], kind: NodeKind, parent: Option<&str>) -> Result<(), RpcError> {
    let Some(parent) = parent.map(|id| find(nodes, id)).transpose()? else {
        return Ok(());
    };
    match (kind, parent.kind) {
        (NodeKind::Workstream, _) => Err(RpcError::conflict(format!(
            "Workstreams never nest: a Workstream cannot sit under {}",
            parent.id
        ))),
        (NodeKind::Agent, _) => Ok(()),
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
fn place(tx: &Connection, parent: Option<&str>, ids: &[String]) -> Result<(), RpcError> {
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
    fn r16_loaded_rail_keeps_complete_conversation_availability_in_tree_order() {
        let (_dir, mut rail) = rail();
        let first = rail
            .insert(NodeKind::Workstream, "first", None, None)
            .unwrap();
        let child = rail
            .insert(NodeKind::Agent, "child", Some(&first.id), None)
            .unwrap();
        let second = rail
            .insert(NodeKind::Workstream, "second", None, None)
            .unwrap();
        rail.db.execute("UPDATE nodes SET conversation_id = 'saved', conversation_cwd = '/project' WHERE id = ?", [&child.id]).unwrap();
        rail.db
            .execute(
                "UPDATE nodes SET conversation_id = 'partial' WHERE id = ?",
                [&second.id],
            )
            .unwrap();
        let tree = rail.tree().unwrap();
        assert_eq!(
            tree.iter()
                .map(|n| (n.id.as_str(), n.can_resume))
                .collect::<Vec<_>>(),
            [
                (first.id.as_str(), false),
                (child.id.as_str(), true),
                (second.id.as_str(), false),
            ]
        );
        assert!(rail.node(&child.id).unwrap().can_resume);
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
    fn a6_nothing_nests_under_an_agent() {
        let (_dir, mut rail) = rail();
        let group = rail.insert(NodeKind::Workstream, "g", None, None).unwrap();
        {
            let kind = NodeKind::Agent;
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
    fn x11_a_stored_terminal_node_is_dropped_on_open() {
        let (dir, mut rail) = rail();
        let group = rail.insert(NodeKind::Workstream, "g", None, None).unwrap();
        for name in ["a", "b", "c"] {
            rail.insert(NodeKind::Agent, name, Some(&group.id), None)
                .unwrap();
        }
        rail.db
            .execute("UPDATE nodes SET kind = 'terminal' WHERE name = 'b'", [])
            .unwrap();
        drop(rail);

        let reopened = Rail::open(&dir.path().join("agents.db")).unwrap();

        let tree = reopened.tree().unwrap();
        let rest: Vec<_> = tree.iter().map(|n| (n.name.as_str(), n.order)).collect();
        assert_eq!(rest, [("g", 0), ("a", 0), ("c", 1)]);
        assert!(tree.iter().all(|n| n.name != "b"));
    }

    #[test]
    fn a8_reopening_unlinks_every_terminal_id() {
        let (dir, mut rail) = rail();
        rail.insert(NodeKind::Agent, "claude", None, Some("1"))
            .unwrap();
        rail.insert(NodeKind::Agent, "shell", None, Some("2"))
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
