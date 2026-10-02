//! The real `rupd` and `rup` on a temp Project, with the fake `claude` as every Agent's program.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use contracts::agent::{RailNode, SpawnParams, StatusEvent};
use contracts::{EventData, Kind, Status};
use rpc::Client;

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
    let dir = tempfile::tempdir().unwrap();
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

pub async fn next_status(client: &mut Client) -> StatusEvent {
    next(client, |data| match data {
        EventData::AgentStatus(status) => Some(status),
        _ => None,
    })
    .await
}

pub async fn next_kind(client: &mut Client) -> Kind {
    next_status(client).await.status.kind
}

/// Wait, from one shared `deadline`, for every one of `ids` to reach `Kind::Idle` on the event
/// stream. A miss names, for each Agent not yet Idle, the status last seen on the event stream
/// (`None` if none arrived) and the Daemon's own `rail.tree` view of it: if the tree already says
/// Idle, the event was lost in delivery rather than the Daemon being slow to reach it.
pub async fn wait_until_idle(
    client: &mut Client,
    ids: &[String],
    deadline: Instant,
) -> Result<(), String> {
    let mut idle: HashSet<String> = HashSet::new();
    let mut last_seen: HashMap<String, Status> = HashMap::new();
    while idle.len() < ids.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let event = match tokio::time::timeout(remaining, client.next_event()).await {
            Ok(Some(event)) => event,
            Ok(None) | Err(_) => break,
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

    let tree: Vec<RailNode> = serde_json::from_value(
        client
            .request("rail.tree", serde_json::Value::Null)
            .await
            .unwrap(),
    )
    .unwrap();
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
        let lost = matches!(tree_status, Some(status) if status.kind == Kind::Idle);
        report += &format!(
            "  {id}: event stream last saw {:?}; rail.tree shows {tree_status:?}{}\n",
            last_seen.get(id),
            if lost {
                " (lost: the Daemon's own tree already says Idle)"
            } else {
                ""
            }
        );
    }
    Err(report)
}
