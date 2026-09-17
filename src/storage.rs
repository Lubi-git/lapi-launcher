use std::{
    collections::HashSet,
    env,
    ffi::OsString,
    fs,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::i18n::Translator;

pub fn details(text: Translator) -> Vec<(String, String)> {
    let contents = fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    let mounts = parse_mounts(&contents);
    let home = env::var_os("HOME").map(PathBuf::from);
    let home = home.map(|path| fs::canonicalize(&path).unwrap_or(path));
    let selected = select_mounts(&mounts, home.as_deref());
    let mut rows = Vec::new();
    for (context, mount, path) in selected {
        rows.push((
            match context {
                "personal" => text.disk_personal(),
                _ => text.disk_system(),
            }
            .into(),
            mount.fs_type.clone(),
        ));
        let devices = physical_devices(mount, Path::new("/sys"), Path::new("/dev"));
        if devices.is_empty() {
            rows.push((text.drive().into(), text.unavailable().into()));
        } else {
            for (index, device) in devices.iter().enumerate() {
                let label = if devices.len() == 1 {
                    text.drive().into()
                } else {
                    text.drive_number(index)
                };
                rows.push((label, describe_device(device, text)));
            }
        }
        let usage = filesystem_usage(&path);
        rows.push((
            text.used_total().into(),
            usage
                .map(|usage| format!("{} / {}", format_gib(usage.used), format_gib(usage.total)))
                .unwrap_or_else(|| text.unavailable().into()),
        ));
        rows.push((
            text.available().into(),
            usage
                .map(|usage| format_gib(usage.available))
                .unwrap_or_else(|| text.unavailable().into()),
        ));
    }
    if rows.is_empty() {
        rows.push((text.disk().into(), text.unavailable().into()));
    }
    rows
}

#[derive(Debug)]
struct Mount {
    device: String,
    mount_point: PathBuf,
    fs_type: String,
    source: PathBuf,
}

fn parse_mounts(contents: &str) -> Vec<Mount> {
    contents
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let before: Vec<_> = before.split_whitespace().collect();
            let after: Vec<_> = after.split_whitespace().collect();
            if before.len() < 6 || after.len() < 3 {
                return None;
            }
            let (major, minor) = before[2].split_once(':')?;
            major.parse::<u32>().ok()?;
            minor.parse::<u32>().ok()?;
            Some(Mount {
                device: before[2].into(),
                mount_point: unescape_path(before[4]),
                fs_type: after[0].into(),
                source: unescape_path(after[1]),
            })
        })
        .collect()
}

fn unescape_path(value: &str) -> PathBuf {
    let input = value.as_bytes();
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'\\'
            && let Some(digits) = input.get(index + 1..index + 4)
            && digits.iter().all(|digit| (b'0'..=b'7').contains(digit))
            && digits[0] <= b'3'
        {
            decoded.push((digits[0] - b'0') * 64 + (digits[1] - b'0') * 8 + (digits[2] - b'0'));
            index += 4;
        } else {
            decoded.push(input[index]);
            index += 1;
        }
    }
    OsString::from_vec(decoded).into()
}

fn mount_for_path<'a>(mounts: &'a [Mount], path: &Path) -> Option<&'a Mount> {
    mounts
        .iter()
        .filter(|mount| path.starts_with(&mount.mount_point))
        .max_by_key(|mount| mount.mount_point.components().count())
}

fn persistent(mount: &Mount) -> bool {
    matches!(
        mount.fs_type.as_str(),
        "ext2"
            | "ext3"
            | "ext4"
            | "btrfs"
            | "xfs"
            | "f2fs"
            | "vfat"
            | "exfat"
            | "ntfs"
            | "ntfs3"
            | "fuseblk"
            | "bcachefs"
            | "reiserfs"
            | "jfs"
            | "zfs"
    ) && !mount
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("zram") || name.starts_with("ram"))
}

fn same_filesystem(first: &Mount, second: &Mount) -> bool {
    first.fs_type == second.fs_type
        && (first.device == second.device
            || fs::canonicalize(&first.source).unwrap_or_else(|_| first.source.clone())
                == fs::canonicalize(&second.source).unwrap_or_else(|_| second.source.clone()))
}

