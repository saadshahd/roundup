//! The real `rupd` and `rup` on a temp Project, with the fake `claude` as every Agent's program.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use contracts::agent::{RailNode, SignalParams, SpawnParams, StatusEvent};
use contracts::{EventData, Kind, Status};
use rpc::Client;
use serde_json::Value;

/// How long the Daemon, its Agents' hooks and their tools have to answer.
const BOUND: Duration = Duration::from_secs(30);

/// How long ten Agents' `SessionStart` hooks have to reach the Daemon (D4's bound), counted from
/// the first `agent.spawn`. Kept apart from `BOUND`, which `Drop` also uses, so this cannot drift.
pub const TEN_IDLE_BOUND: Duration = Duration::from_secs(30);

pub struct Project {
    daemon: Child,
    /// Held open: `--attached` makes the Daemon exit, and stop its Agents, when it closes.
    stdin: Option<ChildStdin>,
    pub dir: tempfile::TempDir,
}

/// Start `rupd --attached` and return once it is serving. `fake` is the environment the fake
/// `claude` reads.
pub fn start(fake: &[(&str, &str)]) -> Project {
    start_in(tempfile::tempdir().unwrap(), fake)
}

fn start_in(dir: tempfile::TempDir, fake: &[(&str, &str)]) -> Project {
    let rup = PathBuf::from(env!("CARGO_BIN_EXE_rup"));
    let rupd = rup.with_file_name("rupd");
    assert!(
        rupd.is_file(),
        "{} is not built; run `cargo build -p rupd`",
        rupd.display()
    );
    let mut daemon = Command::new(rupd)
        .arg(dir.path())
        .arg("--attached")
        .env("RUPD_SOCKET", socket(dir.path()))
        .env("CLAUDE_CONFIG_DIR", dir.path().join("claude-config"))
        .env("ROUNDUP_RUP_BIN", rup)
        .env(
            "ROUNDUP_CLAUDE_BIN",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/fake_claude.py"),
        )
        .envs(fake.iter().copied())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(daemon.stderr.take().unwrap());
    let mut serving = String::new();
    stderr.read_line(&mut serving).unwrap();
    assert!(serving.contains("serving"), "unexpected: {serving:?}");
    // Left unread, the pipe would fill and block the Daemon's logging.
    std::thread::spawn(move || std::io::copy(&mut stderr, &mut std::io::stderr()));
    Project {
        stdin: daemon.stdin.take(),
        daemon,
        dir,
    }
}

fn socket(dir: &Path) -> PathBuf {
    dir.join("rupd.sock")
}

impl Project {
    pub fn crash(&mut self) {
        if self.daemon.try_wait().unwrap().is_none() {
            self.daemon.kill().unwrap();
        }
        self.daemon.wait().unwrap();
        self.stdin.take();
    }

    pub fn restart(&mut self, fake: &[(&str, &str)]) {
        self.crash();
        let placeholder = tempfile::tempdir().unwrap();
        let dir = std::mem::replace(&mut self.dir, placeholder);
        let restarted = start_in(dir, fake);
        *self = restarted;
    }

    pub fn pid(&self) -> u32 {
        self.daemon.id()
    }

    pub async fn client(&self) -> Client {
        Client::connect(&socket(self.dir.path())).await.unwrap()
    }

    /// A client that receives every event from now on.
    pub async fn subscribed(&self) -> Client {
        let client = self.client().await;
        client
            .request("events.subscribe", serde_json::Value::Null)
            .await
            .unwrap();
        client
    }

