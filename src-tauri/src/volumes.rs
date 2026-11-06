use serde::Serialize;

use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};

// GetDriveTypeW returns the documented Win32 drive type codes. The constants
// live in windows-sys' Win32_System_WindowsProgramming feature, which this
// crate does not enable, so the stable numeric values are declared here.
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    pub mount: String,
    pub label: String,
    pub file_system: String,
    pub total: u64,
    pub free: u64,
    /// fixed, removable, network, optical or other.
    pub kind: String,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn kind_of(drive_type: u32) -> &'static str {
    match drive_type {
        DRIVE_FIXED => "fixed",
        DRIVE_REMOVABLE => "removable",
        DRIVE_REMOTE => "network",
        DRIVE_CDROM => "optical",
        DRIVE_RAMDISK => "other",
        _ => "other",
    }
}

/// Lists local Windows volumes with capacity and free space.
///
/// Network and optical drives are skipped on purpose: querying a disconnected
/// mapped drive can block for seconds and would freeze the overview on launch.
pub fn list_volumes() -> Vec<VolumeInfo> {
    let mut volumes = Vec::new();
    let mask = unsafe { GetLogicalDrives() };

    for index in 0..26u32 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index as u8) as char;
        let mount = format!("{letter}:\\");
        let wide_mount = wide(&mount);
        let drive_type = unsafe { GetDriveTypeW(wide_mount.as_ptr()) };
        if drive_type == DRIVE_REMOTE || drive_type == DRIVE_CDROM {
            continue;
        }

        let mut free_to_caller: u64 = 0;
        let mut total: u64 = 0;
        let mut total_free: u64 = 0;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide_mount.as_ptr(),
                &mut free_to_caller,
                &mut total,
                &mut total_free,
            )
        };
        if ok == 0 || total == 0 {
            continue;
        }

        let mut label_buffer = vec![0u16; 261];
        let mut fs_buffer = vec![0u16; 64];
        let mut serial: u32 = 0;
        let mut max_component: u32 = 0;
        let mut fs_flags: u32 = 0;
        let info_ok = unsafe {
            GetVolumeInformationW(
                wide_mount.as_ptr(),
                label_buffer.as_mut_ptr(),
                label_buffer.len() as u32,
                &mut serial,
                &mut max_component,
                &mut fs_flags,
                fs_buffer.as_mut_ptr(),
                fs_buffer.len() as u32,
            )
        };

        let (label, file_system) = if info_ok != 0 {
            (
                String::from_utf16_lossy(&label_buffer)
                    .trim_matches('\0')
                    .trim()
                    .to_string(),
                String::from_utf16_lossy(&fs_buffer)
                    .trim_matches('\0')
                    .trim()
                    .to_string(),
            )
        } else {
            (String::new(), String::new())
        };

        volumes.push(VolumeInfo {
            mount,
            label,
            file_system,
            total,
            free: total_free,
            kind: kind_of(drive_type).to_string(),
        });
    }

    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));
    volumes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_ready_local_volumes_with_sane_figures() {
        let volumes = list_volumes();
        assert!(
            !volumes.is_empty(),
            "expected at least one ready local volume"
        );
        assert!(
            volumes.iter().any(|volume| volume.kind == "fixed"),
            "expected the system drive to be reported as fixed"
        );
        for volume in &volumes {
            assert!(
                volume.mount.ends_with(":\\"),
                "unexpected mount {}",
                volume.mount
            );
            assert!(volume.total > 0, "volume {} has no capacity", volume.mount);
            assert!(
                volume.free <= volume.total,
                "volume {} reports more free space than capacity",
                volume.mount
            );
            assert!(matches!(
                volume.kind.as_str(),
                "fixed" | "removable" | "network" | "optical" | "other"
            ));
        }
    }
}
