//! proc.rs -- leitura /proc para metricas do sistema.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CpuStat {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuStat {
    pub fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq + self.steal
    }

    pub fn active(&self) -> u64 {
        self.total() - self.idle - self.iowait
    }
}

pub fn read_cpu_stat() -> CpuStat {
    let Ok(content) = std::fs::read_to_string("/proc/stat") else { return CpuStat::default() };
    for line in content.lines() {
        if line.starts_with("cpu ") {
            let fields: Vec<u64> = line.split_whitespace().skip(1)
                .filter_map(|s| s.parse().ok()).collect();
            return CpuStat {
                user:    fields.get(0).copied().unwrap_or(0),
                nice:    fields.get(1).copied().unwrap_or(0),
                system:  fields.get(2).copied().unwrap_or(0),
                idle:    fields.get(3).copied().unwrap_or(0),
                iowait:  fields.get(4).copied().unwrap_or(0),
                irq:     fields.get(5).copied().unwrap_or(0),
                softirq: fields.get(6).copied().unwrap_or(0),
                steal:   fields.get(7).copied().unwrap_or(0),
            };
        }
    }
    CpuStat::default()
}

/// Calculate CPU usage % from two successive samples.
pub fn cpu_percent(prev: &CpuStat, curr: &CpuStat) -> f32 {
    let total_diff = curr.total().saturating_sub(prev.total());
    let active_diff = curr.active().saturating_sub(prev.active());
    if total_diff == 0 { return 0.0; }
    (active_diff as f32 / total_diff as f32 * 100.0).clamp(0.0, 100.0)
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct MemInfo {
    pub total_kb: u64,
    pub free_kb: u64,
    pub available_kb: u64,
    pub buffers_kb: u64,
    pub cached_kb: u64,
}

impl MemInfo {
    pub fn used_kb(&self) -> u64 {
        self.total_kb.saturating_sub(self.available_kb)
    }

    pub fn used_percent(&self) -> f32 {
        if self.total_kb == 0 { return 0.0; }
        (self.used_kb() as f32 / self.total_kb as f32 * 100.0).clamp(0.0, 100.0)
    }
}

pub fn read_meminfo() -> MemInfo {
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else { return MemInfo::default() };
    let mut m: HashMap<String, u64> = HashMap::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0].trim_end_matches(':').to_string();
            if let Ok(v) = parts[1].parse::<u64>() {
                m.insert(key, v);
            }
        }
    }
    MemInfo {
        total_kb:     *m.get("MemTotal").unwrap_or(&0),
        free_kb:      *m.get("MemFree").unwrap_or(&0),
        available_kb: *m.get("MemAvailable").unwrap_or(&0),
        buffers_kb:   *m.get("Buffers").unwrap_or(&0),
        cached_kb:    *m.get("Cached").unwrap_or(&0),
    }
}

// ---------------------------------------------------------------------------
// Disk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DiskMount {
    pub device: String,
    pub mount: String,
    pub fstype: String,
    pub total_kb: u64,
    pub used_kb: u64,
    pub free_kb: u64,
}

impl DiskMount {
    pub fn used_percent(&self) -> f32 {
        if self.total_kb == 0 { return 0.0; }
        (self.used_kb as f32 / self.total_kb as f32 * 100.0).clamp(0.0, 100.0)
    }
}

pub fn read_mounts() -> Vec<DiskMount> {
    let Ok(content) = std::fs::read_to_string("/proc/mounts") else { return Vec::new() };
    let mut mounts = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 { continue; }
        let device = parts[0];
        let mount  = parts[1];
        let fstype = parts[2];
        if !["ext4", "btrfs", "xfs", "vfat", "tmpfs", "overlay"].contains(&fstype) { continue; }
        if let Some(stat) = statvfs(mount) {
            mounts.push(DiskMount {
                device: device.to_string(),
                mount:  mount.to_string(),
                fstype: fstype.to_string(),
                total_kb: stat.0 / 1024,
                free_kb:  stat.1 / 1024,
                used_kb:  stat.0.saturating_sub(stat.1) / 1024,
            });
        }
    }
    mounts.dedup_by(|a, b| a.mount == b.mount);
    mounts
}

fn statvfs(path: &str) -> Option<(u64, u64)> {
    // Fallback: parse /proc/diskstats is not straightforward for free space.
    // Use statvfs syscall via libc. If unavailable, return None.
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        let cpath = CString::new(path).ok()?;
        let mut stat: libc::statvfs64 = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::statvfs64(cpath.as_ptr(), &mut stat) };
        if ret == 0 {
            let total = stat.f_blocks * stat.f_frsize;
            let free  = stat.f_bavail * stat.f_frsize;
            Some((total, free))
        } else { None }
    }
    #[cfg(not(target_os = "linux"))]
    { let _ = path; None }
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NetIface {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_rate: u64,
    pub tx_rate: u64,
}

pub fn read_net_dev() -> Vec<NetIface> {
    let Ok(content) = std::fs::read_to_string("/proc/net/dev") else { return Vec::new() };
    let mut ifaces = Vec::new();
    for line in content.lines().skip(2) {
        let trimmed = line.trim();
        let colon_pos = match trimmed.find(':') { Some(p) => p, None => continue };
        let name = trimmed[..colon_pos].trim().to_string();
        let fields: Vec<u64> = trimmed[colon_pos+1..].split_whitespace()
            .filter_map(|s| s.parse().ok()).collect();
        let rx = fields.get(0).copied().unwrap_or(0);
        let tx = fields.get(8).copied().unwrap_or(0);
        ifaces.push(NetIface { name, rx_bytes: rx, tx_bytes: tx, rx_rate: 0, tx_rate: 0 });
    }
    ifaces
}