    pub async fn spawn_agent(&self, client: &Client) -> RailNode {
        let params = SpawnParams {
            cwd: self.dir.path().to_string_lossy().into_owned(),
            prompt: None,
            parent: None,
        };
        let node = client.request("agent.spawn", params).await.unwrap();
        serde_json::from_value(node).unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + BOUND;
        while Instant::now() < deadline && self.daemon.try_wait().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// The next event the Daemon pushes that satisfies `pick`.
pub async fn next<T>(client: &mut Client, pick: impl Fn(EventData) -> Option<T>) -> T {
    tokio::time::timeout(BOUND, async {
        while let Some(event) = client.next_event().await {
            if let Some(found) = pick(event.data) {
                return found;
            }
        }
        panic!("the connection closed before the event");
    })
    .await
    .expect("the event arrived")
}

async fn next_status(client: &mut Client) -> StatusEvent {
    next(client, |data| match data {
        EventData::AgentStatus(status) => Some(status),
        _ => None,
    })
    .await
}

pub async fn next_kind(client: &mut Client) -> Kind {
    next_status(client).await.status.kind
}

/// Hand Agent `id` a Signal directly over RPC, the way `rup signal` does, bypassing the fake
/// `claude` process so a test can drive one Agent's Status without the others.
pub async fn signal(client: &Client, id: &str, payload: Value) {
    let incarnation = rail_tree(client)
        .await
        .into_iter()
        .find(|n| n.id == id)
        .unwrap()
        .incarnation
        .unwrap();
    tokio::time::timeout(
        BOUND,
        client.request(
            "agent.signal",
            SignalParams {
                id: id.to_owned(),
                incarnation,
                payload,
            },
        ),
    )
    .await
    .expect("agent.signal answered within the bound")
    .unwrap();
}

pub async fn rail_tree(client: &Client) -> Vec<RailNode> {
    let tree = tokio::time::timeout(BOUND, client.request("rail.tree", Value::Null))
        .await
        .expect("rail.tree answered within the bound")
        .unwrap();
    serde_json::from_value(tree).unwrap()
}

/// Wait, from one shared `deadline`, for every one of `ids` to reach `Kind::Idle` on the event
/// stream. A miss names, for each Agent not yet Idle, the status last seen on the event stream
/// (`None` if none arrived) and the Daemon's own `rail.tree` view of it: if the tree already shows
/// Idle, the event did not reach this client even though the Daemon already saw it. A connection
/// that closes before the deadline is reported directly, instead of blaming the deadline and
/// then failing to read `rail.tree` on a dead connection.
pub async fn wait_until_idle(
    client: &mut Client,
    ids: &[String],
    deadline: Instant,
) -> Result<(), String> {
    let mut idle: HashSet<String> = HashSet::new();
    let mut last_seen: HashMap<String, Status> = HashMap::new();
    while idle.len() < ids.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = match tokio::time::timeout(remaining, client.next_event()).await {
            Ok(Some(event)) => event,
            Ok(None) => {
                return Err(format!(
                    "the connection closed with {} of {} Agents not yet Idle",
                    ids.len() - idle.len(),
                    ids.len()
                ));
            }
            Err(_) => break,
        };
        if let EventData::AgentStatus(status) = event.data
            && ids.contains(&status.id)
        {
            if status.status.kind == Kind::Idle {
                idle.insert(status.id.clone());
            }
            last_seen.insert(status.id, status.status);
        }
    }
    if idle.len() == ids.len() {
        return Ok(());
    }

    let tree = rail_tree(client).await;
    let mut report = format!(
        "{} of {} Agents were not Idle before the deadline:\n",
        ids.len() - idle.len(),
        ids.len()
    );
    for id in ids.iter().filter(|id| !idle.contains(*id)) {
        let tree_status = tree
            .iter()
            .find(|node| &node.id == id)
            .and_then(|node| node.status.as_ref());
        let idle_in_tree = matches!(tree_status, Some(status) if status.kind == Kind::Idle);
        report += &format!(
            "  {id}: event stream last saw {:?}; rail.tree shows {tree_status:?}{}\n",
            last_seen.get(id),
            if idle_in_tree {
                " (Idle in the tree but not seen on the stream)"
            } else {
                ""
            }
        );
    }
    Err(report)
}
