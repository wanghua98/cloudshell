//! One-shot remote system information collection.
//!
//! The SSH worker runs [`SYSTEM_INFO_COMMAND`] only when the user opens or
//! refreshes a system-information tab.  The command emits small, marked
//! sections that this module parses without keeping another remote process
//! alive.

use std::collections::HashMap;

/// Hard cap for one response. The command only returns a few KiB, but a cap
/// prevents a hostile or unexpectedly configured remote shell from growing the
/// client without bound.
pub const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

/// Portable shell with Linux detail and graceful Unix fallbacks.
///
/// `/proc/stat` and `/proc/net/dev` are sampled twice, one second apart, so CPU
/// breakdown and interface rates describe an interval rather than lifetime
/// averages. All other values are snapshots. A fixed PATH avoids executing
/// commands shadowed by the remote account.
pub const SYSTEM_INFO_COMMAND: &[u8] = br#"PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH
LC_ALL=C
export LC_ALL
echo __CS_OVERVIEW__
printf 'os='
if [ -r /etc/os-release ]; then
    awk -F= '$1=="PRETTY_NAME"{sub(/^[^=]*=/,""); print; exit}' /etc/os-release
else
    uname -s 2>/dev/null
fi
printf 'kernel='; uname -s 2>/dev/null
printf 'kernel_version='; uname -r 2>/dev/null
printf 'architecture='; uname -m 2>/dev/null
printf 'hostname='; hostname 2>/dev/null || uname -n 2>/dev/null
echo __CS_CPU__
if [ -r /proc/cpuinfo ]; then
    awk -F: '
        /^processor[[:space:]]*:/ { cores++ }
        name=="" && /^(model name|Hardware|Processor)[[:space:]]*:/ {
            name=$2; sub(/^[ \t]+/,"",name)
        }
        mhz=="" && /^cpu MHz[[:space:]]*:/ {
            mhz=$2; sub(/^[ \t]+/,"",mhz)
        }
        cache=="" && /^cache size[[:space:]]*:/ {
            cache=$2; sub(/^[ \t]+/,"",cache)
        }
        bogo=="" && /^BogoMIPS[[:space:]]*:/ {
            bogo=$2; sub(/^[ \t]+/,"",bogo)
        }
        END {
            print "name=" name
            print "cores=" cores
            print "mhz=" mhz
            print "cache=" cache
            print "bogomips=" bogo
        }' /proc/cpuinfo
else
    printf 'name='; sysctl -n machdep.cpu.brand_string 2>/dev/null
    printf 'cores='; getconf _NPROCESSORS_ONLN 2>/dev/null
    printf 'mhz=\ncache=\nbogomips=\n'