pub fn compute_net_rates(prev: &[NetIface], curr: &mut Vec<NetIface>, elapsed_secs: f32) {
    for iface in curr.iter_mut() {
        if let Some(p) = prev.iter().find(|p| p.name == iface.name) {
            let rx_diff = iface.rx_bytes.saturating_sub(p.rx_bytes);
            let tx_diff = iface.tx_bytes.saturating_sub(p.tx_bytes);
            iface.rx_rate = (rx_diff as f32 / elapsed_secs) as u64;
            iface.tx_rate = (tx_diff as f32 / elapsed_secs) as u64;
        }
    }
}

// ---------------------------------------------------------------------------
// Processes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ProcEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub rss_kb: u64,
    pub cmd: String,
}

pub fn read_processes(cpu_total_diff: u64, ticks_per_sec: u64) -> Vec<ProcEntry> {
    let Ok(dirs) = std::fs::read_dir("/proc") else { return Vec::new() };
    let mut entries = Vec::new();
    for entry in dirs.flatten() {
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        let Ok(pid) = fname_str.parse::<u32>() else { continue };
        let stat_path = format!("/proc/{}/stat", pid);
        let stat_str  = std::fs::read_to_string(&stat_path).unwrap_or_default();
        let status_path = format!("/proc/{}/status", pid);
        let status_str  = std::fs::read_to_string(&status_path).unwrap_or_default();
        let cmd_path  = format!("/proc/{}/cmdline", pid);
        let cmd_raw   = std::fs::read(cmd_path).unwrap_or_default();
        let cmd = String::from_utf8_lossy(&cmd_raw).replace('\0', " ").trim().to_string();

        // parse name from stat: second field is (name)
        let name = stat_str.find('(').and_then(|start| stat_str.find(')').map(|end| stat_str[start+1..end].to_string())).unwrap_or_default();

        // utime+stime fields (14th and 15th) in stat
        let stat_fields: Vec<&str> = stat_str.split_whitespace().collect();
        let utime: u64 = stat_fields.get(13).and_then(|s| s.parse().ok()).unwrap_or(0);
        let stime: u64 = stat_fields.get(14).and_then(|s| s.parse().ok()).unwrap_or(0);
        let proc_ticks = utime + stime;

        let cpu_pct = if cpu_total_diff > 0 && ticks_per_sec > 0 {
            (proc_ticks as f32 / cpu_total_diff as f32 * ticks_per_sec as f32).clamp(0.0, 100.0)
        } else { 0.0 };

        // VmRSS from /proc/pid/status
        let rss_kb = status_str.lines()
            .find(|l| l.starts_with("VmRSS:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u64);

        entries.push(ProcEntry { pid, name, cpu_pct, rss_kb, cmd });
    }
    entries.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(std::cmp::Ordering::Equal));
    entries.truncate(30);
    entries
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_percent_zero_when_no_diff() {
        let s = CpuStat::default();
        assert_eq!(cpu_percent(&s, &s), 0.0);
    }

    #[test]
    fn test_cpu_percent_max_100() {
        let prev = CpuStat { user: 0, nice: 0, system: 0, idle: 0, iowait: 0, irq: 0, softirq: 0, steal: 0 };
        let curr = CpuStat { user: 100, nice: 0, system: 0, idle: 0, iowait: 0, irq: 0, softirq: 0, steal: 0 };
        assert!(cpu_percent(&prev, &curr) <= 100.0);
    }

    #[test]
    fn test_meminfo_used_percent_range() {
        let m = MemInfo { total_kb: 8_000_000, free_kb: 2_000_000, available_kb: 3_000_000, buffers_kb: 500_000, cached_kb: 1_000_000 };
        let pct = m.used_percent();
        assert!(pct >= 0.0 && pct <= 100.0);
    }

    #[test]
    fn test_meminfo_used_kb() {
        let m = MemInfo { total_kb: 10_000, free_kb: 2_000, available_kb: 4_000, buffers_kb: 0, cached_kb: 0 };
        assert_eq!(m.used_kb(), 6_000);
    }

    #[test]
    fn test_disk_used_percent() {
        let d = DiskMount {
            device: "sda".into(), mount: "/".into(), fstype: "ext4".into(),
            total_kb: 100_000, used_kb: 40_000, free_kb: 60_000,
        };
        assert!((d.used_percent() - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_net_rates_computed() {
        let prev = vec![NetIface { name: "eth0".into(), rx_bytes: 1000, tx_bytes: 500, rx_rate: 0, tx_rate: 0 }];
        let mut curr = vec![NetIface { name: "eth0".into(), rx_bytes: 3000, tx_bytes: 1500, rx_rate: 0, tx_rate: 0 }];
        compute_net_rates(&prev, &mut curr, 2.0);
        assert_eq!(curr[0].rx_rate, 1000); // 2000 bytes / 2s = 1000 B/s
        assert_eq!(curr[0].tx_rate, 500);
    }
}
