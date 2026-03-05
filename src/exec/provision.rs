use crate::core::{Call, Draft, SizeUnit, PercentTarget, Exec};

pub fn exec_provision(draft: Draft) -> Result<Exec, String> {
    let mut command_list = Vec::new();

    for call in &draft.draft {
        let cmd_string = match call {
            Call::PvCreate(path) => {
                format!("pvcreate -y {:?}", path)
            }
            Call::VgCreate { name, pvs, pe_size } => {
                let pvs_str: Vec<String> = pvs.iter().map(|p| format!("{:?}", p)).collect();
                // PE size is converted to bytes for precision in the shell command
                format!("vgcreate -s {}B {} {}", pe_size.to_bytes()?, name, pvs_str.join(" "))
            }
            Call::LvCreate { vg, name, size } => {
                match size {
                    SizeUnit::Percentage(pct, target) => {
                        let t = match target {
                            PercentTarget::Free => "FREE",
                            PercentTarget::Vg => "VG",
                            PercentTarget::Pvs => "PVS",
                        };
                        format!("lvcreate -y -n {} -l {}%{} {}", name, pct.get(), t, vg)
                    }
                    SizeUnit::Extents(e) => {
                        format!("lvcreate -y -n {} -l {} {}", name, e, vg)
                    }
                    _ => {
                        format!("lvcreate -y -n {} -L {}B {}", name, size.to_bytes()?, vg)
                    }
                }
            }
            Call::Mkfs { device, fs } => {
                let fs_name = format!("{:?}", fs).to_lowercase();
                format!("mkfs -t {} {:?}", fs_name, device)
            }
            Call::MkSwap(device) => format!("mkswap {:?}", device),
            Call::Mkdir(path) => format!("mkdir -p {:?}", path),
            Call::Mount { device, path } => format!("mount {:?} {:?}", device, path),
        };
        command_list.push(cmd_string);
    }

    Ok(Exec {
        list: command_list,
        auto_confirm: draft.auto_confirm,
        is_allowed: false,
    })
}