fi
echo __CS_STAT1__
head -n 1 /proc/stat 2>/dev/null
echo __CS_NET1__
cat /proc/net/dev 2>/dev/null
sleep 1
echo __CS_STAT2__
head -n 1 /proc/stat 2>/dev/null
echo __CS_NET2__
cat /proc/net/dev 2>/dev/null
echo __CS_MEMORY__
cat /proc/meminfo 2>/dev/null
echo __CS_FILESYSTEMS__
df -kP 2>/dev/null
echo __CS_END__
"#;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemInfoSnapshot {
    pub os: String,
    pub kernel: String,
    pub kernel_version: String,
    pub architecture: String,
    pub hostname: String,
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub cpu_mhz: String,
    pub cpu_cache: String,
    pub bogomips: String,
    pub cpu: CpuBreakdown,
    pub mem_total_kib: u64,
    pub mem_used_kib: u64,
    pub swap_total_kib: u64,
    pub swap_used_kib: u64,
    pub networks: Vec<NetworkInfo>,
    pub filesystems: Vec<FilesystemInfo>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuBreakdown {
    pub user: f32,
    pub system: f32,
    pub nice: f32,
    pub idle: f32,
    pub io_wait: f32,
    pub hard_irq: f32,
    pub soft_irq: f32,
    pub steal: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkInfo {
    pub name: String,
    pub rx_total: u64,
    pub tx_total: u64,
    pub rx_per_sec: u64,
    pub tx_per_sec: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilesystemInfo {
    pub name: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percent: String,
    pub mount_point: String,
}

pub fn parse_system_info(text: &str) -> Result<SystemInfoSnapshot, String> {
    let sections = split_sections(text);
    let overview = parse_key_values(section(&sections, "__CS_OVERVIEW__"));
    let cpu_values = parse_key_values(section(&sections, "__CS_CPU__"));

    if overview.is_empty() && cpu_values.is_empty() {
        return Err("remote command returned no recognizable system information".into());
    }

    let stat1 = parse_cpu_stat(section(&sections, "__CS_STAT1__"));
    let stat2 = parse_cpu_stat(section(&sections, "__CS_STAT2__"));
    let memory = parse_memory(section(&sections, "__CS_MEMORY__"));
    let net1 = parse_network_counters(section(&sections, "__CS_NET1__"));
    let net2 = parse_network_counters(section(&sections, "__CS_NET2__"));

    let mem_total = memory.get("MemTotal").copied().unwrap_or_default();
    let mem_available = memory.get("MemAvailable").copied().unwrap_or_else(|| {
        memory
            .get("MemFree")
            .copied()
            .unwrap_or_default()
            .saturating_add(memory.get("Buffers").copied().unwrap_or_default())
            .saturating_add(memory.get("Cached").copied().unwrap_or_default())
    });
    let swap_total = memory.get("SwapTotal").copied().unwrap_or_default();
    let swap_free = memory.get("SwapFree").copied().unwrap_or_default();

    Ok(SystemInfoSnapshot {
        os: clean_value(overview.get("os")),
        kernel: clean_value(overview.get("kernel")),
        kernel_version: clean_value(overview.get("kernel_version")),
        architecture: clean_value(overview.get("architecture")),
        hostname: clean_value(overview.get("hostname")),
        cpu_name: clean_value(cpu_values.get("name")),
        cpu_cores: cpu_values
            .get("cores")
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or_default(),
        cpu_mhz: clean_value(cpu_values.get("mhz")),
        cpu_cache: clean_value(cpu_values.get("cache")),
        bogomips: clean_value(cpu_values.get("bogomips")),
        cpu: cpu_breakdown(stat1, stat2),
        mem_total_kib: mem_total,
        mem_used_kib: mem_total.saturating_sub(mem_available),
        swap_total_kib: swap_total,
        swap_used_kib: swap_total.saturating_sub(swap_free),
        networks: merge_networks(&net1, &net2),
        filesystems: parse_filesystems(section(&sections, "__CS_FILESYSTEMS__")),
    })
}

fn section<'a>(sections: &'a HashMap<String, String>, name: &str) -> &'a str {
    sections.get(name).map(String::as_str).unwrap_or_default()
}

fn split_sections(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut current: Option<String> = None;
    let mut body = String::new();

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with("__CS_") && line.ends_with("__") {
            if let Some(name) = current.take() {
                out.insert(name, std::mem::take(&mut body));
            }
            if line == "__CS_END__" {
                break;
            }
            current = Some(line.to_string());
        } else if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(name) = current {
        out.insert(name, body);
    }
    out
}

