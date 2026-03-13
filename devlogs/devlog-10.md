# Devlog Entry [10] — Architecting a Verifiable State Machine for LVM Operations

**Date**: 2026-03-12

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

The primary focus of this development cycle was the modularization and formal verification of the **Verifier** module. The goal was to transform a monolithic, side-effect-heavy validation function into a testable, pure-logic state machine. By decoupling hardware probing from evaluation logic, a framework was established to predict LVM failures, ensure idempotency, and guarantee system safety before any destructive commands are executed.

## Implementation

The transformation followed a strict "Imperative Shell, Functional Core" pattern. The following implementation steps were executed to achieve a high-confidence validation pipeline.

### Decoupling via SystemState Snapshot

The `SystemState` struct was expanded to act as a comprehensive "Oracle" of the host machine. Instead of functions calling `lsblk` or `blkid` mid-calculation, all necessary system facts—such as block device sizes, filesystem signatures, and `/etc/fstab` references—were gathered into a static snapshot during the initial phase. This allowed subsequent logic functions to remain "pure," operating solely on the data provided by the snapshot.

### Implementation of the `verify_done` Orchestrator

A mathematical approach to idempotency was implemented within `verify_done`. The function was designed to calculate the ratio of "Matched Calls" (work already completed) to "Total Calls" (structural work remaining). Logic was added to handle "work-reduction" calls, such as `Mkfs` or `Mkdir`; if these prerequisites already exist on the system, the total required work count is decremented. This ensures that the system correctly transitions to `DraftStatus::Done` without re-running finished tasks.

### Refactoring `verify_possible` into a Modular Pipeline

To manage the complexity of hardware validation, `verify_possible` was decomposed into three distinct sub-functions:

1. **`check_safety`**: Scans for critical conflicts (e.g., wiping a device referenced in `fstab`).
2. **`calculate_capacity`**: Aggregates physical extents from the provided hardware facts.
3. **`calculate_required`**: Computes the necessary extents for Logical Volumes, specifically handling sequential `%FREE` space targets.

### Implementation of Property-Based Relational Strategies

A testing harness was constructed using the `proptest` crate. Rather than generating random inputs, **Relational Generators** were developed. These generators create a `Draft` and then "Mirror" or "Sabotage" a corresponding `SystemState` to force the logic into specific states (`Done`, `Clean`, or `Dirty`). This methodology allowed for the automated verification of 100,000 unique permutations of system configurations.

## Challenges & Resolutions

### The Proptest "Ghost World" Collision

* **Challenge**: Initial property-based tests yielded false failures where a single-call draft was expected to be `Dirty` but was correctly classified by the code as `Clean`.
* **Resolution**: It was realized that `Dirty` is an emergent property of multi-step drafts. The test strategy was updated with a `prop_filter` to ensure "Saboteur" (Dirty) cases are only generated for drafts with two or more calls, maintaining mathematical consistency between the test and the logic.

### Duplicate Target Warning Spam

* **Challenge**: During high-iteration testing, it was observed that if a `Draft` contained duplicate `PvCreate` calls for the same path, the `verify_safety` function pushed redundant warnings for each instance.
* **Resolution**: This was identified as a "Spec Gap." A decision was made to treat duplicate PV paths or LV names as hard validation errors rather than just warnings, as they represent logical paradoxes in the provision plan. Implementation of this collision detection was scheduled for the subsequent cycle.

### Proptest Strategy Meta-Panics

* **Challenge**: The test runner panicked with `Invalid use of empty range 1..1` when attempting to sabotage a draft of length one.
* **Resolution**: The generator range was adjusted to `1..len.max(2)` and guards were added to ensure the number of "matched" calls in a sabotaged state never equals zero or the total length, preventing the generator from hitting an empty selection set.

## Testing & Validation

Validation was conducted through a combination of deterministic unit tests and large-scale property-based testing:

* **Idempotency Tests**: 100,000 iterations were run on `verify_done` to ensure various combinations of partial work always resulted in the correct `DraftStatus`.
* **Safety Matrix Tests**: A "Bit-Flip" generator was used to test the 8 possible safety states of a disk (Full Disk, FS Signature, Fstab Entry). 5,000 tests confirmed that the critical "Kill Switch" only triggers when a device has both a signature and an fstab reference.
* **Zero-Denominator Logic**: Manual unit tests verified that drafts containing only prerequisites (like `Mkdir`) correctly land on `Done` when those directories exist, preventing 0/0 division errors.

## Outcomes

The primary achievement of this cycle was the stabilization of the Verifier into a "Pure Logic" core. The software is now capable of predicting whether an LVM plan will fit on the available hardware and whether it poses a safety risk to the host system without performing any actual IO during the validation phase. The architecture successfully moved from a "Try and Fail" model to a "Predict and Prevent" model.

## Reflection

This development cycle highlighted the "Architecture Tax" associated with building reliable hardware-management software. It was observed that the testing infrastructure often grew larger and more complex than the implementation code itself. This phenomenon is viewed as a positive indicator; the complexity of the "messy" real world is being successfully captured and mitigated within the test suites, allowing the production code to remain lean and mathematically sound.

Furthermore, the strictness of the Rust type system served as a powerful collaborator. By defining the `SystemState` and `Call` structures with precision, the compiler effectively acted as a secondary architect, catching structural inconsistencies before they could manifest as runtime bugs. The shift from "writing code" to "designing specifications" has fundamentally increased the project's robustness.

## Next Steps

The immediate priority is the refinement of the `verify` module to integrate collision detection logic, specifically targeting duplicate PV paths and LV names to prevent logical paradoxes. Following this, the remaining property-based tests for the `calculate_required` math engine will be fully implemented to validate sequential space allocation logic. Finally, the development will shift toward the creation of a comprehensive testing harness for the `exec` module and the establishment of end-to-end (EtoE) integration tests to ensure the stability of the entire provision pipeline.
