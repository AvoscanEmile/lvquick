use proptest::prelude::*;
use std::path::PathBuf;
use crate::core::{Call, Draft, DraftStatus, SizeUnit, Filesystem, PercentTarget, ValidPercentage};
use crate::verifier::provision::{verify_done, SystemState, verify_safety, calculate_capacity, calculate_required, verify_uniqueness, verify_possible};
use std::collections::HashSet;

fn requirement_scenario_strategy() -> impl Strategy<Value = (Draft, u128, u128, u128)> {
    // 1. Setup constants for the test run
    let total_usable = 1000u128; // Total extents available in our "test" VG
    let pe_bytes = 4_194_304u128; // 4MB PE size

    // 2. Generate a sequence of SizeUnit instructions (1 to 10 LVs)
    prop::collection::vec(
        prop_oneof![
            // Fixed Extents (1..200)
            (1..200u64).prop_map(SizeUnit::Extents),
            // Percentage of VG (1..20%)
            (1..20u8).prop_map(|p| {SizeUnit::Percentage(ValidPercentage::new(p).unwrap(), PercentTarget::Vg)}),
            // Percentage of Free (1..50%)
            (1..50u8).prop_map(|p| {SizeUnit::Percentage(ValidPercentage::new(p).unwrap(), PercentTarget::Free)}),
            // Raw Bytes (rounded to roughly 1-10 PEs)
            (1..40_000_000u64).prop_map(SizeUnit::Bytes),
        ],
        1..10
    ).prop_map(move |units| {
        let mut draft_calls = Vec::new();
        let mut expected_total = 0u128;

        for (i, unit) in units.into_iter().enumerate() {
            // Calculate ground truth mirroring the fn logic
            let required = match &unit {
                SizeUnit::Extents(e) => *e as u128,
                SizeUnit::Percentage(pct, target) => {
                    let p = pct.get() as u128;
                    match target {
                        PercentTarget::Vg | PercentTarget::Pvs => (total_usable * p) / 100,
                        PercentTarget::Free => {
                            let free = total_usable.saturating_sub(expected_total);
                            (free * p) / 100
                        }
                    }
                },
                _ => {
                    // Standard ceil division for bytes: (bytes + pe - 1) / pe
                    let b = unit.to_bytes().unwrap_or(0);
                    (b as u128 + pe_bytes - 1) / pe_bytes
                }
            };

            expected_total += required;

            draft_calls.push(Call::LvCreate {
                vg: "test_vg".to_string(),
                name: format!("lv_{}", i),
                size: unit,
            });
        }

        let draft = Draft {
            draft: draft_calls,
            status: DraftStatus::Pending,
            auto_confirm: false,
            draft_type: "provision".to_string(),
            warnings: vec![],
        };

        (draft, total_usable, pe_bytes, expected_total)
    })
}

fn capacity_scenario_strategy() -> impl Strategy<Value = (Draft, SystemState, u128, u128)> {
    // 1. Generate a PE size (powers of 2: 1MB, 2MB, 4MB ... 128MB)
    let pe_size_strat = prop_oneof![
        Just(SizeUnit::Megabytes(1)),
        Just(SizeUnit::Megabytes(4)),
        Just(SizeUnit::Megabytes(16)),
        Just(SizeUnit::Megabytes(32)),
        Just(SizeUnit::Megabytes(128)),
    ];

    // 2. Generate 1 to 10 unique PVs with sizes between 2MB and 1TB
    // We use btree_set of IDs to guarantee unique paths
    (pe_size_strat, prop::collection::btree_set(0..100u32, 1..10)).prop_flat_map(|(pe_unit, ids)| {
        let pe_bytes = pe_unit.to_bytes().unwrap() as u128;
        
        // For each ID, generate a random size (2MB to ~1TB)
        let pv_sizes_strat = prop::collection::vec(2_000_000u64..1_000_000_000_000u64, ids.len());
        
        (Just(pe_unit), Just(ids), pv_sizes_strat, Just(pe_bytes))
    }).prop_map(|(pe_unit, ids, sizes, pe_bytes)| {
        let mut state = SystemState::default();
        let mut pvs = Vec::new();
        let mut total_expected_extents: u128 = 0;
        let overhead: u128 = 1048576; // 1MB

        for (id, size_bytes) in ids.into_iter().zip(sizes.into_iter()) {
            let path = PathBuf::from(format!("/dev/vd{}", id));
            
            // Populate State
            state.paths_exist.insert(path.clone());
            state.block_device_sizes.insert(path.clone(), size_bytes);
            pvs.push(path.clone());

            // Calculate Ground Truth Extents for this PV
            // (size - 1MB) / pe_size
            let usable = (size_bytes as u128).saturating_sub(overhead);
            total_expected_extents += usable / pe_bytes;
        }

        let draft = Draft {
            draft: vec![Call::VgCreate {
                name: "test_vg".to_string(),
                pvs,
                pe_size: pe_unit,
            }],
            status: DraftStatus::Pending,
            auto_confirm: false,
            draft_type: "provision".to_string(),
            warnings: vec![],
        };

        (draft, state, total_expected_extents, pe_bytes)
    })
}

