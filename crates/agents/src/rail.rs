//! The Rail: Groups, Meta-agents, Agents and Terminals as a tree in SQLite (WAL).
//! Sibling `order` values are contiguous from 0; only Groups (and so Meta-agents) hold children.

use std::path::Path;

use contracts::agent::{NodeKind, RailNode};
use contracts::{Kind, Status};
use rpc::{OpenError, RpcError};
use rusqlite::{Connection, params};

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
    fn a6_nothing_nests_under_an_agent() {
        let (_dir, mut rail) = rail();
        let agent = rail
            .insert(NodeKind::Agent, "claude", None, Some("1"))
            .unwrap();

        let err = rail
            .insert(NodeKind::Group, "x", Some(&agent.id), None)
            .unwrap_err();
        assert_eq!(err.code, code::CONFLICT);
    }

    #[test]
    fn a8_an_agent_whose_terminal_is_gone_comes_back_done() {
        let (dir, mut rail) = rail();
        rail.insert(NodeKind::Agent, "claude", None, Some("1"))
            .unwrap();
        drop(rail);

        let tree = Rail::open(&dir.path().join("agents.db"), 7)
            .unwrap()
            .tree()
            .unwrap();
        let status = tree[0].status.as_ref().unwrap();
        assert_eq!(status.kind, Kind::Done);
        assert_eq!(tree[0].terminal_id.as_deref(), Some("1"));
    }
}
