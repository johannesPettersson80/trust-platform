#!/usr/bin/env python3
"""Scaling benchmark for trust-lsp pull diagnostics on multi-file projects.

Generates synthetic projects with N files (function blocks, functions, a struct type and
a global variable list per file, each file calling into the previous one) and measures
the time until every file's textual diagnostics have been pulled, as an editor or CI
tool that checks a whole project would do.

    python3 scripts/perf/project_scaling.py --lsp target/release/trust-lsp 25 50 100 200
    python3 scripts/perf/project_scaling.py --lsp target/release/trust-lsp --edit 50 100 200

Per-file time that grows with N means per-file work is proportional to the project size.
--edit measures the editor case instead: open one file, change it, pull its diagnostics.
"""
import argparse, json, os, subprocess, sys, tempfile, threading, time


def gen_project(root, n):
    src = os.path.join(root, "src")
    os.makedirs(src, exist_ok=True)
    with open(os.path.join(root, "trust-lsp.toml"), "w") as f:
        f.write('[project]\ninclude_paths = ["src"]\nstdlib = "iec"\n')
    for i in range(n):
        prev = (i - 1) % n
        text = f"""TYPE S{i} : STRUCT
    a : INT;
    b : REAL;
    c : ARRAY[0..3] OF DINT;
END_STRUCT END_TYPE

VAR_GLOBAL
    g{i}_count : INT;
    g{i}_state : S{i};
END_VAR

FUNCTION F{i} : INT
VAR_INPUT
    x : INT;
    y : INT;
END_VAR
F{i} := x + y + g{i}_count;
END_FUNCTION

FUNCTION_BLOCK FB{i}
VAR_INPUT
    enable : BOOL;
    value : INT;
END_VAR
VAR_OUTPUT
    out : INT;
    done : BOOL;
END_VAR
VAR
    t : TON;
    s : S{i};
    other : FB{prev};
END_VAR
t(IN := enable, PT := T#100ms);
IF t.Q THEN
    s.a := F{i}(x := value, y := F{prev}(x := 1, y := 2));
    out := s.a + g{prev}_count;
    other(enable := FALSE, value := out);
END_IF;
done := t.Q;
END_FUNCTION_BLOCK
"""
        with open(os.path.join(src, f"Unit{i:04d}.st"), "w") as f:
            f.write(text)
    with open(os.path.join(src, "Main.st"), "w") as f:
        f.write("PROGRAM Main\nVAR\n" + "".join(f"    u{i} : FB{i};\n" for i in range(n)) +
                "END_VAR\n" + "".join(f"u{i}(enable := TRUE, value := {i});\n" for i in range(n)) +
                "END_PROGRAM\n")


class Client:
    def __init__(self, lsp):
        self.p = subprocess.Popen([lsp], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.resp = {}
        self.cv = threading.Condition()
        threading.Thread(target=self.reader, daemon=True).start()

    def send(self, m):
        b = json.dumps(m).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
        self.p.stdin.flush()

    def reader(self):
        out = self.p.stdout
        while True:
            h = b""
            while not h.endswith(b"\r\n\r\n"):
                c = out.read(1)
                if not c:
                    return
                h += c
            n = int([l for l in h.split(b"\r\n") if l.lower().startswith(b"content-length")][0].split(b":")[1])
            m = json.loads(out.read(n))
            if "id" in m and "method" in m:
                self.send({"jsonrpc": "2.0", "id": m["id"], "result": None})
            elif "id" in m:
                with self.cv:
                    self.resp[m["id"]] = m
                    self.cv.notify_all()

    def call(self, i, method, params):
        self.send({"jsonrpc": "2.0", "id": i, "method": method, "params": params})
        with self.cv:
            while i not in self.resp:
                self.cv.wait()
            return self.resp.pop(i)

    def close(self):
        self.p.kill()


def measure(lsp, n):
    with tempfile.TemporaryDirectory() as root:
        gen_project(root, n)
        files = sorted(os.path.join(root, "src", f) for f in os.listdir(os.path.join(root, "src")))
        c = Client(lsp)
        uri = "file://" + root
        c.call(1, "initialize", {"processId": None, "rootUri": uri, "capabilities": {"textDocument": {"diagnostic": {}}},
                                 "workspaceFolders": [{"uri": uri, "name": "bench"}]})
        c.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
        t0 = time.perf_counter()
        first = None
        diags = 0
        for k, f in enumerate(files):
            r = c.call(10 + k, "textDocument/diagnostic", {"textDocument": {"uri": "file://" + f}})
            diags += len((r.get("result") or {}).get("items", []))
            if first is None:
                first = time.perf_counter() - t0
        total = time.perf_counter() - t0
        c.close()
        return len(files), first, total, diags


def measure_edits(lsp, n, edits=10):
    """Editor scenario: open one file, then repeatedly change it and pull its
    diagnostics (what a client does after each keystroke batch). Returns the time to the
    first diagnostics after opening and the median latency after a change."""
    with tempfile.TemporaryDirectory() as root:
        gen_project(root, n)
        target = os.path.join(root, "src", "Unit0000.st")
        with open(target) as f:
            text = f.read()
        uri = "file://" + target
        c = Client(lsp)
        root_uri = "file://" + root
        c.call(1, "initialize", {"processId": None, "rootUri": root_uri, "capabilities": {"textDocument": {"diagnostic": {}}},
                                 "workspaceFolders": [{"uri": root_uri, "name": "bench"}]})
        c.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
        c.send({"jsonrpc": "2.0", "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": uri, "languageId": "st", "version": 1, "text": text}}})
        t0 = time.perf_counter()
        c.call(2, "textDocument/diagnostic", {"textDocument": {"uri": uri}})
        open_latency = time.perf_counter() - t0
        latencies = []
        for k in range(edits):
            changed = text.replace("out := s.a + ", f"out := s.a + {k} + ", 1)
            c.send({"jsonrpc": "2.0", "method": "textDocument/didChange",
                    "params": {"textDocument": {"uri": uri, "version": 2 + k}, "contentChanges": [{"text": changed}]}})
            t0 = time.perf_counter()
            c.call(10 + k, "textDocument/diagnostic", {"textDocument": {"uri": uri}})
            latencies.append(time.perf_counter() - t0)
        c.close()
        latencies.sort()
        return n + 1, open_latency, latencies[len(latencies) // 2]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--lsp", default="trust-lsp")
    ap.add_argument("--edit", action="store_true", help="editor scenario: open/change one file and pull its diagnostics")
    ap.add_argument("sizes", nargs="*", type=int, default=[25, 50, 100, 200])
    a = ap.parse_args()
    if a.edit:
        print(f"{'files':>6} {'open -> diagnostics':>20} {'edit -> diagnostics (median)':>29}")
        for n in a.sizes:
            files, opened, edit = measure_edits(a.lsp, n)
            print(f"{files:6d} {opened*1000:18.0f}ms {edit*1000:27.0f}ms")
        return
    print(f"{'files':>6} {'first pull':>11} {'all pulls':>10} {'per file':>9} {'diagnostics':>12}")
    for n in a.sizes:
        files, first, total, diags = measure(a.lsp, n)
        print(f"{files:6d} {first*1000:9.0f}ms {total*1000:8.0f}ms {total/files*1000:7.2f}ms {diags:12d}")


if __name__ == "__main__":
    main()
