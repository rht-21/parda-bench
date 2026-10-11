"""Best-effort description of the machine, recorded with every run so latency numbers have context."""

import logging
import os
import platform
import subprocess
import sys
from pathlib import Path

from parda_bench.spec.record import Hardware

log = logging.getLogger(__name__)
UNKNOWN = "unknown"


def _sysctl(name: str) -> str | None:
    try:
        out = subprocess.run(["sysctl", "-n", name], capture_output=True, text=True, check=True)
    except (OSError, subprocess.CalledProcessError):
        return None
    return out.stdout.strip()


def _proc_value(path: str, key: str) -> str | None:
    try:
        lines = Path(path).read_text().splitlines()
    except OSError:
        return None
    return next((line.split(":", 1)[1].strip() for line in lines if line.startswith(key)), None)


def probe() -> Hardware:
    if sys.platform == "darwin":
        cpu, memory = _sysctl("machdep.cpu.brand_string"), _sysctl("hw.memsize")
        memory_mb = int(memory) // (1024 * 1024) if memory and memory.isdigit() else None
    else:
        cpu = _proc_value("/proc/cpuinfo", "model name")
        total = _proc_value("/proc/meminfo", "MemTotal")
        memory_mb = int(total.split()[0]) // 1024 if total else None
    if cpu is None:
        log.warning("could not read the CPU model; recording `%s`", UNKNOWN)
    if memory_mb is None:
        log.warning("could not read total memory; recording 0")
    return Hardware(
        os=sys.platform,
        arch=platform.machine(),
        cpu=cpu or UNKNOWN,
        cores=os.cpu_count() or 0,
        memory_mb=memory_mb or 0,
    )
