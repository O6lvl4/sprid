"""sprid against Ghostty, same conditions.

Both are started the way Finder starts them (`open -W -n -a`), with the same
font, grid (100 x 30) and scrollback budget (10 MB), running the same command,
and left to quit when it exits. Measured, median of RUNS:

  startup    `true`: launch to quit
  cat        a file of Claude-Code-like output: launch to quit, minus startup
  peak       the largest phys_footprint seen while `cat` runs
  idle cpu   CPU time used between 5 s and 15 s of `sleep 20`, as a %
  idle mem   phys_footprint at 15 s of `sleep 20`

  python3 scripts/bench_vs.py SPRID_APP GHOSTTY_APP [DATA_FILE]
"""

import os
import re
import statistics
import subprocess
import sys
import threading
import time

RUNS = 3
FONT = "UDEV Gothic 35NFLG"
SIZE = "14.5"


def args_for(app: str, command: list[str]) -> list[str]:
    if "Ghostty" in app:
        return [
            "--config-default-files=false",
            f"--font-family={FONT}",
            f"--font-size={SIZE}",
            "--window-width=100",
            "--window-height=30",
            "--scrollback-limit=10000000",
            "--quit-after-last-window-closed=true",
            "--wait-after-command=false",
            "--confirm-close-surface=false",
            "--window-save-state=never",
            "-e",
        ] + command
    return ["--cols=100", "--rows=30", "-e"] + command


def exe_of(app: str) -> str:
    macos = os.path.join(app, "Contents", "MacOS")
    names = [n for n in os.listdir(macos) if not n.startswith(".")]
    return os.path.join(macos, "ghostty" if "ghostty" in names else names[0])


def pid_of(exe: str, timeout: float = 10.0) -> int:
    end = time.time() + timeout
    while time.time() < end:
        out = subprocess.run(["pgrep", "-n", "-f", exe], capture_output=True, text=True).stdout.split()
        if out:
            return int(out[0])
        time.sleep(0.02)
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
    parts = [float(p) for p in out.replace("-", ":").split(":")]
    secs = 0.0
    for p in parts:
        secs = secs * 60 + p
    return secs


def launch(app: str, command: list[str]) -> subprocess.Popen:
    return subprocess.Popen(["open", "-W", "-n", "-a", app, "--args"] + args_for(app, command))


def timed(app: str, command: list[str], watch_memory: bool = False) -> tuple[float, float]:
    """Seconds from launch to quit, and the peak footprint (MB) if watched."""
    t0 = time.time()
    proc = launch(app, command)
    peak = 0.0
    if watch_memory:
        pid = pid_of(exe_of(app))
        stop = threading.Event()

        def sample():
            nonlocal peak
            while not stop.is_set():
                peak = max(peak, footprint_mb(pid))
                time.sleep(0.25)

        th = threading.Thread(target=sample)
        th.start()
        proc.wait()
        stop.set()
        th.join()
    else:
        proc.wait()
    return time.time() - t0, peak


def idle(app: str) -> tuple[float, float]:
    proc = launch(app, ["/bin/sleep", "20"])
    pid = pid_of(exe_of(app))
    time.sleep(5)
    c0 = cpu_seconds(pid)
    time.sleep(10)
    c1 = cpu_seconds(pid)
    mem = footprint_mb(pid)
    proc.wait()
    return (c1 - c0) / 10 * 100, mem


def measure(app: str, data: str) -> dict:
    startup = statistics.median(timed(app, ["/usr/bin/true"])[0] for _ in range(RUNS))
    cats = [timed(app, ["/bin/cat", data], watch_memory=True) for _ in range(RUNS)]
    cat = statistics.median(c[0] for c in cats) - startup
    peak = statistics.median(c[1] for c in cats)
    idles = [idle(app) for _ in range(RUNS)]
    return {
        "startup": startup,
        "cat": cat,
        "mbps": os.path.getsize(data) / 1e6 / max(cat, 1e-3),
        "peak": peak,
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
