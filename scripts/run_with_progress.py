#!/usr/bin/env python3
"""Run a child command with visible progress and timeout context."""

from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import TextIO


def quoted(value: object) -> str:
    return shlex.quote(str(value))


def timeout_label(timeout_seconds: float) -> str:
    if timeout_seconds <= 0:
        return "disabled"
    if timeout_seconds.is_integer():
        return f"{int(timeout_seconds)}s"
    return f"{timeout_seconds:.3f}s"


def event_line(
    state: str,
    *,
    phase: str,
    target: str,
    pid: int,
    command: list[str],
    elapsed_seconds: float,
    timeout_seconds: float,
    status: int | None = None,
    reason: str | None = None,
) -> str:
    fields = [
        "gate-progress",
        f"state={quoted(state)}",
        f"phase={quoted(phase)}",
        f"target={quoted(target)}",
        f"pid={pid}",
        f"elapsed={elapsed_seconds:.1f}s",
        f"timeout={quoted(timeout_label(timeout_seconds))}",
        f"command={quoted(shlex.join(command))}",
    ]
    if status is not None:
        fields.append(f"status={status}")
    if reason is not None:
        fields.append(f"reason={quoted(reason)}")
    return "[" + " ".join(fields) + "]"


def write_line(line: str, *, log: TextIO | None) -> None:
    print(line, file=sys.stderr, flush=True)
    if log is not None:
        print(line, file=log, flush=True)


def stream_pipe(source: TextIO, sink: TextIO, log: TextIO | None, lock: threading.Lock) -> None:
    for line in source:
        with lock:
            sink.write(line)
            sink.flush()
            if log is not None:
                log.write(line)
                log.flush()


def run_command(args: argparse.Namespace) -> int:
    if not args.command:
        print("run_with_progress: missing command after --", file=sys.stderr)
        return 2

    log: TextIO | None = None
    if args.log is not None:
        args.log.parent.mkdir(parents=True, exist_ok=True)
        log = args.log.open("a", encoding="utf-8")

    started = time.monotonic()
    process = subprocess.Popen(
        args.command,
        cwd=args.cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    assert process.stdout is not None
    assert process.stderr is not None

    lock = threading.Lock()
    stdout_thread = threading.Thread(
        target=stream_pipe,
        args=(process.stdout, sys.stdout, log, lock),
        daemon=True,
    )
    stderr_thread = threading.Thread(
        target=stream_pipe,
        args=(process.stderr, sys.stderr, log, lock),
        daemon=True,
    )
    stdout_thread.start()
    stderr_thread.start()

    write_line(
        event_line(
            "started",
            phase=args.phase,
            target=args.target,
            pid=process.pid,
            command=args.command,
            elapsed_seconds=0.0,
            timeout_seconds=args.timeout_seconds,
        ),
        log=log,
    )

    next_progress = args.progress_interval_seconds
    timed_out = False
    try:
        while True:
            status = process.poll()
            elapsed = time.monotonic() - started
            if status is not None:
                break
            if args.timeout_seconds > 0 and elapsed >= args.timeout_seconds:
                timed_out = True
                write_line(
                    event_line(
                        "timeout",
                        phase=args.phase,
                        target=args.target,
                        pid=process.pid,
                        command=args.command,
                        elapsed_seconds=elapsed,
                        timeout_seconds=args.timeout_seconds,
                        reason="timeout",
                    ),
                    log=log,
                )
                process.terminate()
                try:
                    status = process.wait(timeout=args.kill_grace_seconds)
                except subprocess.TimeoutExpired:
                    write_line(
                        event_line(
                            "killing",
                            phase=args.phase,
                            target=args.target,
                            pid=process.pid,
                            command=args.command,
                            elapsed_seconds=time.monotonic() - started,
                            timeout_seconds=args.timeout_seconds,
                            reason="timeout-kill-grace-exceeded",
                        ),
                        log=log,
                    )
                    process.kill()
                    status = process.wait()
                break
            if elapsed >= next_progress:
                write_line(
                    event_line(
                        "running",
                        phase=args.phase,
                        target=args.target,
                        pid=process.pid,
                        command=args.command,
                        elapsed_seconds=elapsed,
                        timeout_seconds=args.timeout_seconds,
                    ),
                    log=log,
                )
                next_progress += args.progress_interval_seconds
            time.sleep(0.1)

        stdout_thread.join(timeout=2)
        stderr_thread.join(timeout=2)
        elapsed = time.monotonic() - started
        write_line(
            event_line(
                "finished",
                phase=args.phase,
                target=args.target,
                pid=process.pid,
                command=args.command,
                elapsed_seconds=elapsed,
                timeout_seconds=args.timeout_seconds,
                status=int(status),
                reason="timeout" if timed_out else None,
            ),
            log=log,
        )
        return int(status)
    finally:
        if log is not None:
            log.close()


def run_self_test() -> int:
    command = ["cargo", "test", "-p", "trust-runtime"]
    started = event_line(
        "started",
        phase="phase-a",
        target="target-a",
        pid=123,
        command=command,
        elapsed_seconds=0,
        timeout_seconds=60,
    )
    running = event_line(
        "running",
        phase="phase-a",
        target="target-a",
        pid=123,
        command=command,
        elapsed_seconds=30,
        timeout_seconds=60,
    )
    timed_out = event_line(
        "timeout",
        phase="phase-a",
        target="target-a",
        pid=123,
        command=command,
        elapsed_seconds=61,
        timeout_seconds=60,
        reason="timeout",
    )
    required = [
        "state=started",
        "phase=phase-a",
        "target=target-a",
        "pid=123",
        "elapsed=0.0s",
        "timeout=60s",
        "command='cargo test -p trust-runtime'",
    ]
    errors: list[str] = []
    for needle in required:
        if needle not in started:
            errors.append(f"started line missing {needle}")
    if "state=running" not in running or "elapsed=30.0s" not in running:
        errors.append("running line missing elapsed progress")
    if "state=timeout" not in timed_out or "reason=timeout" not in timed_out:
        errors.append("timeout line missing timeout reason")

    if errors:
        print("run_with_progress self-test: failed")
        for error in errors:
            print(f"- {error}")
        return 1
    print("run_with_progress self-test: ok")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase", default="unspecified")
    parser.add_argument("--target", default="unspecified")
    parser.add_argument("--timeout-seconds", type=float, default=0)
    parser.add_argument("--progress-interval-seconds", type=float, default=30)
    parser.add_argument("--kill-grace-seconds", type=float, default=5)
    parser.add_argument("--cwd", type=Path, default=None)
    parser.add_argument("--log", type=Path, default=None)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.self_test:
        return args
    if args.command and args.command[0] == "--":
        args.command = args.command[1:]
    if args.progress_interval_seconds <= 0:
        parser.error("--progress-interval-seconds must be > 0")
    if args.kill_grace_seconds <= 0:
        parser.error("--kill-grace-seconds must be > 0")
    if args.cwd is not None:
        args.cwd = args.cwd.resolve()
    return args


def main() -> int:
    args = parse_args()
    if args.self_test:
        return run_self_test()
    return run_command(args)


if __name__ == "__main__":
    raise SystemExit(main())
