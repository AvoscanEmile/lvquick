use std::path::PathBuf;
use std::str::FromStr;
use std::fmt;

#[cfg_attr(kani, derive(kani::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidPercentage(u8);                                                                                                
impl ValidPercentage {                                                                                
    pub fn new(val: u8) -> Result<Self, String> {                                                         
        match val {
          1..=100 => Ok(Self(val)),                                                                           
          _ => Err(format!("Percentage must be between 1 and 100, got {}%", val)),              
    }                                                                                             
  }                                                                                                                                  
  pub fn get(&self) -> u8 {                                                                         
      self.0                                                                                        
  }                                                                                                 
}

#[cfg_attr(kani, derive(kani::Arbitrary))]   
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentTarget {
    Free,
    Vg, 
    Pvs,  
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SizeUnit {
    Bytes(u64),      
    Sectors(u64),
    Kilobytes(u64),
    Megabytes(u64),
    Gigabytes(u64),
    Terabytes(u64),
    Petabytes(u64),
    Exabytes(u64),
    Percentage(ValidPercentage, PercentTarget),
    Extents(u64),
}

impl FromStr for SizeUnit {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_uppercase();
        if s.contains('%') {
            Self::parse_percentage(&s)
        } else {
            Self::parse_absolute(&s)
        }
    }
}

impl fmt::Display for SizeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizeUnit::Bytes(n) => write!(f, "{}B", n),
            SizeUnit::Kilobytes(n) => write!(f, "{}K", n),
            SizeUnit::Megabytes(n) => write!(f, "{}M", n),
            SizeUnit::Gigabytes(n) => write!(f, "{}G", n),
            SizeUnit::Terabytes(n) => write!(f, "{}T", n),
            SizeUnit::Petabytes(n) => write!(f, "{}P", n),
            SizeUnit::Exabytes(n) => write!(f, "{}E", n),
            SizeUnit::Sectors(n) => write!(f, "{}s", n),
            SizeUnit::Extents(n) => write!(f, "{}", n), 
            SizeUnit::Percentage(p, target) => {
                let suffix = match target {
                    PercentTarget::Free => "FREE",
                    PercentTarget::Vg => "VG",
                    PercentTarget::Pvs => "PVS",
                };
                write!(f, "{}%{}", p.get(), suffix)
            }
        }
    }
}

impl SizeUnit {
    fn parse_percentage(s: &str) -> Result<Self, String> {
        let percent_idx = s.find('%').ok_or("Missing '%' in percentage string")?;
        let num_part = &s[..percent_idx];
        let suffix_part = &s[percent_idx..];

        let target = match suffix_part {
            "%FREE" => PercentTarget::Free,
            "%VG"   => PercentTarget::Vg,
            "%PVS"  => PercentTarget::Pvs,
            _ => return Err(format!("Unsupported percentage target: '{}'", suffix_part)),
        };

        let raw_val = num_part.parse::<u8>()
            .map_err(|_| "Invalid percentage number".to_string())?;
        let valid_percent = ValidPercentage::new(raw_val)?;

        Ok(SizeUnit::Percentage(valid_percent, target))
    }

    fn parse_absolute(s: &str) -> Result<Self, String> {
        let split_idx = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        
        if split_idx == 0 {
            return Err(format!("No numeric value found in size: '{}'", s));
        }

        let val: u64 = s[..split_idx].parse()
            .map_err(|_| "Invalid numeric value".to_string())?;
        let unit = &s[split_idx..];

        match unit {
            "B" => Ok(SizeUnit::Bytes(val)),
            "K" | "KB" => Ok(SizeUnit::Kilobytes(val)),
            "M" | "MB" => Ok(SizeUnit::Megabytes(val)),
            "G" | "GB" => Ok(SizeUnit::Gigabytes(val)),
            "T" | "TB" => Ok(SizeUnit::Terabytes(val)),
            "P" | "PB" => Ok(SizeUnit::Petabytes(val)),
            "E" | "EB" => Ok(SizeUnit::Exabytes(val)),
            "S" => Ok(SizeUnit::Sectors(val)),
            "" => Ok(SizeUnit::Extents(val)),
            _ => Err(format!("Unknown size unit: '{}'", unit)),
        }
    }

