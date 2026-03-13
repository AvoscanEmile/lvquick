use std::path::{Path, PathBuf};
use std::process::Command;
use crate::core::{Call, Draft, DraftStatus, SizeUnit, PercentTarget};
use std::collections::{HashMap, HashSet};

fn probe_pv_exists(path: &Path) -> bool {
    let target_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    
    Command::new("pvs")
        .args(["--reportformat", "json", target_path.to_str().unwrap_or_default()])
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

fn probe_swap_active(path: &Path) -> bool {
    if let Ok(swaps) = std::fs::read_to_string("/proc/swaps") {
        let path_str = path.to_str().unwrap_or_default();
        swaps.lines().any(|line| line.contains(path_str))
    } else {
        false
    }
}

fn probe_fstab_exists(device: &Path, mount_path: &Path) -> bool {
    let fstab = std::fs::read_to_string("/etc/fstab").unwrap_or_default();
    
    let mnt_str = mount_path.to_str().unwrap_or_default();
    if !mnt_str.is_empty() && mnt_str != "none" {
        if fstab.lines().any(|l| !l.starts_with('#') && l.contains(mnt_str)) {
            return true;
        }
    }

    if let Ok(output) = Command::new("blkid").args(["-s", "UUID", "-o", "value", device.to_str().unwrap_or_default()]).output() {
        let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !uuid.is_empty() && fstab.lines().any(|l| !l.starts_with('#') && l.contains(&uuid)) {
            return true;
        }
    }

    let dev_str = device.to_str().unwrap_or_default();
    fstab.lines().any(|l| !l.starts_with('#') && l.contains(dev_str))
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

fn probe_is_full_disk(path: &Path) -> bool {
    Command::new("lsblk")
        .args(["-n", "-d", "-o", "TYPE", path.to_str().unwrap_or_default()])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            let dev_type = out.trim();
            !dev_type.is_empty() && dev_type != "part"
        })
        .unwrap_or(false)
}

#[derive(Debug, Default, Clone)]
pub struct SystemState {
  pub pvs: HashSet<PathBuf>,
  pub vgs: HashSet<String>,
  pub lvs: HashSet<(String, String)>,
  pub mounts: HashSet<PathBuf>,
  pub swaps: HashSet<PathBuf>,
  pub fstabs: HashSet<(PathBuf, PathBuf)>,
  pub filesystems: HashSet<PathBuf>,
  pub paths_exist: HashSet<PathBuf>,
  pub is_full_disk: HashSet<PathBuf>,
  pub block_device_sizes: HashMap<PathBuf, u64>,
  pub fstab_device_refs: HashSet<PathBuf>,
}

impl SystemState {
    pub fn from_draft(draft: &Draft) -> Self {
        let mut state = Self::default();
        for call in &draft.draft {
            match call {
                Call::PvCreate(path) => {
                    if probe_pv_exists(path) { state.pvs.insert(path.clone()); }
                    if probe_fs_exists(path) { state.filesystems.insert(path.clone()); }
                    if probe_fstab_exists(path, Path::new("")) { state.fstab_device_refs.insert(path.clone()); }
                }
                Call::VgCreate { name, pvs, pe_size: _ } => {
                    if probe_vg_exists(name) { 
                        state.vgs.insert(name.clone()); 
                    }

                    for pv in pvs {
                        if pv.exists() { 
                            state.paths_exist.insert(pv.clone()); 
                        }
                        if let Ok(size) = probe_block_device_size(pv) {
                            state.block_device_sizes.insert(pv.clone(), size);
                        }
                        if probe_is_full_disk(pv) { 
                            state.is_full_disk.insert(pv.clone()); 
                        }
                        if probe_fs_exists(pv) { 
                            state.filesystems.insert(pv.clone()); 
                        }
                        if probe_fstab_exists(pv, Path::new("")) { 
                            state.fstab_device_refs.insert(pv.clone()); 
                        }
                    }
                }
                Call::LvCreate { vg, name, .. } => if probe_lv_exists(vg, name) { state.lvs.insert((vg.clone(), name.clone())); },
                Call::Mount { device: _, path } => if probe_mount_exists(path) { state.mounts.insert(path.clone()); },
                Call::Fstab { device, path, .. } => if probe_fstab_exists(device, path) { state.fstabs.insert((device.clone(), path.clone())); },
                Call::MkSwap(device) => if probe_swap_active(device) { state.swaps.insert(device.clone()); },
                Call::Mkfs { device, .. } => if probe_fs_exists(device) { state.filesystems.insert(device.clone()); },
                Call::Mkdir(path) => if path.exists() { state.paths_exist.insert(path.clone()); },
            }
        }
        state
    }
}

