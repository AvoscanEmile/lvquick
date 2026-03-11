use super::SizeUnit;

#[kani::proof]
fn verify_to_bytes_arithmetic_safety() {
    // 1. Symbolic u64 input
    let val: u64 = kani::any();
    
    // 2. Test the "Worst Case" (Exabytes)
    // If this one doesn't overflow, none of the smaller units will.
    let unit = SizeUnit::Exabytes(val);
    
    let result = unit.to_bytes();
    
    // 3. Property: Success for absolute units
    assert!(result.is_ok());
    
    // 4. Property: Exactness
    // Kani proves that for all 'val', the result is correct and hasn't overflowed.
    let bytes = result.unwrap();
    assert_eq!(bytes, (val as u128) * 1_152_921_504_606_846_976);
}

#[kani::proof]
fn verify_to_bytes_error_cases() {
    // Percentage and Extents should always return Err in the current implementation.
    // We can use kani::any() to pick a symbolic u8/PercentTarget to be thorough.
    let percentage = SizeUnit::Percentage(kani::any(), kani::any());
    assert!(percentage.to_bytes().is_err());
    
    let extents = SizeUnit::Extents(kani::any());
    assert!(extents.to_bytes().is_err());
}
