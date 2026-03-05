use std::path::{Path};
use std::process::Command;
use crate::core::{Call, Draft, DraftStatus};

fn probe_pv_exists(path: &Path) -> bool {
    Command::new("pvs")
        .args(["--reportformat", "json", path.to_str().unwrap_or_default()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_vg_exists(name: &str) -> bool {
    Command::new("vgs")
        .args(["--reportformat", "json", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_lv_exists(vg: &str, name: &str) -> bool {
    let lv_path = format!("{}/{}", vg, name);
    Command::new("lvs")
        .args(["--reportformat", "json", &lv_path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_fs_exists(path: &Path) -> bool {
    Command::new("blkid")
        .arg(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn probe_mount_exists(target_path: &Path) -> bool {
    if let Ok(mounts) = std::fs::read_to_string("/proc/mounts") {
        let path_str = target_path.to_str().unwrap_or_default();
        mounts.lines().any(|line| line.contains(path_str))
    } else {
        false
    }
}

fn probe_block_device_size(path: &Path) -> Result<u64, String> {
    let output = Command::new("lsblk")
        .args(["-b", "-n", "-o", "SIZE", path.to_str().unwrap_or_default()])
        .output()
        .map_err(|e| format!("Failed to execute lsblk: {}", e))?;

    if !output.status.success() {
        return Err(format!("lsblk failed for device {:?}", path));
    }

    let size_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    size_str.parse::<u64>().map_err(|_| format!("Failed to parse size: {}", size_str))
}

fn verify_done(draft: &mut Draft) -> Result<(), String> {
    let mut matched_calls = 0;
    let total_calls = draft.draft.len();

    for call in &draft.draft {
        let matched = match call {
            Call::PvCreate(path) => probe_pv_exists(path),
            Call::VgCreate { name, .. } => probe_vg_exists(name),
            Call::LvCreate { vg, name, .. } => probe_lv_exists(vg, name),
            Call::Mkfs { device, .. } | Call::MkSwap(device) => probe_fs_exists(device),
            Call::Mkdir(path) => path.exists(),
            Call::Mount { path, .. } => probe_mount_exists(path),
        };

        if matched {
            matched_calls += 1;
        }
    }

    if matched_calls == total_calls {
        draft.status = DraftStatus::Done;
    } else if matched_calls == 0 {
        draft.status = DraftStatus::Clean;
    } else {
        draft.status = DraftStatus::Dirty;
        return Err("Draft is in a dirty/partial state.".to_string());
    }

    Ok(())
}

fn verify_possible(draft: &mut Draft) -> Result<(), String> {
    if draft.status != DraftStatus::Clean {
        return Err("Cannot run capability check on a non-clean draft.".to_string());
    }

    let mut total_usable_extents: u128 = 0;
    let mut total_required_extents: u128 = 0;
    let mut pe_size_bytes: u128 = 0;

    for call in &draft.draft {
        if let Call::VgCreate { pvs, pe_size, .. } = call {
            pe_size_bytes = pe_size.to_bytes()?; 
            
            for pv in pvs {
                if !pv.exists() {
                    draft.status = DraftStatus::Invalid;
                    return Err(format!("Hardware failure: Path {:?} does not exist.", pv));
                }

                let raw_size = probe_block_device_size(pv)? as u128;
                let metadata_overhead: u128 = 1048576; // 1MB overhead
                
                if raw_size <= metadata_overhead {
                    draft.status = DraftStatus::Invalid;
                    return Err(format!("Device {:?} is too small for LVM metadata.", pv));
                }

                let usable_bytes = raw_size - metadata_overhead;
                total_usable_extents += usable_bytes / pe_size_bytes; 
            }
        }
    }

    for call in &draft.draft {
        if let Call::LvCreate { size, .. } = call {
            let required_extents = match size {
                crate::core::SizeUnit::Extents(e) => *e as u128,
                
                crate::core::SizeUnit::Percentage(pct, target) => {
                    let p = pct.get() as u128;
                    match target {
                        crate::core::PercentTarget::Vg | crate::core::PercentTarget::Pvs => {
                            (total_usable_extents * p) / 100
                        },
                        crate::core::PercentTarget::Free => {
                            let free_extents = total_usable_extents.saturating_sub(total_required_extents);
                            (free_extents * p) / 100
                        }
                    }
                },
                
                _ => {
                    let lv_bytes = size.to_bytes()?;
                    (lv_bytes + pe_size_bytes - 1) / pe_size_bytes
                }
            };

            total_required_extents += required_extents;
        }
    }

    if total_usable_extents >= total_required_extents {
        draft.status = DraftStatus::Ready;
        Ok(())
    } else {
        draft.status = DraftStatus::Invalid;
        Err(format!(
            "Validation Failure: Insufficient disk space. Required {} extents, but only {} available.",
            total_required_extents, total_usable_extents
        ))
    }
}

pub fn verify_provision(mut draft: Draft) -> Result<Draft, String> {
    verify_done(&mut draft)?;

    match draft.status {
        DraftStatus::Done => return Ok(draft), // Main exits 0
        DraftStatus::Dirty => return Err("System is in a Dirty state. Manual intervention required.".into()), // Main exits 4
        DraftStatus::Clean => {
            verify_possible(&mut draft)?;

            match draft.status {
                DraftStatus::Ready => Ok(draft), // Main proceeds to Confirmation
                DraftStatus::Invalid => Err("System cannot fulfill this plan. Invalid hardware or math.".into()), // Main exits 1
                _ => Err("Architectural Error: Unexpected state after Pass 2.".into()),
            }
        }
        _ => Err("Architectural Error: Unexpected state after Pass 1.".into()),
    }
}