pub fn verify_done(draft: &mut Draft, state: &SystemState) -> Result<(), String> {
    let mut matched_calls = 0;
    let mut total_calls = draft.draft.len();

    for call in &draft.draft {
        match call {
            Call::PvCreate(path) => if state.pvs.contains(path) { matched_calls += 1; },
            Call::VgCreate { name, .. } => if state.vgs.contains(name) { matched_calls += 1; },
            Call::LvCreate { vg, name, .. } => if state.lvs.contains(&(vg.clone(), name.clone())) { matched_calls += 1; },
            Call::Mount { path, .. } => if state.mounts.contains(path) { matched_calls += 1; },
            Call::Fstab { device, path, .. } => if state.fstabs.contains(&(device.clone(), path.clone())) { matched_calls += 1; },
            Call::MkSwap(device) => if state.swaps.contains(device) { matched_calls += 1; }
            Call::Mkfs { device, .. } => if state.filesystems.contains(device) { total_calls -= 1; }
            Call::Mkdir(path) => if state.paths_exist.contains(path) { total_calls -= 1; }
        };
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

pub fn verify_safety(draft: &mut Draft, state: &SystemState) -> Result<(), String> {
    for call in &draft.draft {
        if let Call::PvCreate(path) = call {
            if state.is_full_disk.contains(path) {
                draft.warnings.push(format!("Targeting full disk {:?} (not a partition)...", path));
            }
            if state.filesystems.contains(path) {
                if state.fstab_device_refs.contains(path) {
                    draft.status = DraftStatus::Dirty;
                    return Err(format!("CRITICAL: Device {:?} referenced in fstab!", path));
                } else {
                    draft.warnings.push(format!("Device {:?} contains an existing signature...", path));
                }
            }
        }
    }
    Ok(())
}

pub fn calculate_capacity(draft: &mut Draft, state: &SystemState) -> Result<(u128, u128), String> {
    let mut usable_extents = 0;
    let mut pe_size_bytes = 0;

    for call in &draft.draft {
        if let Call::VgCreate { pvs, pe_size, .. } = call {
            pe_size_bytes = pe_size.to_bytes()?;
            for pv in pvs {
                if !state.paths_exist.contains(pv) {
                    draft.status = DraftStatus::Invalid;
                    return Err(format!("Hardware failure: Path {:?} does not exist.", pv));
                }
                let raw_size = *state.block_device_sizes.get(pv).ok_or_else(|| format!("Missing size for {:?}", pv))? as u128;
                let overhead: u128 = 1048576; // 1MB
                if raw_size <= overhead {
                    draft.status = DraftStatus::Invalid;
                    return Err(format!("Device {:?} too small.", pv));
                }
                usable_extents += (raw_size - overhead) / pe_size_bytes;
            }
        }
    }
    Ok((usable_extents, pe_size_bytes))
}

pub fn calculate_required(draft: &Draft, total_usable: u128, pe_bytes: u128) -> Result<u128, String> {
    let mut total_required = 0;
    for call in &draft.draft {
        if let Call::LvCreate { size, .. } = call {
            let required = match size {
                SizeUnit::Extents(e) => *e as u128,
                SizeUnit::Percentage(pct, target) => {
                    let p = pct.get() as u128;
                    match target {
                        PercentTarget::Vg | PercentTarget::Pvs => (total_usable * p) / 100,
                        PercentTarget::Free => ((total_usable.saturating_sub(total_required)) * p) / 100
                    }
                },
                _ => (size.to_bytes()? + pe_bytes - 1) / pe_bytes
            };
            total_required += required;
        }
    }
    Ok(total_required)
}

pub fn verify_uniqueness(draft: &Draft) -> Result<(), String> {
    let mut seen_lvs = HashSet::new();
    let mut seen_pvs = HashSet::new();

    for call in &draft.draft {
        match call {
            Call::PvCreate(path) => {
                if !seen_pvs.insert(path) {
                    return Err(format!("Duplicate PV declaration: {:?}", path));
                }
            }
            Call::LvCreate { vg, name, .. } => {
                if !seen_lvs.insert((vg, name)) {
                    return Err(format!("Duplicate LV name '{}' in VG '{}'", name, vg));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn verify_possible(draft: &mut Draft, state: &SystemState) -> Result<(), String> {
    let validation_result = (|| {
        if draft.status != DraftStatus::Clean {
            return Err("Cannot run capability check on a non-clean draft.".to_string());
        }

        verify_uniqueness(draft)?;
        verify_safety(draft, state)?;

        let (usable, pe_bytes) = calculate_capacity(draft, state)?;
        let required = calculate_required(draft, usable, pe_bytes)?;

        if usable < required {
            return Err(format!(
                "Insufficient disk space. Required {}, available {}.",
                required, usable
            ));
        }
        
        Ok(())
    })();

    match validation_result {
        Ok(_) => {
            draft.status = DraftStatus::Ready;
            Ok(())
        }
        Err(e) => {
            draft.status = DraftStatus::Invalid;
            Err(e)
        }
    }
}

pub fn verify_provision(mut draft: Draft) -> Result<Draft, String> {
    let state = SystemState::from_draft(&draft);
    verify_done(&mut draft, &state)?;

    match draft.status {
        DraftStatus::Done => return Ok(draft), // Main exits 0
        DraftStatus::Dirty => return Err("System is in a Dirty state. Manual intervention required.".into()), // Main exits 4
        DraftStatus::Clean => {
            verify_possible(&mut draft, &state)?;

            match draft.status {
                DraftStatus::Ready => Ok(draft), // Main proceeds to Confirmation
                DraftStatus::Invalid => Err("System cannot fulfill this plan. Invalid hardware or math.".into()), // Main exits 1
                _ => Err("Architectural Error: Unexpected state after Pass 2.".into()),
            }
        }
        _ => Err("Architectural Error: Unexpected state after Pass 1.".into()),
    }
}
