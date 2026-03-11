use proptest::prelude::*;
use super::*;
use std::str::FromStr;

// Helper for  FsMount
fn arb_fs_mount() -> impl Strategy<Value = FsMount> {
    (arb_filesystem(), prop::option::of(arb_unix_path())).prop_map(|(fs, mount_path)| {
        FsMount { fs, mount_path }
    })
}

// Strategy for the full LvRequest
fn arb_lv_request() -> impl Strategy<Value = LvRequest> {
    (
        arb_lv_name(),
        arb_size_unit(),
        prop::option::of(arb_fs_mount())
    ).prop_map(|(name, size, fs)| {
        LvRequest { name, size, fs }
    })
}

// Helper for random Unix-like paths
fn arb_unix_path() -> impl Strategy<Value = PathBuf> {
    "/[a-z]{1,5}(/[a-z]{1,5}){0,2}".prop_map(PathBuf::from)
}

fn arb_size_unit() -> impl Strategy<Value = SizeUnit> {
    prop_oneof![
        any::<u64>().prop_map(SizeUnit::Bytes),
        any::<u64>().prop_map(SizeUnit::Sectors),
        any::<u64>().prop_map(SizeUnit::Kilobytes),
        any::<u64>().prop_map(SizeUnit::Megabytes),
        any::<u64>().prop_map(SizeUnit::Gigabytes),
        any::<u64>().prop_map(SizeUnit::Terabytes),
        any::<u64>().prop_map(SizeUnit::Petabytes),
        any::<u64>().prop_map(SizeUnit::Exabytes),
        any::<u64>().prop_map(SizeUnit::Extents),
        (1..=100u8, prop_oneof![
            Just(PercentTarget::Free),
            Just(PercentTarget::Vg),
            Just(PercentTarget::Pvs)
        ]).prop_map(|(p, t)| SizeUnit::Percentage(ValidPercentage::new(p).unwrap(), t)),
    ]
}

fn arb_filesystem() -> impl Strategy<Value = Filesystem> {
    prop_oneof![
        Just(Filesystem::Xfs),
        Just(Filesystem::Ext4),
        Just(Filesystem::Btrfs),
        Just(Filesystem::Vfat),
        Just(Filesystem::Swap),
        Just(Filesystem::F2FS),
        Just(Filesystem::Ntfs),
        Just(Filesystem::Exfat),
    ]
}

prop_compose! {
    // Generates valid LV names based on LVM rules
    fn arb_lv_name()(s in "[a-zA-Z0-9_\\.][a-zA-Z0-9_\\.-]{0,20}") -> String {
        s
    }
}

