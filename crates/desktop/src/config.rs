use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `sockaddr_un.sun_path` on macOS holds 104 bytes including the terminating NUL.
const SOCKET_PATH_LIMIT: usize = 104;

/// How long `open_project` waits for the Daemon to answer `daemon.ping`.
pub const READY_BOUND: Duration = Duration::from_secs(10);

/// How long one `rpc` call waits for its reply before the App reports an unknown outcome.
pub const CALL_BOUND: Duration = Duration::from_secs(30);

/// Everything the App reads from its environment, resolved once at startup.
#[derive(Clone, Debug)]
pub struct Config {
    pub rupd_bin: PathBuf,
    pub socket: PathBuf,
    pub ready_bound: Duration,
    pub call_bound: Duration,
}

impl Config {
    /// `rupd_env` is `ROUNDUP_RUPD_BIN`; without it the Daemon is the `rupd` beside `exe`.
    /// The socket is unique to this run through `pid`.
    pub fn locate(
        rupd_env: Option<OsString>,
        exe: &Path,
        socket_dir: &Path,
        pid: u32,
    ) -> Result<Self, String> {
        let rupd_bin = rupd_env
            .map(PathBuf::from)
            .unwrap_or_else(|| exe.parent().unwrap_or_else(|| Path::new(".")).join("rupd"));
        let socket = socket_dir.join(format!("roundup-{pid}.sock"));
        check_limit(&socket)?;
        Ok(Self {
            rupd_bin,
            socket,
            ready_bound: READY_BOUND,
            call_bound: CALL_BOUND,
        })
    }

    /// The socket path for one Daemon run: `socket` unchanged for the first (`run` 0), and a
    /// fresh path for each further run (a reopen, S5) so the old and new Daemon never share one.
    /// A reopen whose prefixed path would break the 104-byte limit is refused here, before the
    /// App ever asks `rupd` to bind it (S5 keeps S1's "under the 104-byte limit" exactly).
    pub(crate) fn socket_for_run(&self, run: u64) -> Result<PathBuf, String> {
        if run == 0 {
            return Ok(self.socket.clone());
        }
        let file_name = self
            .socket
            .file_name()
            .expect("the socket path has a file name");
        let path = self
            .socket
            .with_file_name(format!("{run}-{}", file_name.to_string_lossy()));
        check_limit(&path)?;
        Ok(path)
    }

    pub fn from_env() -> Result<Self, String> {
        let exe = std::env::current_exe()
            .map_err(|err| format!("cannot find the App's own executable: {err}"))?;
        Self::locate(
            std::env::var_os("ROUNDUP_RUPD_BIN"),
            &exe,
            &std::env::temp_dir(),
            std::process::id(),
        )
    }
}

fn check_limit(socket: &Path) -> Result<(), String> {
    let bytes = socket.as_os_str().len();
    if bytes >= SOCKET_PATH_LIMIT {
        return Err(format!(
            "the Daemon socket path {} is {bytes} bytes; a unix socket path must be under {SOCKET_PATH_LIMIT}",
            socket.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `s1_a_socket_path_of_103_bytes_is_accepted_and_104_is_rejected` (`tests/app.rs`):
    /// builds a base socket path of an exact byte length so a reopen's `{run}-` prefix can be
    /// pushed past the limit on purpose.
    fn dir_of(socket_len: usize) -> PathBuf {
        let socket_name_len = "/roundup-1.sock".len();
        PathBuf::from(format!("/{}", "d".repeat(socket_len - socket_name_len - 1)))
    }

    #[test]
    fn s5_a_reopen_whose_prefixed_path_would_break_the_limit_is_refused_up_front() {
        let config = Config::locate(None, Path::new("/a/roundup"), &dir_of(102), 1).unwrap();
        assert_eq!(config.socket.as_os_str().len(), 102);

        let reopened = config.socket_for_run(1);

        let err = reopened.expect_err("a 104-byte reopen path must be refused, not bind-failed");
        assert!(err.contains("104"));
    }

    #[test]
    fn s5_a_reopen_whose_prefixed_path_stays_under_the_limit_is_accepted() {
        let config = Config::locate(None, Path::new("/a/roundup"), &dir_of(101), 1).unwrap();

        let reopened = config.socket_for_run(1).unwrap();

        assert_eq!(reopened.as_os_str().len(), 103);
    }
}
