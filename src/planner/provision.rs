use std::path::PathBuf;
use crate::core::{Call, Filesystem, LvRequest};

pub fn plan_provision(
    pvs: Vec<PathBuf>, 
    vg_name: String, 
    pe_size: crate::core::SizeUnit, 
    lvs: Vec<LvRequest>
) -> Result<Vec<Call>, String> {
    let mut plan = Vec::new();

    for pv in pvs.clone() {
        plan.push(Call::PvCreate(pv));
    }

    plan.push(Call::VgCreate {
        name: vg_name.clone(),
        pvs,
        pe_size,
    });

    for lv in lvs {
        let lv_name = lv.name.clone();
        let device_path = PathBuf::from(format!("/dev/{}/{}", vg_name, lv_name));

        // Create the LV
        plan.push(Call::LvCreate {
            vg: vg_name.clone(),
            name: lv_name,
            size: lv.size,
        });

        if let Some(fs_mount) = lv.fs {
            // Format the device
            match fs_mount.fs {
                Filesystem::Swap => {
                    plan.push(Call::MkSwap(device_path.clone()));
                }
                _ => {
                    plan.push(Call::Mkfs {
                        device: device_path.clone(),
                        fs: fs_mount.fs,
                    });
                }
            }

            if let Some(path) = fs_mount.mount_path {
                plan.push(Call::Mkdir(path.clone()));
                plan.push(Call::Mount {
                    device: device_path,
                    path,
                });
            }
        }
    }

    Ok(plan)
}