fn draft_and_state_strategy() -> impl Strategy<Value = (Draft, SystemState, DraftStatus)> {
    let pool: Vec<u32> = (0..100).collect();
    // 1. Generate a vector of unique IDs to prevent naming collisions
    prop::sample::subsequence(pool, 2..20).prop_flat_map(|ids| {
        // Convert IDs into unique calls (e.g., id 5 -> "/dev/sd5" or "vg5")
        let mut calls = Vec::new();
        for id in ids {
            calls.push(match id % 3 {
                0 => Call::PvCreate(PathBuf::from(format!("/dev/vd{}", id))),
                1 => Call::VgCreate { 
                    name: format!("vg{}", id), 
                    pvs: vec![PathBuf::from(format!("/dev/vd{}", id))], 
                    pe_size: SizeUnit::Megabytes(4) 
                },
                _ => Call::LvCreate { 
                    vg: format!("vg{}", id), 
                    name: format!("lv{}", id), 
                    size: SizeUnit::Extents(100) 
                },
            });
        }

        prop_oneof![
            // WORLD 1: CLEAN - System is empty
            Just((
                create_draft(calls.clone()),
                SystemState::default(),
                DraftStatus::Clean
            )),

            // WORLD 2: DONE - Every call is reflected in SystemState
            Just(calls.clone()).prop_map(|cs| {
                (create_draft(cs.clone()), build_state(&cs), DraftStatus::Done)
            }),

            // WORLD 3: DIRTY - Only the first 'n' calls are in SystemState
            (1..calls.len()).prop_map(move |n| {
                let partial_calls = &calls[0..n];
                (create_draft(calls.clone()), build_state(partial_calls), DraftStatus::Dirty)
            })
        ]
    })
}


fn create_draft(calls: Vec<Call>) -> Draft {
    Draft {
        draft: calls,
        status: DraftStatus::Pending,
        auto_confirm: false,
        draft_type: "provision".to_string(),
        warnings: vec![],
    }
}

fn build_state(calls: &[Call]) -> SystemState {
    let mut state = SystemState::default();
    for call in calls {
        match call {
            Call::PvCreate(p) => { state.pvs.insert(p.clone()); }
            Call::VgCreate { name, .. } => { state.vgs.insert(name.clone()); }
            Call::LvCreate { vg, name, .. } => { state.lvs.insert((vg.clone(), name.clone())); }
            Call::MkSwap(p) => { state.swaps.insert(p.clone()); }
            Call::Mkfs { device, .. } => { state.filesystems.insert(device.clone()); }
            Call::Mkdir(p) => { state.paths_exist.insert(p.clone()); }
            _ => {}
        }
    }
    state
}

fn safety_bits() -> impl Strategy<Value = (bool, bool, bool)> {
    (any::<bool>(), any::<bool>(), any::<bool>())
}

