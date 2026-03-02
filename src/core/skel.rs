use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum SizeUnit {
    Bytes(u64),      
    Sectors(u64),
    Kilobytes(u64),
    Megabytes(u64),
    Gigabytes(u64),
    Terabytes(u64),
    Petabytes(u64),
    Exabytes(u64),
    Percentage(u8),
    Extents(u64),
}

#[derive(Debug, Clone)]
pub enum Filesystem {
    Xfs,
    Ext4,
    Btrfs,
    Vfat,
    Swap,
    F2FS,
    Ntfs,
    Exfat,
}

pub struct LvRequest {
    pub name: String,
    pub size: SizeUnit,
    pub fs: Option<Filesystem>,
    pub mount_path: Option<PathBuf>,
}

pub enum Command {
    Provision {
        pvs: Vec<PathBuf>,
        vg_name: String,
        pe_size: SizeUnit,
        lvs: Vec<LvRequest>,
    }
}

pub struct Action {
    pub command: Command,
    pub auto_confirm: bool,
}

pub enum Call {
    PvCreate(PathBuf),
    VgCreate { name: String, pvs: Vec<PathBuf>, pe_size: SizeUnit },
    LvCreate { vg: String, name: String, size: SizeUnit },
    Mkfs { device: PathBuf, fs: Filesystem },
    MkSwap(PathBuf),
    Mkdir(PathBuf),
    Mount { device: PathBuf, path: PathBuf },
    PartitionDisk(PathBuf),
    WipeSignatures(PathBuf),
}
