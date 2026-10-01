//! The Rail: Groups, Meta-agents, Agents and Terminals as a tree in SQLite (WAL).
//! Sibling `order` values are contiguous from 0; only Groups (and so Meta-agents) hold children.

use std::path::Path;

use contracts::agent::{NodeKind, RailNode};
use contracts::{Kind, Status};
use rpc::{OpenError, RpcError};
use rusqlite::{Connection, Transaction, params};

pub struct Rail {
    db: Connection,
    /// When this Rail was opened: since when an Agent without a Terminal has been `done`.
    opened: i64,
}

impl Rail {
    pub fn open(path: &Path, now: i64) -> Result<Self, OpenError> {
        let db = Connection::open(path)?;
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
            );",
        )?;
        // Terminal ids restart at 1 with the Daemon, so a stored id could name an unrelated Terminal.
        db.execute("UPDATE nodes SET terminal_id = NULL", [])?;
        Ok(Self { db, opened: now })
    }

    /// Every node, each parent before its children, siblings by `order`.
    pub fn tree(&self) -> Result<Vec<RailNode>, RpcError> {
        let mut flat = load(&self.db, self.opened)?;
        flat.sort_by_key(|node| node.order);
        let mut out = Vec::with_capacity(flat.len());
        descend(&flat, None, &mut out);
        Ok(out)
    }

    /// Append a node under `parent` (`None` is the root). Only a Group can be a parent.
    pub fn insert(
        &mut self,
        kind: NodeKind,
        name: &str,
        parent: Option<&str>,
        terminal_id: Option<&str>,
    ) -> Result<RailNode, RpcError> {
        let tx = self.db.transaction().map_err(sql)?;
        let nodes = load(&tx, self.opened)?;
        check_parent(&nodes, parent)?;
        let order = siblings(&nodes, parent).len() as i64;
        tx.execute(
            "INSERT INTO nodes (kind, name, parent, ord, terminal_id) VALUES (?, ?, ?, ?, ?)",
            params![kind_name(kind), name, parent, order, terminal_id],
        )
        .map_err(sql)?;
        let id = tx.last_insert_rowid().to_string();
        tx.commit().map_err(sql)?;
        self.node(&id)
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
        let nodes = load(&tx, self.opened)?;
        let node = find(&nodes, id)?;
        check_parent(&nodes, parent)?;
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

    fn node(&self, id: &str) -> Result<RailNode, RpcError> {
        find(&load(&self.db, self.opened)?, id).cloned()
    }
}

fn sql(err: rusqlite::Error) -> RpcError {
    RpcError::internal(err)
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Group => "group",
        NodeKind::Agent => "agent",
        NodeKind::Terminal => "terminal",
    }
}

fn load(db: &Connection, opened: i64) -> Result<Vec<RailNode>, RpcError> {
    let mut query = db
        .prepare("SELECT id, kind, name, parent, ord, meta, terminal_id FROM nodes")
        .map_err(sql)?;
    let rows = query
        .query_map([], |row| {
            let kind = match row.get::<_, String>(1)?.as_str() {
                "group" => NodeKind::Group,
                "agent" => NodeKind::Agent,
                _ => NodeKind::Terminal,
            };
            Ok(RailNode {
                id: row.get::<_, i64>(0)?.to_string(),
                kind,
                name: row.get(2)?,
                parent: row.get::<_, Option<i64>>(3)?.map(|id| id.to_string()),
                order: row.get(4)?,
                // No Terminal outlives the Daemon, so a stored Agent is never alive.
                status: (kind == NodeKind::Agent).then(|| Status {
                    kind: Kind::Done,
                    label: "terminal gone".into(),
                    since: opened,
                }),
                meta: row.get(5)?,
                terminal_id: row.get(6)?,
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

fn check_parent(nodes: &[RailNode], parent: Option<&str>) -> Result<(), RpcError> {
    match parent.map(|id| find(nodes, id)).transpose()? {
        Some(node) if node.kind != NodeKind::Group => Err(RpcError::conflict(format!(
            "{} is not a Group: only Groups and Meta-agents hold children",
            node.id
        ))),
        _ => Ok(()),
    }
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
    use contracts::Kind;
    use contracts::agent::NodeKind;
    use rpc::code;

    use super::Rail;

    fn rail() -> (tempfile::TempDir, Rail) {
        let dir = tempfile::tempdir().unwrap();
        let rail = Rail::open(&dir.path().join("agents.db"), 7).unwrap();
        (dir, rail)
    }

    #[test]
    fn a6_nothing_nests_under_an_agent_or_a_terminal() {
        let (_dir, mut rail) = rail();
        let group = rail.insert(NodeKind::Group, "g", None, None).unwrap();
        for kind in [NodeKind::Agent, NodeKind::Terminal] {
            let leaf = rail.insert(kind, "leaf", None, Some("1")).unwrap();
            let inserted = rail
                .insert(NodeKind::Group, "x", Some(&leaf.id), None)
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
        let group = rail.insert(NodeKind::Group, "g", None, None).unwrap();
        let err = rail.rename(&format!("0{}", group.id), "other").unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
        assert_eq!(rail.tree().unwrap()[0].name, "g");
    }

    #[test]
    fn a8_an_agent_whose_terminal_is_gone_comes_back_done_and_unlinked() {
        let (dir, mut rail) = rail();
        rail.insert(NodeKind::Agent, "claude", None, Some("1"))
            .unwrap();
        drop(rail);

        let mut reopened = Rail::open(&dir.path().join("agents.db"), 9).unwrap();
        // A new, unrelated Terminal reuses id 1 after the restart.
        reopened
            .insert(NodeKind::Terminal, "shell", None, Some("1"))
            .unwrap();
        let tree = reopened.tree().unwrap();

        let agent = &tree[0];
        let status = agent.status.as_ref().unwrap();
        assert_eq!((status.kind, status.since), (Kind::Done, 9));
        assert_eq!(agent.terminal_id, None);
        assert_eq!(tree[1].terminal_id.as_deref(), Some("1"));
    }
}