fn parse_key_values(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn clean_value(value: Option<&String>) -> String {
    value
        .map(|value| value.trim().trim_matches(['"', '\'']).to_string())
        .unwrap_or_default()
}

fn parse_memory(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter_map(|line| {
            let (key, rest) = line.split_once(':')?;
            let value = rest.split_whitespace().next()?.parse().ok()?;
            Some((key.to_string(), value))
        })
        .collect()
}

fn parse_cpu_stat(text: &str) -> Option<[u64; 8]> {
    let mut fields = text
        .lines()
        .find(|line| line.trim_start().starts_with("cpu "))?
        .split_whitespace()
        .skip(1)
        .map(|field| field.parse::<u64>().unwrap_or_default());
    Some(std::array::from_fn(|_| fields.next().unwrap_or_default()))
}

fn cpu_breakdown(first: Option<[u64; 8]>, second: Option<[u64; 8]>) -> CpuBreakdown {
    let (Some(first), Some(second)) = (first, second) else {
        return CpuBreakdown::default();
    };
    let delta = std::array::from_fn::<_, 8, _>(|i| second[i].saturating_sub(first[i]));
    let total: u64 = delta.iter().sum();
    if total == 0 {
        return CpuBreakdown::default();
    }
    let pct = |value: u64| value as f32 * 100.0 / total as f32;
    CpuBreakdown {
        user: pct(delta[0]),
        nice: pct(delta[1]),
        system: pct(delta[2]),
        idle: pct(delta[3]),
        io_wait: pct(delta[4]),
        hard_irq: pct(delta[5]),
        soft_irq: pct(delta[6]),
        steal: pct(delta[7]),
    }
}

/// Linux `/proc/net/dev`: interface, received bytes, transmitted bytes.
fn parse_network_counters(text: &str) -> HashMap<String, (u64, u64)> {
    const MAX_INTERFACES: usize = 256;
    text.lines()
        .filter_map(|line| {
            let (name, counters) = line.split_once(':')?;
            let values: Vec<u64> = counters
                .split_whitespace()
                .filter_map(|value| value.parse().ok())
                .collect();
            if values.len() < 9 {
                return None;
            }
            Some((name.trim().to_string(), (values[0], values[8])))
        })
        .take(MAX_INTERFACES)
        .collect()
}

fn merge_networks(
    first: &HashMap<String, (u64, u64)>,
    second: &HashMap<String, (u64, u64)>,
) -> Vec<NetworkInfo> {
    let mut rows: Vec<_> = second
        .iter()
        .map(|(name, &(rx, tx))| {
            let (old_rx, old_tx) = first.get(name).copied().unwrap_or((rx, tx));
            NetworkInfo {
                name: name.clone(),
                rx_total: rx,
                tx_total: tx,
                rx_per_sec: rx.saturating_sub(old_rx),
                tx_per_sec: tx.saturating_sub(old_tx),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        let a_rate = a.rx_per_sec.saturating_add(a.tx_per_sec);
        let b_rate = b.rx_per_sec.saturating_add(b.tx_per_sec);
        b_rate.cmp(&a_rate).then_with(|| a.name.cmp(&b.name))
    });
    rows
}

fn parse_filesystems(text: &str) -> Vec<FilesystemInfo> {
    const MAX_FILESYSTEMS: usize = 256;
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 6 {
                return None;
            }
            let total_kib = fields[1].parse::<u64>().ok()?;
            let used_kib = fields[2].parse::<u64>().ok()?;
            let available_kib = fields[3].parse::<u64>().ok()?;
            Some(FilesystemInfo {
                name: fields[0].to_string(),
                total: total_kib.saturating_mul(1024),
                used: used_kib.saturating_mul(1024),
                available: available_kib.saturating_mul(1024),
                percent: fields[4].to_string(),
                mount_point: fields[5..].join(" "),
            })
        })
        .take(MAX_FILESYSTEMS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
noise before markers
__CS_OVERVIEW__
os="Ubuntu 24.04.2 LTS"
kernel=Linux
kernel_version=6.8.0-60-generic
architecture=x86_64
hostname=demo
__CS_CPU__
name=Example CPU @ 2.50GHz
cores=4
mhz=2494.140
cache=36608 KB
bogomips=4988.28
__CS_STAT1__
cpu  100 10 20 800 10 5 5 0
__CS_NET1__
Inter-| Receive | Transmit
  eth0: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0
__CS_STAT2__
cpu  120 10 30 860 20 5 5 0
__CS_NET2__
Inter-| Receive | Transmit
  eth0: 1500 0 0 0 0 0 0 0 2800 0 0 0 0 0 0 0
__CS_MEMORY__
MemTotal:        2048000 kB
MemAvailable:     512000 kB
SwapTotal:       1024000 kB
SwapFree:         768000 kB
__CS_FILESYSTEMS__
Filesystem 1024-blocks Used Available Capacity Mounted on
/dev/vda2 100000 40000 60000 40% /
__CS_END__
"#;

    #[test]
    fn parses_snapshot_and_interval_deltas() {
        let info = parse_system_info(SAMPLE).unwrap();
        assert_eq!(info.os, "Ubuntu 24.04.2 LTS");
        assert_eq!(info.hostname, "demo");
        assert_eq!(info.cpu_cores, 4);
        assert_eq!(info.mem_used_kib, 1_536_000);
        assert_eq!(info.swap_used_kib, 256_000);
        assert!((info.cpu.user - 20.0).abs() < 0.01);
        assert!((info.cpu.system - 10.0).abs() < 0.01);
        assert!((info.cpu.idle - 60.0).abs() < 0.01);
        assert_eq!(info.networks[0].rx_per_sec, 500);
        assert_eq!(info.networks[0].tx_per_sec, 800);
        assert_eq!(info.filesystems[0].mount_point, "/");
        assert_eq!(info.filesystems[0].total, 102_400_000);
    }

    #[test]
    fn rejects_output_without_markers() {
        assert!(parse_system_info("permission denied").is_err());
    }
}
