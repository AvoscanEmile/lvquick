use std::path::PathBuf;
use std::str::FromStr;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentTarget {
    Free,
    Vg, 
    Pvs,  
}

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
    Percentage(ValidPercentage, PercentTarget),
    Extents(u64),
}

impl FromStr for SizeUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_uppercase();
        
        // Handle LVM percentage edge cases
        if let Some(percent_idx) = s.find('%') {
            let num_part = &s[..percent_idx];
            let suffix_part = &s[percent_idx..]; // Includes the '%'

            // Parse the target
            let target = match suffix_part {
                "%FREE" => PercentTarget::Free,
                "%VG" => PercentTarget::Vg,
                "%PVS" => PercentTarget::Pvs,
                _ => return Err(format!("Unsupported percentage target: '{}'", suffix_part)),
            };

            // Parse and validate the numeric portion
            let raw_val = num_part.parse::<u8>().map_err(|_| "Invalid percentage number".to_string())?;
            let valid_percent = ValidPercentage::new(raw_val)?;

            return Ok(SizeUnit::Percentage(valid_percent, target));
        }

        // Handle absolute sizes (Bytes, Megabytes, etc.)
        let split_idx = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if split_idx == 0 {
            return Err(format!("No numeric value found in size: '{}'", s));
        }

        let val: u64 = s[..split_idx].parse().map_err(|_| "Invalid numeric value".to_string())?;
        let unit = &s[split_idx..];

        match unit {
            "B" => Ok(SizeUnit::Bytes(val)),
            "K" | "KB" => Ok(SizeUnit::Kilobytes(val)),
            "M" | "MB" => Ok(SizeUnit::Megabytes(val)),
            "G" | "GB" => Ok(SizeUnit::Gigabytes(val)),
            "T" | "TB" => Ok(SizeUnit::Terabytes(val)),
            "P" | "PB" => Ok(SizeUnit::Petabytes(val)),
            "EB" => Ok(SizeUnit::Exabytes(val)),
            "S" => Ok(SizeUnit::Sectors(val)),
            "" | "E" => Ok(SizeUnit::Extents(val)),
            _ => Err(format!("Unknown size unit: '{}'", unit)),
        }
    }
}

impl SizeUnit {
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

#[derive(Debug, Clone)]
pub struct FsMount {
    pub fs: Filesystem,
    pub mount_path: Option<PathBuf>,
}

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
        let size = SizeUnit::from_str(parts[1])?;

        // We only look for a mount path if a filesystem was successfully declared.
        let fs = if let Some(&fs_str) = parts.get(2) {
            if !fs_str.is_empty() {
                // FS exists: Parse it and look for the optional mount path
                let fs_type = Filesystem::from_str(fs_str)?;
                let mount_path = parts.get(3)
                    .filter(|val| !val.is_empty())
                    .map(PathBuf::from);

                Some(FsMount { fs: fs_type, mount_path })
            } else {
                None
            }
        } else {
            None
        };

        Ok(LvRequest { name, size, fs })
    }
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

#[derive(Debug, Clone)]
pub enum Call {
    PvCreate(PathBuf),
    VgCreate { name: String, pvs: Vec<PathBuf>, pe_size: SizeUnit },
    LvCreate { vg: String, name: String, size: SizeUnit },
    Mkfs { device: PathBuf, fs: Filesystem },
    MkSwap(PathBuf),
    Mkdir(PathBuf),
    Mount { device: PathBuf, path: PathBuf },
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
}

pub struct Exec {
    pub list: Vec<String>,
    pub auto_confirm: bool,
    pub is_allowed: bool, 
}
