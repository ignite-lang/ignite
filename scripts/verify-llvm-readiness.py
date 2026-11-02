#!/usr/bin/env python3
"""Exercise frozen LLVM run/replay without changing benchmark source files."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import signal
import socket
import subprocess
import threading
import time

MS = 1_000_000
SUPERVISOR = "Terminal.server.supervisor"


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def fields(value):
    return value.get("fields", {}) if isinstance(value, dict) else {}


def event(record):
    return fields(record.get("value", {})).get("kind", {}) if record.get("kind") == "event" else {}


def event_is(record, variant):
    return event(record).get("variant") == variant and record.get("machine") == SUPERVISOR


def effect_is(record, variant):
    return record.get("kind") == "effect" and record.get("value", {}).get("variant") == variant


def at(record):
    return record["at"]["ns"]


def standing(record, contract="Terminal.reachable"):
    return record.get("kind") == "standing" and fields(record.get("value", {})).get("contract") == contract


def received(record, value):
    return (record.get("kind") == "signal" and record.get("value", {}).get("ref") == "Terminal.promptProbe.attempts"
            and fields(record["value"].get("value", {})).get("received") is value)


def source_check(repo, manifest):
    for name, expected in manifest.items():
        file = repo / name
        require(file.is_file() and hashlib.sha256(file.read_bytes()).hexdigest() == expected,
                "Source manifest mismatch: " + name)


class Run:
    def __init__(self, args, name, clock, epoch=None):
        self.args = args
        self.name = name
        self.state = args.work / "states" / name
        require(not self.state.exists(), "Refusing to reuse existing state: " + str(self.state))
        self.seen, self.lines, self.errors = [], [], []
        self.queue = queue.Queue()
        self.stopped = set()
        command = [str(args.cli), "run", str(args.repo / "examples/terminal/terminal.design.ignite"),
                   "--assembly", "local", "--state", str(self.state), "--build", str(args.work / "build"),
                   "--target", "llvm", "--runtime", str(args.runtime), "--json", "--settle", "1 s",
                   "--port", "Terminal.ingress=0", "--clock", clock]
        if epoch:
            command += ["--epoch", epoch]
        self.command = command
        self.proc = subprocess.Popen(command, cwd=args.repo, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.out_thread = threading.Thread(target=self.read_stdout, daemon=True)
        self.err_thread = threading.Thread(target=self.read_stderr, daemon=True)
        self.out_thread.start()
        self.err_thread.start()

    def read_stdout(self):
        try:
            for line in self.proc.stdout:
                self.lines.append(line)
                try:
                    self.queue.put(json.loads(line))
                except ValueError as error:
                    self.queue.put(RuntimeError(f"Invalid --json record: {line!r}: {error}"))
                    return
        except Exception as error:
            self.queue.put(error)
        finally:
            self.queue.put(None)

    def read_stderr(self):
        try:
            for line in self.proc.stderr:
                self.errors.append(line)
        except Exception as error:
            self.queue.put(error)

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.cleanup()

    def send(self, text):
        require(self.proc.poll() is None, "Controller exited before command: " + text)
        self.proc.stdin.write(text + "\n")
        self.proc.stdin.flush()

    def wait(self, what, predicate):
        deadline = time.monotonic() + self.args.timeout
        while time.monotonic() < deadline:
            try:
                value = self.queue.get(timeout=max(0.001, deadline - time.monotonic()))
            except queue.Empty:
                break
            if value is None:
                raise AssertionError("Controller ended waiting for " + what + "\n" + "".join(self.errors))
            if isinstance(value, Exception):
                raise value
            self.seen.append(value)
            if predicate(value):
                return value
        raise AssertionError("Timed out waiting for " + what + "\n" + "".join(self.errors))

    def event(self, variant):
        return self.wait("supervisor " + variant, lambda row: event_is(row, variant))

    def status(self):
        self.send("status")
        return self.wait("status", lambda row: row.get("kind") == "status")

    def pids(self):
        return [event(row)["fields"][0] for row in self.seen if event_is(row, "Started")]

    def own_executable(self, pid):
        try:
            return Path(f"/proc/{pid}/exe").resolve().is_relative_to(self.state / "bin")
        except (OSError, ValueError):
            return False

    def signal(self, pid, sig):
        require(self.own_executable(pid), "Refusing to signal a non-owned process: " + str(pid))
        os.kill(pid, sig)
        if sig == signal.SIGSTOP:
            self.stopped.add(pid)
        if sig == signal.SIGCONT:
            self.stopped.discard(pid)

    def owned_pids(self):
        result = set(self.pids())
        for entry in Path("/proc").iterdir():
            if entry.name.isdigit() and self.own_executable(int(entry.name)):
                result.add(int(entry.name))
        return result

    def no_live_processes(self):
        deadline = time.monotonic() + 2
        while True:
            live = []
            for pid in self.owned_pids():
                if not self.own_executable(pid):
                    continue
                path = Path(f"/proc/{pid}/status")
                if not path.exists():
                    continue
                try:
                    state = next((line for line in path.read_text().splitlines() if line.startswith("State:")), "")
                except FileNotFoundError:
                    continue
                if "Z" not in state:
                    live.append(pid)
            if not live:
                return
            if time.monotonic() >= deadline:
                raise AssertionError("Processes remain after stop: " + str(live))
            time.sleep(0.02)

    def stop_and_replay(self):
        self.send("stop")
        code = self.proc.wait(timeout=self.args.timeout)
        self.out_thread.join(timeout=2)
        self.err_thread.join(timeout=2)
        require(not self.out_thread.is_alive() and not self.err_thread.is_alive(), "Controller output readers did not finish")
        while True:
            try:
                pending = self.queue.get_nowait()
            except queue.Empty:
                break
            if isinstance(pending, Exception):
                raise pending
        require(code == 0, f"stop exited {code}\n" + "".join(self.errors))
        self.no_live_processes()
        command = [str(self.args.cli), "replay", str(self.args.repo / "examples/terminal/terminal.design.ignite"),
                   "--assembly", "local", "--state", str(self.state)]
        replay = subprocess.run(command, cwd=self.args.repo, text=True, capture_output=True, timeout=self.args.timeout)
        (self.args.work / (self.name + "-replay.stdout")).write_text(replay.stdout)
        (self.args.work / (self.name + "-replay.stderr")).write_text(replay.stderr)
        require(replay.returncode == 0, f"replay exited {replay.returncode}: {replay.stderr}")
        return replay.stdout

    def cleanup(self):
        for pid in list(self.stopped):
            if self.own_executable(pid):
                try:
                    os.kill(pid, signal.SIGCONT)
                except ProcessLookupError:
                    pass
        if self.proc.poll() is None:
            try:
                self.send("stop")
                self.proc.wait(timeout=3)
            except (OSError, AssertionError, subprocess.TimeoutExpired):
                for pid in self.owned_pids():
                    if self.own_executable(pid):
                        try:
                            os.kill(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                self.proc.kill()
                self.proc.wait(timeout=3)
        self.out_thread.join(timeout=2)
        self.err_thread.join(timeout=2)
        (self.args.work / (self.name + "-stdout.jsonl")).write_text("".join(self.lines))
        (self.args.work / (self.name + "-stderr.log")).write_text("".join(self.errors))
        (self.args.work / (self.name + "-command.json")).write_text(json.dumps(self.command, indent=2) + "\n")


def echo(port):
    with socket.create_connection(("127.0.0.1", port), timeout=5) as connection:
        connection.settimeout(5)
        seen = b""
        while not seen.endswith(b"terminal> "):
            chunk = connection.recv(1024)
            require(chunk, "EOF before terminal prompt")
            seen += chunk
        connection.sendall(b"echo hello\r\n")
        seen = b""
        while b"hello" not in seen:
            chunk = connection.recv(1024)
            require(chunk, "EOF before echo reply")
            seen += chunk
        connection.sendall(b"quit\r\n")


def manual(args):
    with Run(args, "manual-restart", "manual") as run:
        run.event("Ready")
        port = run.status().get("ports", {}).get("Terminal.ingress")
        require(isinstance(port, int) and 0 < port < 65536 and port != 2323, "Missing isolated ingress port")
        echo(port)
        pid = run.pids()[0]
        run.signal(pid, signal.SIGKILL)
        ended = run.wait("supervisor exit", lambda row: event_is(row, "Exited") or event_is(row, "Signalled"))
        require(event(ended)["fields"][0] == pid, "Exited pid differs")
        run.wait("restart attempt 1", lambda row: effect_is(row, "Note") and "restart attempt 1 delay 100 ms" in row["value"]["fields"][0])
        timer = run.wait("restart timer", lambda row: effect_is(row, "SetTimer") and row.get("machine") == SUPERVISOR)
        require(timer["value"]["fields"][1]["ns"] == at(ended) + 100 * MS, "Incorrect backoff timer")
        run.send("advance 99 ms")
        before = len(run.seen)
        run.status()
        require(not any(event_is(row, "Started") for row in run.seen[before:]), "Restart before 100 ms")
        run.send("advance 1 ms")
        started = run.event("Started")
        require(at(started) - at(ended) == 100 * MS, "Manual restart differs from exact 100 ms")
        run.event("Ready")
        run.signal(run.pids()[-1], signal.SIGKILL)
        run.wait("restart attempt 2", lambda row: effect_is(row, "Note") and "restart attempt 2 delay 200 ms" in row["value"]["fields"][0])
        run.stop_and_replay()
        return dict(manual_restart_ns=at(started) - at(ended), second_backoff_ms=200, echo=True, stopped=True, replay=True)


def real(args):
    with Run(args, "real-restart", "real") as run:
        run.event("Ready")
        pid = run.pids()[0]
        run.signal(pid, signal.SIGKILL)
        ended = run.wait("supervisor exit", lambda row: event_is(row, "Exited") or event_is(row, "Signalled"))
        started = run.event("Started")
        delay = at(started) - at(ended)
        require(100 * MS <= delay <= 500 * MS, f"Real restart {delay} ns outside 100–500 ms")
        run.event("Ready")
        run.stop_and_replay()
        return dict(real_restart_ns=delay, stopped=True, replay=True)


def probe(args):
    with Run(args, "probe-timeout", "manual", "2026-10-31T22:59:30Z") as run:
        run.event("Ready")
        first = run.wait("first reachable standing", standing)
        require(fields(first["value"])["good"] == 1 and fields(first["value"])["executed"] == 1, "Initial attempt is not good")
        pid = run.pids()[0]
        run.signal(pid, signal.SIGSTOP)
        run.send("advance 10 seconds")
        attempt = run.wait("probe attempt", lambda row: effect_is(row, "Attempt"))
        require(at(attempt) == 10_000 * MS, "Probe not dispatched at 10 seconds")
        run.send("advance 2 seconds")
        bad = run.wait("bad attempt signal", lambda row: received(row, False))
        require(at(bad) in (10_000 * MS, 12_000 * MS), "Unexpected timeout observation instant")
        violated = run.wait("violated reachable standing", standing)
        value = fields(violated["value"])
        require(value["status"]["variant"] == "Violated" and value["good"] == 1 and value["executed"] == 2, "Incorrect violated standing")
        timeout_seen = any((effect_is(row, "Note") and row["value"]["fields"][0] == "probe timeout") or
                           (event(row).get("variant") == "AttemptDone" and fields(event(row).get("fields", [None, {}])[1]).get("received") is False)
                           for row in run.seen)
        require(timeout_seen, "Missing timeout/bad AttemptDone evidence")
        status = run.status()
        current = next(s for s in status["standings"] if fields(s).get("contract") == "Terminal.reachable")
        require(current == violated["value"], "Status and journal standing differ")
        run.signal(pid, signal.SIGCONT)
        run.send("advance 10 seconds")
        good = run.wait("recovered probe signal", lambda row: received(row, True) and at(row) == 20_000 * MS)
        still = run.wait("retained rolling violation", lambda row: standing(row) and at(row) == 20_000 * MS)
        require(fields(still["value"])["status"]["variant"] == "Violated" and fields(still["value"])["executed"] == 3,
                "Rolling window forgot failed attempt")
        run.send("advance 10 seconds")
        closed = run.wait("closed calendar period", lambda row: standing(row, "Terminal.customerAccess") and at(row) == 30_000 * MS)
        require(fields(closed["value"])["status"]["variant"] == "Violated", "Closed period is not violated")
        consequence = run.wait("breach consequence", lambda row: row.get("kind") == "consequence")
        fired = fields(consequence["value"])
        require(fired["consequence"] == "access_commitment_breached" and fired["standing_seq"] == closed["seq"],
                "Consequence differs from closed standing")
        replay = run.stop_and_replay()
        journal = [json.loads(line) for line in (run.state / "journal.jsonl").read_text().splitlines()]
        require(sum(row.get("kind") == "consequence" for row in journal) == 1, "Consequence duplicated")
        require("Terminal.reachable" in replay, "Replay lacks reachable standing")
        return dict(timeout_at_ns=at(bad), violated_good=1, violated_executed=2, recovered_signal_at_ns=at(good),
                    rolling_violation_retained=True, consequence_once=True, stopped=True, replay=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--source-manifest", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=15)
    args = parser.parse_args()
    for field in ("cli", "repo", "runtime", "work", "source_manifest"):
        setattr(args, field, getattr(args, field).resolve())
    require(not args.work.is_relative_to(args.repo), "Verifier work must be outside frozen source")
    require(not (args.work / "report.json").exists(), "Refusing to overwrite an existing verifier report")
    args.work.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(args.source_manifest.read_text())
    source_check(args.repo, manifest)
    report = dict(candidate=args.candidate, target="llvm", cli=str(args.cli),
                  cli_sha256=hashlib.sha256(args.cli.read_bytes()).hexdigest(), runtime=str(args.runtime),
                  runtime_sha256=hashlib.sha256(args.runtime.read_bytes()).hexdigest(),
                  source_manifest_sha256=hashlib.sha256(args.source_manifest.read_bytes()).hexdigest(),
                  scenarios={}, accepted=False)
    try:
        for name, function in [("manual", manual), ("real", real), ("probe", probe)]:
            report["scenarios"][name] = function(args)
            print(json.dumps({name: report["scenarios"][name]}), flush=True)
        source_check(args.repo, manifest)
        report["accepted"] = True
    except Exception as error:
        report["error"] = repr(error)
        raise
    finally:
        (args.work / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