// Proptest harness
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100000))]
    
    #[test]
    fn test_parse_percentage_valid_cases(
        val in 0..=255u8, 
        target_idx in 0..3usize,
        prefix in "[ \t]*", 
        suffix in "[ \t]*",
        is_lowercase in proptest::bool::ANY,
    ) {
        let targets = ["%FREE", "%VG", "%PVS"];
        let mut target_str = targets[target_idx].to_string();
        if is_lowercase {
            target_str = target_str.to_lowercase();
        }
        
        let input = format!("{}{}{}{}", prefix, val, target_str, suffix);
        let result = SizeUnit::from_str(&input);

        if (1..=100).contains(&val) {
            assert!(result.is_ok(), "Failed on valid input: '{}'", input);
            if let Ok(SizeUnit::Percentage(p, t)) = result {
                assert_eq!(p.get(), val);
                let expected_t = match target_idx {
                    0 => PercentTarget::Free,
                    1 => PercentTarget::Vg,
                    2 => PercentTarget::Pvs,
                    _ => unreachable!(),
                };
                assert_eq!(t, expected_t);
            }
        } else {
            assert!(result.is_err(), "Should have rejected value {}: '{}'", val, input);
        }
    }

    #[test]
    fn test_parse_percentage_chaos(
        s in "[a-zA-Z0-9 \t]*%[a-zA-Z0-9 \t]*"
    ) {
        let result = SizeUnit::from_str(&s);
        if result.is_ok() {
            assert!(matches!(result.unwrap(), SizeUnit::Percentage(_, _)));
        }
    }

    #[test]
    fn test_parse_absolute_valid(
        val in 0..u64::MAX,
        unit_idx in 0..16usize, // Covering all valid unit strings
        prefix in "[ \t]*",
        suffix in "[ \t]*",
        is_lowercase in proptest::bool::ANY,
    ) {
        let units = [
            "B", "K", "KB", "M", "MB", "G", "GB", 
            "T", "TB", "P", "PB", "EB", "S", "", "E"
        ];
        let mut unit_str = units[unit_idx % units.len()].to_string();
        if is_lowercase {
            unit_str = unit_str.to_lowercase();
        }

        let input = format!("{}{}{}{}", prefix, val, unit_str, suffix);
        let result = SizeUnit::from_str(&input);

        assert!(result.is_ok(), "Should parse valid absolute size: '{}'", input);
        
        // Detailed variant check
        match result.unwrap() {
            SizeUnit::Bytes(v) => assert_eq!(v, val),
            SizeUnit::Kilobytes(v) => assert_eq!(v, val),
            SizeUnit::Megabytes(v) => assert_eq!(v, val),
            SizeUnit::Gigabytes(v) => assert_eq!(v, val),
            SizeUnit::Terabytes(v) => assert_eq!(v, val),
            SizeUnit::Petabytes(v) => assert_eq!(v, val),
            SizeUnit::Exabytes(v) => assert_eq!(v, val),
            SizeUnit::Sectors(v) => assert_eq!(v, val),
            SizeUnit::Extents(v) => assert_eq!(v, val),
            SizeUnit::Percentage(_, _) => panic!("Should not be percentage"),
        }
    }

    #[test]
    fn test_parse_absolute_overflow(
        // Generates a string represention of a number larger than u64::MAX
        // (u64::MAX is 18,446,744,073,709,551,615)
        num_str in "1[0-9]{20}B" 
    ) {
        let result = SizeUnit::from_str(&num_str);
        // Should return an Error because .parse::<u64>() will overflow
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_absolute_junk(
        s in "[A-Z]{3,10}[0-9]{1,5}" // Reversed: letters before numbers
    ) {
        let result = SizeUnit::from_str(&s);
        // This hits the split_idx == 0 check or the parse error
        assert!(result.is_err());
    }

    #[test]
    fn test_from_str_dispatch(
        cmd in prop_oneof![
            // Valid Percentages (Contiguous)
            "[0-9]{1,3}%(FREE|VG|PVS|free|vg|pvs)",
            // Valid Absolute Sizes (Contiguous)
            "[0-9]{1,10}(B|K|KB|M|MB|G|GB|T|TB|P|PB|EB|S|E|b|k|kb|m|mb|g|gb|t|tb|p|pb|eb|s|e)",
            // Extents (Just numbers)
            "[0-9]{1,10}"
        ],
        padding_l in "[ \t\n\r]*",
        padding_r in "[ \t\n\r]*",
    ) {
        let input = format!("{}{}{}", padding_l, cmd, padding_r);
        let result = SizeUnit::from_str(&input);

        // Logic check: Since we generated a "valid" command, 
        // it should only fail if the number is > 100 for percentages.
        if input.contains('%') {
            // Extract the number to check the 1-100 range
            let num_part: u8 = cmd.split('%').next().unwrap().parse().unwrap_or(255);
            if (1..=100).contains(&num_part) {
                assert!(result.is_ok(), "Strict failure on: '{}'", input);
            } else {
                assert!(result.is_err());
            }
        } else {
            // Absolute sizes should always work for u64 in this regex range
            assert!(result.is_ok(), "Absolute failure on: '{}'", input);
        }
    }

    #[test]
    fn test_filesystem_from_str_exhaustive(
        (input, expected_variant) in prop_oneof![
            "[xX][fF][sS]".prop_map(|s| (s, Some(Filesystem::Xfs))),
            "[eE][xX][tT]4".prop_map(|s| (s, Some(Filesystem::Ext4))),
            "[bB][tT][rR][fF][sS]".prop_map(|s| (s, Some(Filesystem::Btrfs))),
            "[vV][fF][aA][tT]".prop_map(|s| (s, Some(Filesystem::Vfat))),
            "[sS][wW][aA][pP]".prop_map(|s| (s, Some(Filesystem::Swap))),
            "[fF]2[fF][sS]".prop_map(|s| (s, Some(Filesystem::F2FS))),
            "[nN][tT][fF][sS]".prop_map(|s| (s, Some(Filesystem::Ntfs))),
            "[eE][xX][fF][aA][tT]".prop_map(|s| (s, Some(Filesystem::Exfat))),
            "[a-zA-Z0-9]{6,15}".prop_map(|s| (s, None)),
            " xfs".prop_map(|s| (s, None)),
            "ntfs ".prop_map(|s| (s, None)),
        ]
    ) {
        let result = Filesystem::from_str(&input);

        match expected_variant {
            Some(expected) => {
                assert!(result.is_ok(), "Failed to parse valid FS: '{}'", input);
                assert_eq!(result.unwrap(), expected, "Mismatched variant for: '{}'", input);
            }
            None => {
                assert!(result.is_err(), "Strict Mode violation: accepted '{}'", input);
            }
        }
    }

    #[test]
    fn test_lv_request_from_str_integration(
        name in arb_lv_name(),
        // Reuse our logic for valid sizes/filesystems
        size_val in 1..100u8,
        fs_str in prop_oneof!["xfs", "ext4", "btrfs", "vfat", "ntfs"],
        path_str in "/[a-z]{1,5}/[a-z]{1,5}",
        // Determine which "mode" of the string to build
        mode in 0..4usize 
    ) {
        let size_str = format!("{}%FREE", size_val);
        
        // Build the input string based on the "mode"
        let input = match mode {
            0 => format!("{}:{}", name, size_str),               // Min: name:size
            1 => format!("{}:{}:", name, size_str),              // name:size: (Empty FS)
            2 => format!("{}:{}:{}", name, size_str, fs_str),    // name:size:fs (FS, no mount)
            3 => format!("{}:{}:{}:{}", name, size_str, fs_str, path_str), // Full
            _ => unreachable!(),
        };

        let result = LvRequest::from_str(&input);
        
        // --- Validation ---
        assert!(result.is_ok(), "Failed to parse valid structure: '{}'", input);
        let req = result.unwrap();

        assert_eq!(req.name, name);
        
        // Verify FS/Mount logic specifically
        match mode {
            0 | 1 => assert!(req.fs.is_none(), "Should have no FS for mode {}", mode),
            2 => {
                let fs_mount = req.fs.unwrap();
                assert_eq!(fs_mount.fs.to_string().to_lowercase(), fs_str);
                assert!(fs_mount.mount_path.is_none());
            },
            3 => {
                let fs_mount = req.fs.unwrap();
                assert_eq!(fs_mount.fs.to_string().to_lowercase(), fs_str);
                assert_eq!(fs_mount.mount_path.unwrap(), PathBuf::from(path_str));
            },
            _ => {}
        }
    }

    #[test]
    fn test_lv_request_structural_errors(
        name in arb_lv_name(),
        path in "/mnt/data"
    ) {
        // 1. The "Forbidden" state: name:size::path (Mount without FS)
        let forbidden = format!("{}:10G::{}", name, path);
        assert!(LvRequest::from_str(&forbidden).is_err());

        // 2. Too many parts
        let too_many = format!("{}:10G:xfs:/mnt:extra", name);
        assert!(LvRequest::from_str(&too_many).is_err());

        // 3. Invalid Name (Starting with hyphen)
        let bad_name = format!("-invalid:10G");
        assert!(LvRequest::from_str(&bad_name).is_err());
    }

    #[test]
    fn test_size_unit_reflexivity(unit in arb_size_unit()) {
        let serialized = unit.to_string();
        let deserialized = SizeUnit::from_str(&serialized);

        assert!(deserialized.is_ok(), "Failed to parse its own output: '{}'", serialized);
        assert_eq!(unit, deserialized.unwrap(), "Reflexivity failed for: '{}'", serialized);
    }

    #[test]
    fn test_filesystem_reflexivity(fs in arb_filesystem()) {
        let serialized = fs.to_string();
        let deserialized = Filesystem::from_str(&serialized);

        assert!(deserialized.is_ok(), "Filesystem failed to parse its own Display output: '{}'", serialized);
        assert_eq!(fs, deserialized.unwrap(), "Reflexivity mismatch for filesystem: '{}'", serialized);
    }

    #[test]
    fn test_lv_request_reflexivity(req in arb_lv_request()) {
        let serialized = req.to_string();
        let deserialized = LvRequest::from_str(&serialized);

        assert!(deserialized.is_ok(), "LvRequest failed to parse its own output: '{}'", serialized);
        let back = deserialized.unwrap();
        
        // Assert field-by-field to make debugging easier if it fails
        assert_eq!(req.name, back.name, "Name mismatch for: '{}'", serialized);
        assert_eq!(req.size, back.size, "Size mismatch for: '{}'", serialized);
        assert_eq!(req.fs.is_some(), back.fs.is_some(), "FS presence mismatch for: '{}'", serialized);
        
        if let (Some(orig_fs), Some(back_fs)) = (req.fs, back.fs) {
            assert_eq!(orig_fs.fs, back_fs.fs, "FS type mismatch for: '{}'", serialized);
            assert_eq!(orig_fs.mount_path, back_fs.mount_path, "Path mismatch for: '{}'", serialized);
        }
    }
}

// Validates that the ValidPercentage::new(val) behaves as intended. 
#[test]
fn valid_percentage_proof() {
    for val in 0..=255u8 {
        let res = ValidPercentage::new(val);
        if (1..=100).contains(&val) {
            assert!(res.is_ok(), "Failed on valid value: {}", val);
        } else {
            assert!(res.is_err(), "Accepted invalid value: {}", val);
        }
    }
    println!("Successfully verified all 256 possible u8 values.");
}
