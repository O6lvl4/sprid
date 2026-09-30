"""sprid against Ghostty, same conditions.

Both are started the way Finder starts them (`open -n -a`), with the same
font, grid (100 x 30) and scrollback budget (10 MB), running the same script.
Timing comes from the script itself: it stamps a marker file when its work
is done, so neither terminal's way of quitting (Ghostty keeps its hidden
quick-terminal window, and so its process, after the last window closes)
enters the numbers. After each run the terminal is terminated.

Measured, median of RUNS:

  startup    launch until a script that only stamps the marker has run
  cat        launch until `cat` of a Claude-Code-like file is done, minus
             startup: the time the terminal took to take all of it in
  peak       the largest phys_footprint seen while `cat` runs
  idle cpu   CPU time used from 10 s to 30 s with nothing running, as a %
  idle mem   phys_footprint at 30 s

  python3 scripts/bench_vs.py SPRID_APP GHOSTTY_APP [DATA_FILE]
"""

import os
import re
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time

RUNS = 5
FONT = "UDEV Gothic 35NFLG"
SIZE = "14.5"
WORK = tempfile.mkdtemp(prefix="sprid-bench-")
MARKER = os.path.join(WORK, "done")


def script(name: str, body: str) -> str:
    path = os.path.join(WORK, name)
    with open(path, "w") as f:
        f.write("#!/bin/sh\n" + body + "\n")
    os.chmod(path, 0o755)
    return path


def args_for(app: str, command: str) -> list[str]:
    if "Ghostty" in app:
        # `--command`, not `-e`: Ghostty 1.3 asks before running an `-e`
        # command it was handed through `open`.
        return [
            "--config-default-files=false",
            f"--font-family={FONT}",
            f"--font-size={SIZE}",
            "--window-width=100",
            "--window-height=30",
            "--scrollback-limit=10000000",
            "--confirm-close-surface=false",
            "--window-save-state=never",
            f"--command={command}",
        ]
    return ["--cols=100", "--rows=30", "-e", command]


def exe_of(app: str) -> str:
    macos = os.path.join(app, "Contents", "MacOS")
    names = [n for n in os.listdir(macos) if not n.startswith(".")]
    return os.path.join(macos, "ghostty" if "ghostty" in names else names[0])


def pid_of(exe: str, timeout: float = 15.0) -> int:
    end = time.time() + timeout
    while time.time() < end:
        out = subprocess.run(["pgrep", "-n", "-f", exe], capture_output=True, text=True).stdout.split()
        if out:
            return int(out[0])
        time.sleep(0.01)
    raise RuntimeError(f"{exe} did not start")


def footprint_mb(pid: int) -> float:
    out = subprocess.run(["footprint", str(pid)], capture_output=True, text=True).stdout
    m = re.search(r"Footprint:\s+([\d.]+)\s+(KB|MB|GB)", out)
    if not m:
        return 0.0
    v = float(m.group(1))
    return {"KB": v / 1024, "MB": v, "GB": v * 1024}[m.group(2)]


def cpu_seconds(pid: int) -> float:
    out = subprocess.run(["ps", "-o", "cputime=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
    if not out:
        return 0.0
    secs = 0.0
    for p in out.replace("-", ":").split(":"):
        secs = secs * 60 + float(p)
    return secs


def stop(pid: int) -> None:
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    for _ in range(200):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.02)
    os.kill(pid, signal.SIGKILL)


def run(app: str, command: str, watch_memory: bool = False, timeout: float = 300.0) -> tuple[float, float, int]:
    """Seconds from launch until the script stamps the marker, the peak
    footprint (MB) if watched, and the terminal's pid (still running)."""
    if os.path.exists(MARKER):
        os.remove(MARKER)
    t0 = time.time()
    subprocess.Popen(["open", "-n", "-a", app, "--args"] + args_for(app, command))
    pid = pid_of(exe_of(app))
    peak = 0.0
    next_sample = 0.0
    end = t0 + timeout
    while not os.path.exists(MARKER) and time.time() < end:
        if watch_memory and time.time() >= next_sample:
            peak = max(peak, footprint_mb(pid))
            next_sample = time.time() + 0.25
        time.sleep(0.005)
    done = os.stat(MARKER).st_mtime if os.path.exists(MARKER) else float("nan")
    return done - t0, peak, pid


def measure(app: str, data: str) -> dict:
    stamp = script("startup.sh", f"touch {MARKER}")
    cat = script("cat.sh", f"cat '{data}'\ntouch {MARKER}")
    idle_sh = script("idle.sh", f"touch {MARKER}\nexec sleep 60")

    startups = []
    for _ in range(RUNS):
        secs, _, pid = run(app, stamp)
        stop(pid)
        startups.append(secs)
    startup = statistics.median(startups)

    cats = []
    for _ in range(RUNS):
        secs, peak, pid = run(app, cat, watch_memory=True)
        stop(pid)
        cats.append((secs - startup, peak))
    cat_secs = statistics.median(c[0] for c in cats)

    idles = []
    for _ in range(RUNS):
        _, _, pid = run(app, idle_sh)
        # Past the start's settling (window events, first frames).
        time.sleep(10)
        c0 = cpu_seconds(pid)
        time.sleep(20)
        c1 = cpu_seconds(pid)
        idles.append(((c1 - c0) / 20 * 100, footprint_mb(pid)))
        stop(pid)

    return {
        "startup": startup,
        "cat": cat_secs,
        "mbps": os.path.getsize(data) / 1e6 / max(cat_secs, 1e-3),
        "peak": statistics.median(c[1] for c in cats),
        "idle_cpu": statistics.median(i[0] for i in idles),
        "idle_mem": statistics.median(i[1] for i in idles),
    }


def main() -> None:
    sprid, ghostty = sys.argv[1], sys.argv[2]
    data = sys.argv[3] if len(sys.argv) > 3 else "/tmp/sprid-bench.txt"
    results = {}
    for name, app in (("sprid", sprid), ("Ghostty", ghostty)):
        print(f"measuring {name} ...", file=sys.stderr)
        results[name] = measure(app, data)
    s, g = results["sprid"], results["Ghostty"]
    print(f"| | sprid | Ghostty {version(ghostty)} |")
    print("|---|---|---|")
    print(f"| startup | {s['startup']:.2f} s | {g['startup']:.2f} s |")
    print(f"| cat {os.path.getsize(data) / 1e6:.0f} MB | {s['cat']:.2f} s ({s['mbps']:.0f} MB/s) | {g['cat']:.2f} s ({g['mbps']:.0f} MB/s) |")
    print(f"| peak footprint during cat | {s['peak']:.0f} MB | {g['peak']:.0f} MB |")
    print(f"| idle CPU | {s['idle_cpu']:.2f} % | {g['idle_cpu']:.2f} % |")
    print(f"| idle footprint | {s['idle_mem']:.0f} MB | {g['idle_mem']:.0f} MB |")


def version(app: str) -> str:
    out = subprocess.run([exe_of(app), "--version"], capture_output=True, text=True).stdout
    m = re.search(r"Ghostty ([\d.]+)", out)
    return m.group(1) if m else ""


if __name__ == "__main__":
    main()
