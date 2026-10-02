//! A real `rupd` in a child process of its own, as A4's and A15's tests both need: only a process
//! of its own can be given a fake `claude` and extra environment variables before it starts.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub const SERVE_ENV: &str = "ROUNDUP_TEST_SERVE_DIR";

/// Not a test of its own: with `SERVE_ENV` set this process becomes a Daemon until it is killed.
#[tokio::test]
async fn serve_for_the_parent() {
    let Some(dir) = std::env::var_os(SERVE_ENV) else {
        return;
    };
    let dir = Path::new(&dir);
    let daemon = rupd::Daemon::open(&dir.join(".roundup")).unwrap();
    let listener = tokio::net::UnixListener::bind(dir.join("rupd.sock")).unwrap();
    rupd::serve(listener, std::sync::Arc::new(daemon))
        .await
        .unwrap();
}

/// A Daemon child process that is killed when this drops.
pub struct Served {
    pub dir: tempfile::TempDir,
    child: Child,
}

impl Served {
    /// `fake_claude` is the script `ROUNDUP_CLAUDE_BIN` points the Daemon at; `extra_env` is set on
    /// the Daemon's own environment before it starts.
    pub async fn start(fake_claude: &str, extra_env: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake-claude");
        std::fs::write(&fake, fake_claude).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "common::serve_for_the_parent"])
            .env(SERVE_ENV, dir.path())
            .env("ROUNDUP_CLAUDE_BIN", &fake)
            .env("ROUNDUP_RUP_BIN", env!("CARGO_BIN_EXE_rup"))
            .env("CLAUDE_CONFIG_DIR", dir.path())
            .envs(extra_env.iter().copied())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let served = Self { dir, child };
        for _ in 0..500 {
            if rpc::Client::connect(&served.socket()).await.is_ok() {
                return served;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the Daemon never listened");
    }

    pub fn socket(&self) -> std::path::PathBuf {
        self.dir.path().join("rupd.sock")
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