fn safety_scenario_strategy() -> impl Strategy<Value = (Draft, SystemState, bool, usize)> {
    prop::collection::vec((0..100u32, safety_bits()), 1..5).prop_map(|scenarios| {
        let mut draft_calls = Vec::new();
        let mut state = SystemState::default();
        let mut should_fail = false;
        let mut expected_warnings = 0;

        let mut seen_ids = std::collections::HashSet::new();

        for (id, (is_full, has_fs, in_fstab)) in scenarios {
            if !seen_ids.insert(id) { continue; }

            let path = PathBuf::from(format!("/dev/vd{}", id));
            draft_calls.push(Call::PvCreate(path.clone()));

            if is_full {
                state.is_full_disk.insert(path.clone());
                expected_warnings += 1;
            }

            if has_fs {
                state.filesystems.insert(path.clone());
                if in_fstab {
                    state.fstab_device_refs.insert(path.clone());
                    should_fail = true; 
                } else {
                    expected_warnings += 1;
                }
            }
        }

        let draft = Draft {
            draft: draft_calls,
            status: DraftStatus::Clean,
            auto_confirm: false,
            draft_type: "provision".to_string(),
            warnings: Vec::new(),
        };

        (draft, state, should_fail, expected_warnings)
    })
}

#[cfg(test)]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100000))]

    #[test]
    fn test_verify_done_invariants((mut draft, state, expected_status) in draft_and_state_strategy()) {
        let result = verify_done(&mut draft, &state);

        prop_assert_eq!(&draft.status, &expected_status);

        if expected_status == DraftStatus::Dirty {
            prop_assert!(result.is_err());
        } else {
            prop_assert!(result.is_ok());
        }
    }

    #[test]
    fn test_verify_safety_logic((mut draft, state, should_fail, expected_warnings) in safety_scenario_strategy()) {
        let result = verify_safety(&mut draft, &state);

        if should_fail {
            prop_assert!(result.is_err());
            prop_assert_eq!(draft.status, DraftStatus::Dirty);
            prop_assert!(result.unwrap_err().to_lowercase().contains("fstab"));
        } else {
            prop_assert!(result.is_ok());
            prop_assert_eq!(draft.status, DraftStatus::Clean);
            let unique_warnings: HashSet<_> = draft.warnings.iter().collect();
            prop_assert_eq!(unique_warnings.len(), expected_warnings);
        }
    }

    #[test]
    fn test_calculate_capacity_math_integrity(input in capacity_scenario_strategy()) {
        let (mut draft, state, expected_extents, expected_pe_bytes) = input; // Destructure here
        let result = calculate_capacity(&mut draft, &state);
        
        match result {
            Ok((actual_extents, actual_pe_bytes)) => {
                prop_assert_eq!(actual_extents, expected_extents);
                prop_assert_eq!(actual_pe_bytes, expected_pe_bytes);
                prop_assert_ne!(draft.status, DraftStatus::Invalid);
            },
            Err(e) => prop_assert!(false, "Expected Ok capacity, got error: {}", e),
        }
    }

    #[test]
    fn test_calculate_capacity_hardware_failure(input in capacity_scenario_strategy()) {
        let (mut draft, mut state, _, _) = input;

        if let Some(Call::VgCreate { pvs, .. }) = draft.draft.first() {
            if let Some(pv_to_remove) = pvs.first() {
                state.paths_exist.remove(pv_to_remove);
            }
        }

        let result = calculate_capacity(&mut draft, &state);
        prop_assert!(result.is_err(), "Should have failed due to missing PV");
        let err_msg = result.unwrap_err().to_lowercase();
        prop_assert!(err_msg.contains("hardware") || err_msg.contains("path"));
        prop_assert_eq!(draft.status, DraftStatus::Invalid);
    }

    #[test]
    fn test_calculate_required_sequential_logic(input in requirement_scenario_strategy()) {
        let (draft, total_usable, pe_bytes, expected_total) = input;
        
        let result = calculate_required(&draft, total_usable, pe_bytes);
        
        match result {
            Ok(actual_total) => {
                prop_assert_eq!(
                    actual_total, 
                    expected_total, 
                    "Math mismatch! Usable: {}, PE: {}", total_usable, pe_bytes
                );
            },
            Err(e) => prop_assert!(false, "Expected Ok requirements, got error: {}", e),
        }
    }

    #[test]
    fn test_verify_uniqueness_happy_path(ids in prop::collection::btree_set(0..100u32, 1..20)) {
        let mut draft_calls = Vec::new();
        for id in ids {
            if id % 2 == 0 {
                draft_calls.push(Call::PvCreate(PathBuf::from(format!("/dev/vd{}", id))));
            } else {
                draft_calls.push(Call::LvCreate {
                    vg: "vg0".to_string(),
                    name: format!("lv{}", id),
                    size: SizeUnit::Extents(10),
                });
            }
        }

        let draft = Draft {
            draft: draft_calls,
            status: DraftStatus::Pending,
            auto_confirm: false,
            draft_type: "provision".to_string(),
            warnings: vec![],
        };
        
        prop_assert!(verify_uniqueness(&draft).is_ok());
    }

    #[test]
    fn test_verify_uniqueness_sabotage(ids in prop::collection::btree_set(0..100u32, 2..20)) {
        let mut draft_calls = Vec::new();
        for id in ids {
            draft_calls.push(Call::PvCreate(PathBuf::from(format!("/dev/vd{}", id))));
        }

        // Sabotage: Duplicate the first entry
        let duplicate = draft_calls[0].clone();
        draft_calls.push(duplicate);

        let draft = Draft {
            draft: draft_calls,
            status: DraftStatus::Pending,
            auto_confirm: false,
            draft_type: "provision".to_string(),
            warnings: vec![],
        };

        let result = verify_uniqueness(&draft);
        prop_assert!(result.is_err(), "Should have detected duplicate PV");
        prop_assert!(result.unwrap_err().to_lowercase().contains("duplicate"));
    }

    #[test]
    fn test_verify_possible_status_pipeline(input in capacity_scenario_strategy()) {
        let (draft, state, expected_extents, _) = input;

        // --- SCENARIO 1: CONDITIONAL SUCCESS ---
        {
            let mut test_draft = draft.clone();
            test_draft.status = DraftStatus::Clean;
            let result = verify_possible(&mut test_draft, &state);
            
            // In this strategy, required is always 0 because there are no LvCreates.
            // Therefore, as long as usable >= 0 (which is always true for u128),
            // verify_possible should return Ok.
            prop_assert!(result.is_ok(), "Expected Ok for {} usable, got {:?}", expected_extents, result);
            prop_assert_eq!(test_draft.status, DraftStatus::Ready);
        }

        // --- SCENARIO 2: MATH FAILURE (INVALID) ---
        {
            let mut test_draft = draft.clone();
            // Add a massive LV that exceeds our 1000 extent usable capacity
            test_draft.draft.push(Call::LvCreate {
                vg: "test_vg".to_string(),
                name: "overprovisioned_lv".to_string(),
                size: SizeUnit::Extents((expected_extents as u64).saturating_add(100)),
            });

            let result = verify_possible(&mut test_draft, &state);
            
            prop_assert!(result.is_err());
            prop_assert_eq!(test_draft.status, DraftStatus::Invalid);
        }

        // --- SCENARIO 3: UNIQUENESS FAILURE (INVALID) ---
        {
            let mut test_draft = draft.clone();
            // Duplicate the first PV call to trigger verify_uniqueness
            if let Some(first_call) = test_draft.draft.first().cloned() {
                test_draft.draft.push(first_call);
            }

            let result = verify_possible(&mut test_draft, &state);
            
            prop_assert!(result.is_err());
            prop_assert_eq!(test_draft.status, DraftStatus::Invalid);
        }
    }
}

#[test]
fn test_zero_denominator_logic() {
    let mut draft = Draft {
        auto_confirm: false,
        draft_type: "provision".to_string(),
        draft: vec![
            Call::Mkfs { 
                device: PathBuf::from("/dev/sdb1"), 
                fs: Filesystem::Ext4 
            },
            Call::Mkdir(PathBuf::from("/mnt/data")), 
        ],
        status: DraftStatus::Pending,
        warnings: vec![],
    };

    let mut state = SystemState::default();
    state.filesystems.insert(PathBuf::from("/dev/sdb1"));
    state.paths_exist.insert(PathBuf::from("/mnt/data"));

    let res = verify_done(&mut draft, &state);
    
    // Logic: total_calls starts at 2. 
    // Mkfs matches -> total_calls = 1.
    // Mkdir matches -> total_calls = 0.
    // matched_calls = 0. 0 == 0 -> Done.
    assert_eq!(draft.status, DraftStatus::Done);
    assert!(res.is_ok());
}

