use std::{collections::HashMap, env, fs, path::Path};

use crate::i18n::Translator;

pub struct SystemSection {
    pub label: String,
    pub summary: String,
    pub details: Vec<(String, String)>,
}

pub struct SystemInfo {
    pub sections: [SystemSection; 3],
    pub os_icon: String,
    pub desktop_icon: String,
}

impl SystemInfo {
    pub fn read(text: Translator) -> Self {
        let release = fs::read_to_string("/etc/os-release")
            .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
            .unwrap_or_default();
        let release = parse_release(&release);
        let os_name = release
            .get("PRETTY_NAME")
            .or_else(|| release.get("NAME"))
            .cloned()
            .unwrap_or_else(|| "Linux".into());
        let desktop = env_value("XDG_CURRENT_DESKTOP");
        let hostname = read("/proc/sys/kernel/hostname");
        let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let cpu = cpuinfo
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| matches!(key.trim(), "model name" | "Hardware" | "Model"))
            .map(|(_, value)| value.trim().to_owned())
            .unwrap_or_else(|| text.unavailable().into());
        let memory = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mut pc_details = vec![
            (text.model().into(), read("/sys/class/dmi/id/product_name")),
            ("CPU".into(), cpu),
            (
                text.threads().into(),
                std::thread::available_parallelism()
                    .map(|count| count.to_string())
                    .unwrap_or_else(|_| text.unavailable().into()),
            ),
        ];
        let graphics = read_graphics();
        if graphics.is_empty() {
            pc_details.push(("GPU".into(), text.unavailable().into()));
        } else {
            for (index, name) in graphics.iter().enumerate() {
                let label = if graphics.len() == 1 {
                    "GPU".into()
                } else {
                    format!("GPU {}", index + 1)
                };
                pc_details.push((label, name.clone()));
            }
        }
        pc_details.push((
            text.total_memory().into(),
            format_memory(memory_total(&memory), text),
        ));
        pc_details.extend(crate::storage::details(text));
        let uptime = read("/proc/uptime")
            .split_whitespace()
            .next()
            .and_then(|seconds| seconds.parse::<f64>().ok())
            .map(|seconds| text.uptime(seconds as u64 / 3600, seconds as u64 % 3600 / 60))
            .unwrap_or_else(|| text.unavailable().into());
        let desktop_icon = match desktop.to_lowercase().as_str() {
            desktop if desktop.contains("kde") => "kde",
            desktop if desktop.contains("gnome") => "org.gnome.Settings",
            desktop if desktop.contains("xfce") => "org.xfce.settings.manager",
            _ => "preferences-desktop",
        }
        .to_owned();
        Self {
            os_icon: release
                .get("LOGO")
                .cloned()
                .unwrap_or_else(|| "distributor-logo".into()),
            desktop_icon,
            sections: [
                SystemSection {
                    label: text.operating_system().into(),
                    summary: os_name,
                    details: vec![
                        ("Kernel".into(), read("/proc/sys/kernel/osrelease")),
                        (text.architecture().into(), env::consts::ARCH.into()),
                        (text.uptime_label().into(), uptime),
                    ],
                },
                SystemSection {
                    label: text.desktop_environment().into(),
                    summary: desktop,
                    details: vec![
                        (text.session().into(), env_value("XDG_SESSION_DESKTOP")),
                        (text.protocol().into(), env_value("XDG_SESSION_TYPE")),
                        ("Terminal".into(), env_value("TERM")),
                        ("Shell".into(), env_value("SHELL")),
                    ],
                },
                SystemSection {
                    label: "PC".into(),
                    summary: hostname,
                    details: pc_details,
                },
            ],
        }
    }
}

fn memory_total(contents: &str) -> Option<u64> {
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key != "MemTotal" {
            return None;
        }
        value
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
            .filter(|&total| total > 0)
    })
}

fn format_memory(kib: Option<u64>, text: Translator) -> String {
    kib.map(|value| format!("{:.1} GiB", value as f64 / 1_048_576.0))
        .unwrap_or_else(|| text.unavailable().into())
}

fn read_graphics() -> Vec<String> {
    let pci_ids = [
        "/usr/share/hwdata/pci.ids",
        "/usr/share/misc/pci.ids",
        "/usr/share/pci.ids",
    ]
    .iter()
    .find_map(|path| fs::read_to_string(path).ok())
    .unwrap_or_default();
    let mut devices: Vec<_> = fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| is_graphics_card(&entry.file_name().to_string_lossy()))
        .filter_map(|entry| fs::canonicalize(entry.path().join("device")).ok())
        .collect();
    if let Ok(entries) = fs::read_dir("/sys/bus/pci/devices") {
        devices.extend(entries.filter_map(Result::ok).filter_map(|entry| {
            let class = fs::read_to_string(entry.path().join("class")).ok()?;
            let class = u32::from_str_radix(class.trim().trim_start_matches("0x"), 16).ok()?;
            (class >> 16 == 0x03)
                .then(|| fs::canonicalize(entry.path()).ok())
                .flatten()
        }));
    }
    devices.sort();
    devices.dedup();
    devices
        .iter()
        .filter_map(|device| graphics_name(device, &pci_ids))
        .collect()
}