fn select_mounts<'a>(
    mounts: &'a [Mount],
    home: Option<&Path>,
) -> Vec<(&'static str, &'a Mount, PathBuf)> {
    let mut selected: Vec<(&str, &Mount, PathBuf)> = Vec::new();
    for (context, path) in [("personal", home), ("sistema", Some(Path::new("/")))] {
        let Some(path) = path else { continue };
        let Some(mount) = mount_for_path(mounts, path).filter(|mount| persistent(mount)) else {
            continue;
        };
        if selected
            .iter()
            .any(|(_, previous, _)| same_filesystem(previous, mount))
        {
            continue;
        }
        selected.push((context, mount, path.to_path_buf()));
    }
    selected
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Usage {
    total: u64,
    used: u64,
    available: u64,
}

fn filesystem_usage(path: &Path) -> Option<Usage> {
    let output = Command::new("df")
        .env("LC_ALL", "C")
        .args(["-B1", "--output=size,used,avail", "--"])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_usage(std::str::from_utf8(&output.stdout).ok()?)
}

fn parse_usage(contents: &str) -> Option<Usage> {
    let mut lines = contents.lines().filter(|line| !line.trim().is_empty());
    lines.next()?;
    let fields: Vec<_> = lines.next()?.split_whitespace().collect();
    if fields.len() != 3 || lines.next().is_some() {
        return None;
    }
    let total = fields[0].parse().ok()?;
    if total == 0 {
        return None;
    }
    Some(Usage {
        total,
        used: fields[1].parse().ok()?,
        available: fields[2].parse().ok()?,
    })
}

fn physical_devices(mount: &Mount, sys: &Path, dev: &Path) -> Vec<PathBuf> {
    let block = fs::canonicalize(sys.join("dev/block").join(&mount.device))
        .ok()
        .or_else(|| {
            let source = dev.join(mount.source.strip_prefix("/dev").ok()?);
            let source = fs::canonicalize(source).ok()?;
            fs::canonicalize(sys.join("class/block").join(source.file_name()?)).ok()
        });
    let Some(block) = block else {
        return Vec::new();
    };
    let mut pending = vec![block.clone()];
    if mount.fs_type == "btrfs" {
        for volume in entries(&sys.join("fs/btrfs")) {
            let devices: Vec<_> = entries(&volume.join("devices"))
                .iter()
                .filter_map(|path| fs::canonicalize(path).ok())
                .collect();
            if devices.contains(&block) {
                pending = devices;
                break;
            }
        }
    }
    let mut seen = HashSet::new();
    let mut physical = Vec::new();
    while let Some(device) = pending.pop() {
        let Ok(device) = fs::canonicalize(device) else {
            continue;
        };
        if !seen.insert(device.clone()) {
            continue;
        }
        if device.join("partition").exists() {
            if let Some(parent) = device.parent() {
                pending.push(parent.to_path_buf());
            }
            continue;
        }
        let slaves = entries(&device.join("slaves"));
        if !slaves.is_empty() {
            pending.extend(slaves);
        } else if !device.starts_with(sys.join("devices/virtual")) {
            physical.push(device);
        }
    }
    physical.sort();
    physical
}

fn entries(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

fn attribute(device: &Path, attribute: &str) -> Option<String> {
    let value = fs::read_to_string(device.join(attribute)).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn describe_device(device: &Path, text: Translator) -> String {
    let model = ["device/model", "device/name", "model"]
        .iter()
        .find_map(|name| attribute(device, name))
        .unwrap_or_else(|| text.model_unavailable().into());
    let capacity = attribute(device, "size")
        .and_then(|size| size.parse::<u64>().ok())
        .and_then(|sectors| sectors.checked_mul(512))
        .map(|bytes| text.physical_capacity(bytes))
        .unwrap_or_else(|| text.capacity_unavailable().into());
    format!("{model} · {} · {capacity}", device_type(device, text))
}

fn device_type(device: &Path, text: Translator) -> String {
    let name = device.file_name().unwrap_or_default().to_string_lossy();
    if device
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with("usb"))
    {
        return "USB".into();
    }
    if name.starts_with("nvme") {
        return "SSD NVMe".into();
    }
    if name.starts_with("mmcblk") {
        return if attribute(device, "device/type").as_deref() == Some("MMC") {
            "eMMC".into()
        } else {
            "SD/MMC".into()
        };
    }
    if device
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with("virtio"))
    {
        return "Virtual".into();
    }
    match attribute(device, "queue/rotational").as_deref() {
        Some("0") => "SSD".into(),
        Some("1") => "HDD".into(),
        _ => text.type_unavailable().into(),
    }
}

fn format_gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / 1_073_741_824.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::os::unix::{ffi::OsStrExt, fs::symlink};

    #[test]
    fn mountinfo_decodes_escaped_paths_and_optional_fields() {
        let mounts = parse_mounts(
            "36 25 8:1 /home /home/my\\040files\\134dir rw shared:1 master:2 - ext4 /dev/disk\\040one rw\ninvalid\n",
        );
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].mount_point, Path::new("/home/my files\\dir"));
        assert_eq!(mounts[0].source, Path::new("/dev/disk one"));
        assert_eq!(unescape_path("/a\\011b\\012c"), Path::new("/a\tb\nc"));
        assert_eq!(unescape_path("/a\\999"), Path::new("/a\\999"));
        assert_eq!(unescape_path("/a\\377").as_os_str().as_bytes(), b"/a\xff");
    }

    #[test]
    fn personal_mount_uses_longest_component_prefix() {
        let mounts = parse_mounts(
            "1 0 8:1 / / rw - ext4 /dev/sda1 rw\n2 1 8:2 / /home rw - ext4 /dev/sda2 rw\n3 2 8:3 / /home/me rw - xfs /dev/sdb1 rw\n",
        );
        let selected = select_mounts(&mounts, Some(Path::new("/home/me/docs")));
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].0, "personal");
        assert_eq!(selected[0].1.device, "8:3");
        assert_eq!(selected[1].0, "sistema");
        assert_eq!(selected[1].1.device, "8:1");
        assert_eq!(
            mount_for_path(&mounts, Path::new("/home/metadata"))
                .unwrap()
                .device,
            "8:2"
        );
    }

    #[test]
    fn btrfs_subvolumes_share_one_filesystem_entry() {
        let mounts = parse_mounts(
            "1 0 0:36 /root / rw - btrfs /dev/mapper/fixture-volume rw\n2 1 0:37 /home /home rw - btrfs /dev/mapper/fixture-volume rw\n",
        );
        let selected = select_mounts(&mounts, Some(Path::new("/home/me")));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0, "personal");
    }

    #[test]
    fn overlay_and_memory_or_network_mounts_are_not_disks() {
        let mounts = parse_mounts(
            "1 0 0:40 / / rw - overlay composefs rw\n2 1 0:36 /home /var/home rw - btrfs /dev/mapper/fixture-volume rw\n",
        );
        let selected = select_mounts(&mounts, Some(Path::new("/var/home/me")));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].1.fs_type, "btrfs");
        for filesystem in ["tmpfs", "squashfs", "nfs", "nfs4", "cifs", "fuse.sshfs"] {
            let mounts = parse_mounts(&format!("1 0 0:1 / / rw - {filesystem} none rw\n"));
            assert!(select_mounts(&mounts, Some(Path::new("/home/me"))).is_empty());
        }
        let mounts = parse_mounts("1 0 252:0 / / rw - ext4 /dev/zram0 rw\n");
        assert!(select_mounts(&mounts, None).is_empty());
    }

    #[test]
    fn df_preserves_used_and_available_without_counting_reserved_as_used() {
        let usage = parse_usage("1B-blocks Used Avail\n1000 600 300\n").unwrap();
        assert_eq!(
            usage,
            Usage {
                total: 1000,
                used: 600,
                available: 300
            }
        );
        assert_ne!(usage.used, usage.total - usage.available);
        assert!(parse_usage("1B-blocks Used Avail\n0 0 0\n").is_none());
        assert!(parse_usage("1B-blocks Used Avail\n1000 - 300\n").is_none());
        assert!(parse_usage("1B-blocks Used Avail\n1000 600 300\n2000 600 300\n").is_none());
    }

    fn write_attribute(path: &Path, value: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }

    #[test]
    fn resolves_mapper_stacks_partitions_and_btrfs_virtual_device_numbers() {
        let temp = TempDir::new();
        let sys = temp.path.join("sys");
        let dev = temp.path.join("dev");
        let drive = sys.join("devices/pci0/nvme/nvme0/nvme0n1");
        let partition = drive.join("nvme0n1p3");
        let encrypted = sys.join("devices/virtual/block/dm-0");
        let logical = sys.join("devices/virtual/block/dm-1");
        write_attribute(&drive.join("device/model"), "  Fixture NVMe 1000  \n");
        write_attribute(&drive.join("size"), "1953125000\n");
        write_attribute(&partition.join("partition"), "3\n");
        write_attribute(&partition.join("size"), "1900000000\n");
        fs::create_dir_all(encrypted.join("slaves")).unwrap();
        fs::create_dir_all(logical.join("slaves")).unwrap();
        symlink(&partition, encrypted.join("slaves/nvme0n1p3")).unwrap();
        symlink(&encrypted, logical.join("slaves/dm-0")).unwrap();
        fs::create_dir_all(sys.join("class/block")).unwrap();
        symlink(&logical, sys.join("class/block/dm-1")).unwrap();
        fs::create_dir_all(dev.join("mapper")).unwrap();
        fs::write(dev.join("dm-1"), "").unwrap();
        symlink("../dm-1", dev.join("mapper/home")).unwrap();
        let mount = parse_mounts("1 0 0:36 /home /home rw - btrfs /dev/mapper/home rw\n").remove(0);
        let devices = physical_devices(&mount, &sys, &dev);
        assert_eq!(devices.as_slice(), std::slice::from_ref(&drive));
        assert_eq!(
            describe_device(&drive, Translator::new(crate::i18n::Language::Spanish)),
            "Fixture NVMe 1000 · SSD NVMe · 1.00 TB físicos"
        );
    }

    #[test]
    fn multi_device_btrfs_deduplicates_physical_drives() {
        let temp = TempDir::new();
        let sys = temp.path.join("sys");
        let first = sys.join("devices/pci0/block/sda");
        let second = sys.join("devices/pci0/block/sdb");
        let first_partition = first.join("sda1");
        let second_partition = second.join("sdb1");
        write_attribute(&first_partition.join("partition"), "1\n");
        write_attribute(&second_partition.join("partition"), "1\n");
        fs::create_dir_all(sys.join("dev/block")).unwrap();
        symlink(&first_partition, sys.join("dev/block/8:1")).unwrap();
        let volume_devices = sys.join("fs/btrfs/fixture-volume/devices");
        fs::create_dir_all(&volume_devices).unwrap();
        symlink(&first_partition, volume_devices.join("sda1")).unwrap();
        symlink(&second_partition, volume_devices.join("sdb1")).unwrap();
        symlink(&first_partition, volume_devices.join("duplicate")).unwrap();
        let mount = parse_mounts("1 0 8:1 / / rw - btrfs /dev/sda1 rw\n").remove(0);
        assert_eq!(
            physical_devices(&mount, &sys, &temp.path.join("dev")),
            [first, second]
        );
    }

    #[test]
    fn usb_rotational_flag_does_not_claim_a_flash_drive_is_hdd() {
        let temp = TempDir::new();
        let drive = temp.path.join("usb1/block/sda");
        write_attribute(&drive.join("queue/rotational"), "1\n");
        assert_eq!(
            device_type(&drive, Translator::new(crate::i18n::Language::Spanish)),
            "USB"
        );
        let sata = temp.path.join("pci0/block/sdb");
        write_attribute(&sata.join("queue/rotational"), "1\n");
        assert_eq!(
            device_type(&sata, Translator::new(crate::i18n::Language::Spanish)),
            "HDD"
        );
        write_attribute(&sata.join("queue/rotational"), "0\n");
        assert_eq!(
            device_type(&sata, Translator::new(crate::i18n::Language::Spanish)),
            "SSD"
        );
        let emmc = temp.path.join("mmc_host/mmc0/mmcblk0");
        write_attribute(&emmc.join("device/type"), "MMC\n");
        assert_eq!(
            device_type(&emmc, Translator::new(crate::i18n::Language::Spanish)),
            "eMMC"
        );
    }
}
