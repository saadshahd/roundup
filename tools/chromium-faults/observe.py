"""Run an unchanged command with privileged, bounded Chromium fault evidence.

The kernel retains each blocked fault before browser teardown, including BTF
backing-file identity and the wait stack. Live pending reports wait 100 ms to
avoid flooding CI with ordinary disk reads; capture itself has no age filter.
Keep at most 1024 active faults and the longest 128 completed faults. Publish
at most 256 live fault pairs and 256 live-set acknowledgements, plus final evidence. Every
limit or tracer failure is explicit and never changes the command's result.
Live state comes from kernel-map snapshots, not unmatched stream notifications;
only a detached final snapshot can verify final pending evidence. Lost events
invalidate history even when an independent snapshot recovers live state.
Console backpressure disables console reports once; the artifact keeps capturing.
"""
import argparse
import datetime
import heapq
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import threading
import time

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    command = options.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("a command is required")
    log = None
    log_error = None
    if options.log:
        try:
            log = options.log.open("w")
        except OSError as error:
            log_error = str(error)
    output_lock = threading.Lock()
    output_fd = None
    output_error = None
    try:
        # Reopen rather than dup: O_NONBLOCK must not alter the caller's or
        # command's shared file description. No Python buffer can later flush
        # a failed write during shutdown and replace the command's exit status.
        if stat.S_ISREG(os.fstat(1).st_mode):
            # Preserve the shared offset when stdout is already a file.
            output_fd = os.dup(1)
        else:
            output_fd = os.open("/proc/self/fd/1", os.O_WRONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOCTTY)
    except OSError as error:
        output_error = str(error)

    def publish(event):
        nonlocal log, output_fd, output_error
        with output_lock:
            value = json.dumps(event) + "\n"
            log_failure = None
            if log:
                try:
                    log.write(value)
                    log.flush()
                except OSError as error:
                    log = None
                    log_failure = json.dumps({"status": "error", "detail": f"fault log write: {error}"}) + "\n"
            if output_fd is not None:
                try:
                    # One nonblocking attempt, no queue or retry. A short write
                    # loses the remainder too; disable output before another
                    # record could be appended to that incomplete JSON line.
                    payload = ((log_failure or "") + value).encode("utf-8")
                    written = os.write(output_fd, payload)
                    if written != len(payload):
                        output_error = f"short write ({written}/{len(payload)} bytes)"
                except OSError as error:
                    output_error = str(error)
                if output_error:
                    os.close(output_fd)
                    output_fd = None
            if output_error:
                # Artifact only: never report a failed output through itself.
                # Emit once; all subsequent console records are suppressed.
                if log:
                    try:
                        log.write(json.dumps({"status": "error", "detail": f"fault stdout disabled; console records lost; artifact capture continues: {output_error}"}) + "\n")
                        log.flush()
                    except OSError:
                        log = None
                output_error = None

    if log_error:
        publish({"status": "error", "detail": f"fault log unavailable: {log_error}"})

    # The pipe gates exec, not browser readiness. All probes attach before the
    # unchanged command runs with the caller's uid, environment and arguments.
    read_fd, write_fd = os.pipe()
    gate = "import os,sys; os.read(int(sys.argv[1]),1); os.close(int(sys.argv[1])); os.execvp(sys.argv[2],sys.argv[2:])"
    child = subprocess.Popen([sys.executable, "-c", gate, str(read_fd), *command], pass_fds=(read_fd,))
    os.close(read_fd)
    ready = threading.Event()
    condition = threading.Condition()
    attached = False
    stopped = False
    tracer = None
    readers = []
    pending = {}
    # Faults a snapshot found absent whose COMPLETED record is still in flight:
    # the snapshot reads the map when handled, the printf crosses per-CPU buffers.
    resolved = {}
    longest = []
    reports = 0
    snapshots = 0
    diagnostics = 0
    active_limit = False
    history_complete = True
    final_verified = False
    snapshot = None
    snapshot_integrity = None
    snapshot_stacks = {}
    snapshot_final = False
    snapshot_valid = False
    kernel_error = False

    def invalidate(detail, kernel=False):
        nonlocal history_complete, kernel_error, diagnostics
        history_complete = False
        kernel_error |= kernel
        if diagnostics >= 16:
            return
        diagnostics += 1
        publish({"status": "error", "detail": detail, "history_complete": False})

    def retain(data):
        nonlocal active_limit
        header, basename, stack = data.split("\n", 2)
        fields = header.split()
        status = fields[0].lower()
        pid, tid, address, offset, entry, start, end, map_offset, inode, device = map(int, fields[1:11])
        event = {"status": status, "pid": pid, "tid": tid, "address": address, "offset": offset, "entry_ns": entry, "vma_start": start, "vma_end": end, "vma_offset": map_offset, "inode": inode, "device": device, "file_basename": basename}
        key = (tid, entry)
        if status == "pending":
            event["stack"] = stack[:8192]
            try:
                # Resolve only the faulted data address. Kernel VMA, inode,
                # device and basename survive even an immediate unmap/exit.
                mapping = "mapping no longer present"
                with open(f"/proc/{pid}/maps") as maps:
                    for row in maps:
                        bounds = row.split()[0].split("-")
                        if int(bounds[0], 16) <= address < int(bounds[1], 16):
                            mapping = row.strip()[:4096]
                            break
                event["mapping"] = mapping
            except OSError as error:
                event["mapping"] = f"unavailable: {error}"
            with condition:
                if key in pending or len(pending) < 1024:
                    if key not in pending:
                        resolved.pop(key, None)
                        pending[key] = event
                    condition.notify_all()
                elif not active_limit:
                    active_limit = True
                    invalidate("1024 simultaneous pending fault limit reached", kernel=True)
        else:
            event["duration_ns"] = int(fields[11])
            event["return"] = int(fields[12])
            with condition:
                prior = pending.pop(key, None) or resolved.pop(key, None)
                if prior:
                    event.update(mapping=prior["mapping"], stack=prior["stack"])
                    if prior.pop("reported", False):
                        publish(event)
                item = (event["duration_ns"], entry, tid, event)
                if len(longest) < 128:
                    heapq.heappush(longest, item)
                else:
                    heapq.heappushpop(longest, item)
                condition.notify_all()

    def reconcile(values, stacks, final):
        nonlocal final_verified, snapshots
        keys = set()
        if len(values) > 1025:
            invalidate("kernel snapshot exceeds 1024 retained pending faults", kernel=True)
        for tid_text, fields in list(values.items())[:1025]:
            tid = int(tid_text)
            if tid == 0:
                continue
            pid, address, offset, entry, start, end, map_offset, inode, device, basename = fields
            keys.add((tid, entry))
            stack = stacks.get(f"{tid},{entry}")
            if stack is None:
                # A live traversal races completion. Never join another fault's
                # stack; the next snapshot recovers a fault that remains held.
                if final:
                    invalidate("final live fault missing its kernel wait stack", kernel=True)
                continue
            retain(f"PENDING {pid} {tid} {address} {offset} {entry} {start} {end} {map_offset} {inode} {device}\n{basename}\n{stack}")
        with condition:
            for key in list(pending):
                if key not in keys:
                    event = pending.pop(key)
                    reported = event.get("reported", False)
                    alive = Path(f"/proc/{key[0]}").exists()
                    if alive:
                        resolved[key] = event
                        if len(resolved) > 1024:
                            del resolved[next(iter(resolved))]
                    # A vanished task independently establishes termination;
                    # otherwise the kernel map establishes only absence now.
                    event = dict(event, status="resolved" if alive else "terminated", resolution="absent from kernel live-fault map")
                    event.pop("reported", None)
                    event.pop("quiet", None)
                    if reported:
                        publish(event)
            final_verified = final and not kernel_error
            if snapshots < 256:
                publish({"status": "live_snapshot", "observed_ns": time.monotonic_ns(), "pending": sorted(keys), "kernel_integrity": not kernel_error})
                snapshots += 1
            condition.notify_all()

    def consume():
        nonlocal attached, diagnostics, snapshot, snapshot_stacks, snapshot_integrity, snapshot_final, snapshot_valid
        try:
            for raw in tracer.stdout:
                if not raw.strip():
                    continue
                message = json.loads(raw)
                if message.get("type") == "attached_probes":
                    continue
                if message.get("type") == "map":
                    if snapshot is not None and set(message["data"]) == {"@live"}:
                        snapshot = message["data"]["@live"]
                    elif snapshot is not None and set(message["data"]) == {"@stack"}:
                        snapshot_stacks = message["data"]["@stack"]
                    elif snapshot is not None and set(message["data"]) == {"@integrity"}:
                        snapshot_integrity = message["data"]["@integrity"]
                    else:
                        invalidate(f"unexpected kernel snapshot: {str(message)[:2048]}")
                    continue
                if message.get("type") != "printf":
                    snapshot_valid = False
                    invalidate(str(message)[:2048], kernel=message.get("type") != "lost_events")
                    continue
                data = message["data"]
                if data == "READY\n":
                    attached = True
                    publish({"status": "attached", "root_pid": child.pid, "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "clock": "kernel monotonic ns", "monotonic_ns": time.monotonic_ns(), "page_size": os.sysconf("SC_PAGE_SIZE")})
                    ready.set()
                elif data in ("SNAPSHOT\n", "FINAL\n"):
                    snapshot = {}
                    snapshot_stacks = {}
                    snapshot_integrity = None
                    snapshot_final = data == "FINAL\n"
                    snapshot_valid = True
                elif data in ("SNAPSHOT_END\n", "FINAL_END\n"):
                    if snapshot is not None and "0" in snapshot and snapshot_integrity is not None and snapshot_valid and snapshot_final == (data == "FINAL_END\n"):
                        if snapshot_integrity:
                            invalidate("kernel fault tracking map capacity exceeded", kernel=True)
                        reconcile(snapshot, snapshot_stacks, snapshot_final)
                    else:
                        invalidate("incomplete kernel live-fault snapshot")
                    snapshot = None
                elif data.startswith(("PENDING ", "COMPLETED ")):
                    retain(data)
                else:
                    invalidate(data[:2048], kernel=True)
        except (ValueError, KeyError, IndexError, TypeError, OSError) as error:
            invalidate(f"tracer stream: {error}", kernel=True)
        finally:
            if not attached:
                invalidate("fault observer did not attach: permission, BTF or kernel probe support failure", kernel=True)
            elif child.poll() is None:
                invalidate("fault tracer stopped while the command was still running", kernel=True)
            ready.set()

    def stderr():
        nonlocal kernel_error, history_complete
        for number, raw in enumerate(tracer.stderr):
            if any(word in raw for word in ("ERROR", "error:", "Lost", "WARNING")):
                # Even beyond the diagnostic cap, failure invalidates evidence.
                kernel_error = True
                history_complete = False
            if number < 16:
                publish({"status": "error" if kernel_error else "diagnostic", "detail": raw[:2048].strip()})

    def announce():
        nonlocal reports
        with condition:
            while not stopped:
                due = [event for event in pending.values() if not event.get("reported") and not event.get("quiet")]
                now = time.monotonic_ns()
                for event in due:
                    if now - event["entry_ns"] >= 100_000_000:
                        if reports < 256:
                            publish(event)
                            event["reported"] = True
                            reports += 1
                        elif reports == 256:
                            publish({"status": "error", "detail": "live report cap reached; active faults and longest completions remain retained"})
                            reports += 1
                        if reports > 256:
                            event["quiet"] = True
                deadline = min((event["entry_ns"] + 100_000_000 for event in due), default=now + 1_000_000_000)
                # This timer reduces reporting noise; tests coordinate on the
                # report acknowledgement, never a sleep or a Chrome delay.
                condition.wait(max(0.001, (deadline - time.monotonic_ns()) / 1e9))

    def interrupted(signum, _frame):
        # Cancellation targets this run's command, never a host browser.
        if child.poll() is None:
            child.send_signal(signum)

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    result = 1
    try:
        # Keep the pre-0.23 string bound: 1024-byte strings overflow the probe's
        # tuple/printf allocations on newer bpftrace before any probe attaches.
        tracer = subprocess.Popen(["sudo", "-n", "--", "env", "BPFTRACE_MAX_MAP_KEYS=4096", "BPFTRACE_PERF_RB_PAGES=64", "BPFTRACE_MAX_STRLEN=64", "timeout", "--signal=INT", "--kill-after=5s", "15m", "bpftrace", "-k", "-f", "json", "-B", "line", str(HERE / "faults.bt"), str(child.pid), str(os.sysconf("SC_PAGE_SIZE"))], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        readers = [threading.Thread(target=target, daemon=True) for target in (consume, stderr, announce)]
        for reader in readers:
            reader.start()
        if not ready.wait(45):
            publish({"status": "error", "detail": "fault observer attachment exceeded 45 seconds"})
        os.write(write_fd, b"G")
        os.close(write_fd)
        write_fd = None
        result = child.wait()
    except OSError as error:
        publish({"status": "error", "detail": f"fault observer launch: {error}"})
        if write_fd is not None:
            os.write(write_fd, b"G")
            os.close(write_fd)
            write_fd = None
        result = child.wait()
    finally:
        if write_fd is not None:
            os.close(write_fd)
        if tracer and tracer.poll() is None:
            # sudo forwards to the timeout supervisor, which owns only this
            # tracer. Its 15-minute watchdog also bounds a lost control channel.
            try:
                subprocess.run(["sudo", "-n", "kill", "-INT", str(tracer.pid)], timeout=5, check=False)
                tracer.wait(timeout=10)
            except (OSError, subprocess.TimeoutExpired) as error:
                publish({"status": "error", "detail": f"tracer cleanup: {error}; its watchdog remains bounded to 15 minutes"})
        with condition:
            stopped = True
            condition.notify_all()
        for reader in readers:
            reader.join(timeout=5)
        if tracer:
            if all(not reader.is_alive() for reader in readers):
                tracer.stdout.close()
                tracer.stderr.close()
            if attached and tracer.returncode != 0:
                invalidate(f"fault observer exited {tracer.returncode}", kernel=True)
        if attached and not final_verified:
            invalidate("no verified final kernel live-fault snapshot", kernel=True)
        with condition:
            retained_pending = list(pending.values())
            retained_completed = list(longest)
        verified_pending = []
        unverified_pending = 0
        for event in retained_pending:
            if not Path(f"/proc/{event['tid']}").exists():
                publish(dict(event, status="terminated", resolution="task no longer exists"))
                continue
            if final_verified and not kernel_error:
                verified_pending.append(event)
            else:
                unverified_pending += 1
                event["status"] = "unverified"
            event.pop("reported", None)
            event.pop("quiet", None)
            publish(event)
        for _, _, _, event in sorted(retained_completed, reverse=True):
            publish(event)
        publish({"status": "finished", "command_status": result, "still_pending": len(verified_pending), "unverified_pending": unverified_pending, "live_state_verified": final_verified and not kernel_error, "history_complete": history_complete, "completed_retained": len(retained_completed)})
        closing_error = None
        with output_lock:
            if log:
                try:
                    log.close()
                except OSError as error:
                    closing_error = str(error)
                log = None
        if closing_error:
            publish({"status": "error", "detail": f"fault log close: {closing_error}"})
        if output_fd is not None:
            os.close(output_fd)
    if result < 0:
        # SIGKILL cannot have a handler; every terminating signal still has
        # to reach the caller as a signal rather than a fabricated exit code.
        if -result != signal.SIGKILL:
            signal.signal(-result, signal.SIG_DFL)
        os.kill(os.getpid(), -result)
    return result


if __name__ == "__main__":
    sys.exit(main())