    pub fn to_bytes(&self) -> Result<u128, String> {
        match self {
            SizeUnit::Bytes(b)     => Ok(*b as u128),
            SizeUnit::Sectors(s)   => Ok((*s as u128) * 512),
            SizeUnit::Kilobytes(k) => Ok((*k as u128) * 1024),
            SizeUnit::Megabytes(m) => Ok((*m as u128) * 1_048_576),
            SizeUnit::Gigabytes(g) => Ok((*g as u128) * 1_073_741_824),
            SizeUnit::Terabytes(t) => Ok((*t as u128) * 1_099_511_627_776),
            SizeUnit::Petabytes(p) => Ok((*p as u128) * 1_125_899_906_842_624),
            SizeUnit::Exabytes(e)  => Ok((*e as u128) * 1_152_921_504_606_846_976),
            SizeUnit::Percentage(_, _) => Err(
                "Cannot calculate raw bytes from a Percentage without Volume Group context.".into()
            ),
            SizeUnit::Extents(_) => Err(
                "Cannot calculate raw bytes from Extents without knowing the Physical Extent (PE) size.".into()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl FromStr for Filesystem {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
          "xfs" => Ok(Filesystem::Xfs),
          "ext4" => Ok(Filesystem::Ext4),
          "btrfs" => Ok(Filesystem::Btrfs),
          "vfat" => Ok(Filesystem::Vfat),
          "swap" => Ok(Filesystem::Swap),
          "f2fs" => Ok(Filesystem::F2FS),
          "ntfs" => Ok(Filesystem::Ntfs),
          "exfat" => Ok(Filesystem::Exfat),
          _ => Err(format!("Unsupported or unknown filesystem: '{}'", s)),
        }
    }
}

impl fmt::Display for Filesystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Filesystem::Xfs => "xfs",
            Filesystem::Ext4 => "ext4",
            Filesystem::Btrfs => "btrfs",
            Filesystem::Vfat => "vfat",
            Filesystem::Swap => "swap",
            Filesystem::F2FS => "f2fs",
            Filesystem::Ntfs => "ntfs",
            Filesystem::Exfat => "exfat",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsMount {
    pub fs: Filesystem,
    pub mount_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvRequest {
    pub name: String,
    pub size: SizeUnit,
    pub fs: Option<FsMount>,
}

impl FromStr for LvRequest {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();

        if parts.len() < 2 {
            return Err(format!("Invalid format '{}'. Min: name:size", s));
        }
        if parts.len() > 4 {
            return Err(format!("Invalid format '{}'. Max: name:size:fs:mount", s));
        }

        
        let name = parts[0].to_string();
        if name.is_empty() || name.starts_with('-') || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return Err(format!("Invalid LV name: '{}'", name));
        }

        let fs_part = parts.get(2).unwrap_or(&"");
        let mount_part = parts.get(3).unwrap_or(&"");

        if fs_part.is_empty() && !mount_part.is_empty() {
            return Err(format!("Provided a mountpath '{}' but no filesystem.", mount_part));
        }

        let size = SizeUnit::from_str(parts[1])?;

        let fs = if !fs_part.is_empty() {
            let fs_type = Filesystem::from_str(fs_part)?;
            let mount_path = if !mount_part.is_empty() {
                Some(PathBuf::from(mount_part))
            } else {
                None
            };
            Some(FsMount { fs: fs_type, mount_path })
        } else {
            None
        };

        Ok(LvRequest { name, size, fs })
    }
}

impl fmt::Display for LvRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.size)?;

        if let Some(fs_mount) = &self.fs {
            write!(f, ":{}", fs_mount.fs)?;
            if let Some(path) = &fs_mount.mount_path {
                write!(f, ":{}", path.display())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Command {
    Provision {
        pvs: Vec<PathBuf>,
        vg_name: String,
        pe_size: SizeUnit,
        lvs: Vec<LvRequest>,
    }
}

#[derive(Debug)]
pub struct Action {
    pub command: Command,
    pub auto_confirm: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Call {
    PvCreate(PathBuf),
    VgCreate { name: String, pvs: Vec<PathBuf>, pe_size: SizeUnit },
    LvCreate { vg: String, name: String, size: SizeUnit },
    Mkfs { device: PathBuf, fs: Filesystem },
    MkSwap(PathBuf),
    Mkdir(PathBuf),
    Mount { device: PathBuf, path: PathBuf },
    Fstab { device: PathBuf, path: PathBuf, fs: Filesystem},
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftStatus {
    Pending,
    Done,
    Clean,
    Ready,
    Dirty,
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Draft {
    pub auto_confirm: bool, 
    pub draft_type: String,
    pub draft: Vec<Call>,
    pub status: DraftStatus, 
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct Exec {
    pub list: Vec<String>,
    pub auto_confirm: bool,
    pub is_allowed: bool, 
    pub warnings: Vec<String>, 
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