fn is_graphics_card(name: &str) -> bool {
    name.strip_prefix("card").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn graphics_name(device: &Path, pci_ids: &str) -> Option<String> {
    for field in ["product_name", "label"] {
        if let Ok(name) = fs::read_to_string(device.join(field)) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.into());
            }
        }
    }
    let vendor = fs::read_to_string(device.join("vendor")).ok();
    let identifier = fs::read_to_string(device.join("device")).ok();
    if let (Some(vendor), Some(identifier)) = (
        vendor.as_deref().and_then(parse_pci_id),
        identifier.as_deref().and_then(parse_pci_id),
    ) {
        return Some(pci_name(pci_ids, vendor, identifier).unwrap_or_else(|| {
            let name = match vendor {
                0x1002 => "AMD",
                0x10de => "NVIDIA",
                0x8086 => "Intel",
                _ => "GPU PCI",
            };
            format!("{name} [{vendor:04x}:{identifier:04x}]")
        }));
    }
    fs::read_link(device.join("driver"))
        .ok()?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn parse_pci_id(value: &str) -> Option<u16> {
    u16::from_str_radix(value.trim().trim_start_matches("0x"), 16).ok()
}

fn pci_name(contents: &str, vendor_id: u16, device_id: u16) -> Option<String> {
    let mut vendor_name = None;
    for line in contents.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('\t') {
            if line.starts_with("\t\t") || vendor_name.is_none() {
                continue;
            }
            let Some((identifier, name)) = line.trim_start().split_once(char::is_whitespace) else {
                continue;
            };
            if parse_pci_id(identifier) == Some(device_id) && !name.trim().is_empty() {
                return vendor_name.map(|vendor| format!("{vendor} {}", name.trim()));
            }
        } else {
            if vendor_name.is_some() {
                break;
            }
            let Some((identifier, name)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            if parse_pci_id(identifier) == Some(vendor_id) && !name.trim().is_empty() {
                vendor_name = Some(name.trim().to_owned());
            }
        }
    }
    vendor_name.map(|vendor| format!("{vendor} [{vendor_id:04x}:{device_id:04x}]"))
}

fn read(path: &str) -> String {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "No disponible".into())
}

fn env_value(key: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "No disponible".into())
}

fn parse_release(contents: &str) -> HashMap<String, String> {
    contents
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.to_owned(), value.trim_matches(['"', '\'']).to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{is_graphics_card, memory_total, pci_name};

    #[test]
    fn memory_total_remains_separate_from_disk_usage() {
        assert_eq!(
            memory_total("MemTotal: 8388608 kB\nMemAvailable: 6291456 kB\n"),
            Some(8_388_608)
        );
        assert_eq!(memory_total("MemTotal: invalid kB"), None);
        assert_eq!(memory_total("MemTotal: 0 kB"), None);
        assert_eq!(memory_total("MemAvailable: 100 kB"), None);
    }

    #[test]
    fn pci_names_ignore_subsystems_and_stop_at_the_next_vendor() {
        let ids = "# Devices\n1002  AMD\n\t164e  Raphael\n\t\t1002 73ff  Subsystem\n\t73ff  Radeon RX 6600\n10de  NVIDIA\n\t164e  Other GPU\n\t9999  Other vendor device\n";
        assert_eq!(
            pci_name(ids, 0x1002, 0x73ff).as_deref(),
            Some("AMD Radeon RX 6600")
        );
        assert_eq!(
            pci_name(ids, 0x10de, 0x164e).as_deref(),
            Some("NVIDIA Other GPU")
        );
        assert_eq!(
            pci_name(ids, 0x1002, 0x9999).as_deref(),
            Some("AMD [1002:9999]")
        );
        assert_eq!(pci_name(ids, 0x8086, 0x164e), None);
    }

    #[test]
    fn graphics_cards_exclude_connectors_and_render_nodes() {
        assert!(is_graphics_card("card0"));
        assert!(is_graphics_card("card12"));
        assert!(!is_graphics_card("card"));
        assert!(!is_graphics_card("card0-DP-1"));
        assert!(!is_graphics_card("renderD128"));
    }
}
