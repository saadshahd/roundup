//! Git owns blob identity and compression; private tree refs retain stored Marks across GC.
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use rpc::RpcError;
use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(crate) struct Object(String);

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl Object {
    fn parse(text: &str) -> Result<Self, RpcError> {
        if text.len() != 40
            || !text
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(RpcError::internal(format!("invalid Git object: {text}")));
        }
        Ok(Self(text.into()))
    }

    pub(crate) fn name(&self) -> &str {
        &self.0
    }
}

pub(crate) struct Objects {
    path: PathBuf,
}

impl Objects {
    pub(crate) fn new(dir: &Path) -> io::Result<Self> {
        Ok(Self {
            path: std::fs::canonicalize(dir)?.join("objects.git"),
        })
    }

    pub(crate) fn init(&self) -> Result<(), RpcError> {
        if !self.path.join("config").exists() {
            self.run(
                &[
                    "init",
                    "--bare",
                    "--object-format=sha1",
                    "--template=",
                    "--initial-branch=checkpoints",
                ],
                &[],
            )?;
        }
        Ok(())
    }

    pub(crate) fn write(&self, bytes: &[u8]) -> Result<Object, RpcError> {
        self.oid(&["hash-object", "-w", "--stdin", "--no-filters"], bytes)
    }

    pub(crate) fn retain<'a>(
        &self,
        workstream: &str,
        id: u32,
        objects: impl Iterator<Item = &'a Object>,
    ) -> Result<(), RpcError> {
        let mut entries = String::new();
        for object in objects {
            entries.push_str(&format!(
                "100644 blob {}\t{}\n",
                object.name(),
                object.name()
            ));
        }
        let tree = self.oid(&["mktree"], entries.as_bytes())?;
        let identity: String = workstream
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        // Content-specific refs never replace retention for an earlier successful write.
        let reference = format!("refs/checkpoints/w{identity}/{id}/{}", tree.name());
        self.run(&["update-ref", "--no-deref", &reference, tree.name()], &[])?;
        Ok(())
    }

    pub(crate) fn read(&self, object: &Object) -> Result<Vec<u8>, RpcError> {
        let bytes = self.run(&["cat-file", "blob", object.name()], &[])?;
        // cat-file can decode a damaged loose blob without checking its pathname's hash.
        let actual = self.oid(&["hash-object", "--stdin", "--no-filters"], &bytes)?;
        if actual != *object {
            return Err(RpcError::internal(format!(
                "corrupt Git object {} (read {})",
                object.name(),
                actual.name()
            )));
        }
        Ok(bytes)
    }

    fn oid(&self, args: &[&str], input: &[u8]) -> Result<Object, RpcError> {
        let output = self.run(args, input)?;
        Object::parse(
            std::str::from_utf8(&output)
                .map_err(RpcError::internal)?
                .trim(),
        )
    }

    fn run(&self, args: &[&str], input: &[u8]) -> Result<Vec<u8>, RpcError> {
        let mut command = Command::new("git");
        // Inherited Git overrides must not redirect private writes into the Project's repository.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .current_dir(self.path.parent().expect("object store has a parent"))
            .arg("--git-dir")
            .arg(&self.path)
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "gc.auto=0",
                "-c",
                "core.fsync=loose-object,reference",
            ])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let result = execute(command, input, Duration::from_secs(30));
        result.map_err(|error| RpcError::internal(format!("Git {}: {error}", args.join(" "))))
    }
}

// Drain both outputs while feeding stdin, so large binary files cannot fill a pipe and deadlock.
fn execute(mut command: Command, input: &[u8], timeout: Duration) -> io::Result<Vec<u8>> {
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = scope.spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes)?;
            Ok::<_, io::Error>(bytes)
        });
        let errors = scope.spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes)?;
            Ok::<_, io::Error>(bytes)
        });
        let status = match child.wait_timeout(timeout) {
            Ok(Some(status)) => Ok(status),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                match result {
                    Err(error) => Err(error),
                    _ => Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("{} second deadline exceeded", timeout.as_secs_f64()),
                    )),
                }
            }
        };
        let written = writer.join().expect("Git stdin thread");
        let output = output.join().expect("Git stdout thread")?;
        let errors = errors.join().expect("Git stderr thread")?;
        if !status?.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&errors).into_owned(),
            ));
        }
        written?;
        Ok(output)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n14_git_deadline_terminates_stalled_command() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "exec sleep 60"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let error = execute(command, &[], Duration::from_millis(50)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
