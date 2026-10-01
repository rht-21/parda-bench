//! Best-effort description of the machine, recorded with every run so latency numbers have context.

use std::process::Command;

use parda_spec::record::Hardware;

const UNKNOWN: &str = "unknown";

pub fn probe() -> Hardware {
    Hardware {
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        cpu: cpu_name().unwrap_or_else(|| {
            tracing::warn!("could not read the CPU model; recording `{UNKNOWN}`");
            UNKNOWN.to_owned()
        }),
        cores: std::thread::available_parallelism()
            .ok()
            .and_then(|n| u32::try_from(n.get()).ok())
            .unwrap_or_else(|| {
                tracing::warn!("could not read the core count; recording 0");
                0
            }),
        memory_mb: memory_mb().unwrap_or_else(|| {
            tracing::warn!("could not read total memory; recording 0");
            0
        }),
    }
}

fn cpu_name() -> Option<String> {
    if cfg!(target_os = "macos") {
        sysctl("machdep.cpu.brand_string")
    } else {
        let info = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        info.lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_owned())
    }
}

fn memory_mb() -> Option<u64> {
    if cfg!(target_os = "macos") {
        sysctl("hw.memsize")?
            .parse::<u64>()
            .ok()
            .map(|b| b / (1024 * 1024))
    } else {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb = info
            .lines()
            .find(|l| l.starts_with("MemTotal:"))?
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()?;
        Some(kb / 1024)
    }
}

fn sysctl(name: &str) -> Option<String> {
    let out = Command::new("sysctl").args(["-n", name]).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
