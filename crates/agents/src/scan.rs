//! F5: find, by name, a vendor program running under a Door that the Daemon did not start. The
//! names come from the Adapters (`claude_code::programs`); this file writes none. It reads the
//! process table once per scan and prevents nothing.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Stdio};

use contracts::agent::Stray;

/// Programs that run the file named after them: their program name is that file's.
const INTERPRETERS: [&str; 7] = ["sh", "bash", "dash", "zsh", "node", "python", "python3"];

/// One row of `ps -o pid=,ppid=,comm=,args=`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pid: u32,
    ppid: u32,
    comm: String,
    args: String,
}

/// The process table as `ps` prints it, which macOS and Linux both do.
pub(crate) fn read_table() -> std::io::Result<String> {
    let out = Command::new("ps")
        .args(["-ax", "-ww", "-o", "pid=,ppid=,comm=,args="])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!("ps exited {}", out.status)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The rows of `table`; a row that is not `pid ppid comm args` is skipped.
pub(crate) fn parse(table: &str) -> Vec<Row> {
    table
        .lines()
        .filter_map(|line| {
            let (pid, rest) = line.trim_start().split_once(char::is_whitespace)?;
            let (ppid, rest) = rest.trim_start().split_once(char::is_whitespace)?;
            let rest = rest.trim_start();
            let (comm, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Some(Row {
                pid: pid.parse().ok()?,
                ppid: ppid.parse().ok()?,
                comm: comm.to_owned(),
                args: args.trim().to_owned(),
            })
        })
        .collect()
}

fn base(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

/// The names a process answers to: the basename of its executable and of `argv[0]`, and, for an
/// interpreter, of the first argument that is not an option.
fn names(row: &Row) -> Vec<&str> {
    let mut words = row.args.split_whitespace();
    let argv0 = words.next().map_or("", base);
    let mut names = vec![base(&row.comm), argv0];
    if INTERPRETERS.contains(&argv0) {
        names.extend(words.find(|word| !word.starts_with('-')).map(base));
    }
    names
}

/// The processes below any process whose arguments hold `marker` (a Door's own program), not
/// those themselves, that go by one of `programs`. A Terminal the Daemon registered is a child of
/// the Daemon and so never below a Door.
pub(crate) fn strays(table: &[Row], marker: &str, programs: &[&str]) -> Vec<Stray> {
    let own: HashSet<u32> = table
        .iter()
        .filter(|row| row.args.contains(marker))
        .map(|row| row.pid)
        .collect();
    let mut children: HashMap<u32, Vec<&Row>> = HashMap::new();
    for row in table {
        children.entry(row.ppid).or_default().push(row);
    }
    let mut found = Vec::new();
    let mut seen = own.clone();
    let mut pending: Vec<u32> = own.iter().copied().collect();
    while let Some(parent) = pending.pop() {
        for child in children.get(&parent).into_iter().flatten() {
            if !seen.insert(child.pid) {
                continue;
            }
            pending.push(child.pid);
            if names(child).iter().any(|name| programs.contains(name)) {
                found.push(Stray {
                    pid: child.pid,
                    command: child.args.clone(),
                });
            }
        }
    }
    found.sort_by_key(|stray| stray.pid);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
    1     0 init            /sbin/init
  100     1 fake-claude     sh /p/fake-claude --settings /p/7.settings.json
  101   100 sh              sh /tmp/x/claude --flag
  102   100 node            node /srv/mcp.js
  103   102 claude          claude -p hi
  104   100 sleep           sleep 30
  200     1 claude          claude --version
  105   101 sleep           sleep 5
";

    #[test]
    fn f5_a_vendor_name_below_the_door_is_a_stray_by_executable_or_by_interpreter_argument() {
        let found = strays(&parse(TABLE), "/p/7.settings.json", &["claude"]);

        let pids: Vec<_> = found.iter().map(|stray| stray.pid).collect();
        assert_eq!(pids, [101, 103]);
        assert_eq!(found[1].command, "claude -p hi");
    }

    #[test]
    fn f5_the_door_itself_and_a_row_outside_its_tree_are_not_strays() {
        let table = parse(TABLE);

        assert!(strays(&table, "/p/7.settings.json", &["other"]).is_empty());
        assert!(strays(&table, "/p/8.settings.json", &["claude"]).is_empty());
    }

    #[test]
    fn f5_a_row_that_is_not_a_row_is_skipped() {
        assert_eq!(parse("garbage\n\n  x y z\n 5 6 comm\n").len(), 1);
    }
}
