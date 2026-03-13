# Devlog Entry [11] — Formalizing the LVM Verification Pipeline via Property-Based Testing

**Date**: 2026-03-12

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

The primary objective of this development cycle was the hardening of the `verifier` module, specifically the `verify_possible` orchestration logic. The goal was to transform a series of disconnected mathematical checks and safety guards into a robust, terminal state machine capable of validating complex LVM (Logical Volume Manager) provisioning drafts. This involved ensuring that any failure—whether resulting from physical hardware constraints, naming collisions, or lifecycle violations—correctly transitioned the system state to a definitive `Invalid` status before any shell execution occurred.

## Implementation

### Refinement of Mathematical Guardrails

The core logic for capacity and requirement calculation was refined to ensure mathematical accuracy across varying units. A persistent challenge involving the conversion of `SizeUnit` variants to raw bytes was addressed by implementing `to_bytes()` with `u128` precision. This was done to prevent overflow during the accumulation of physical extents across multiple devices. The `calculate_capacity` function was structured to subtract a hardcoded 1MB overhead for LVM metadata per Physical Volume (PV) before determining the number of usable Physical Extents (PE) based on the user-defined `pe_size`.

### State Machine Formalization in `verify_possible`

The `verify_possible` function was re-architected to act as a strict gatekeeper. To ensure architectural consistency, an immediately invoked closure pattern was utilized to wrap all internal validation steps (`verify_uniqueness`, `verify_safety`, `calculate_capacity`, and `calculate_required`). This allowed the `?` operator to be used for clean error propagation while ensuring that any returned `Err` was caught by a final `match` block. Consequently, the `DraftStatus` is now guaranteed to transition to `Ready` on success or `Invalid` on any failure, providing a predictable interface for the forthcoming executioner module.

### Lifecycle Guard Integration

A strict lifecycle check was implemented to ensure that `verify_possible` is only executed on drafts marked as `Clean`. It was decided that attempting to verify a `Pending` or `Dirty` draft should be treated as a terminal logic error. To maintain the integrity of the state machine, this check was moved inside the validation closure so that "out-of-order" calls would result in a transition to `Invalid`, preventing the system from proceeding with uninitialized data.

## Challenges & Resolutions

### Integer Mismatches in Test Scenarios

* **Challenge**: A compilation error was encountered during the integration of Proptest scenarios where `expected_extents` (calculated as `u128`) was passed to `SizeUnit::Extents`, which expects a `u64`.
* **Resolution**: The value was safely cast using `.try_into().unwrap()` within the test scope. Since the test was designed to intentionally exceed capacity, the risk of truncation was acceptable as the failure would still be triggered.



### "Zero-Capacity" Boundary Conditions

* **Challenge**: Proptest identified an edge case where a tiny device (e.g., 2MB) combined with standard LVM metadata overhead and a 1MB PE size resulted in 0 usable extents. This caused the "Happy Path" test to fail because $0 \ge 0$ was mathematically valid but logically unexpected.
* **Resolution**: Mathematical purity was prioritized. The test assertions were updated to recognize that if a draft requires 0 extents, a capacity of 0 extents is a valid (albeit empty) success. This ensured the verifier remained a pure mathematical evaluator rather than a subjective policy enforcer.



### Status Persistence in Early Returns

* **Challenge**: Initial implementations of the status guard caused the function to return an `Err` before the status was updated to `Invalid`, leaving the draft in a `Pending` state.
* **Resolution**: The lifecycle guard was moved into the unified error-handling closure. This ensured that every possible exit path from the function—including the check for the correct starting status—passed through the final state-transition logic.



## Testing & Validation

Validation was conducted using the `proptest` framework to perform high-entropy fuzzing of the LVM logic. A `capacity_scenario_strategy` was developed to generate complex, valid-but-randomized `SystemState` snapshots alongside corresponding `Draft` objects.

* **State Pipeline Test**: A "Master" integration test was executed to verify the Result $\rightarrow$ Status mapping. This test covered three distinct scenarios: a successful "Ready" path, a "Math Failure" (over-provisioning), and a "Uniqueness Failure" (duplicate device paths).
* **Chaos Resilience**: The parser and planner were subjected to "junk" input and structural errors to ensure that no invalid string could bypass the initial drafting phase.
* **Outcome**: A final suite of 31 tests passed successfully, with several tests running for over 60 seconds to explore deep state-space branches.

## Outcomes

* **Robust State Machine**: The project now possesses a terminal verifier that guarantees a `Ready` status only when physical and logical constraints are fully satisfied.
* **Error Consistency**: All validation errors, regardless of their source (safety, math, or uniqueness), now result in a consistent `Invalid` state, simplifying the front-end error handling.
* **Verified Math**: The capacity calculations were proven resilient against boundary conditions, including near-zero disk space and large-scale over-provisioning.

## Reflection

The transition from unit testing to property-based testing revealed significant "blind spots" in the initial logic, particularly regarding how LVM metadata overhead interacts with small physical disks. The decision to enforce a strict state machine—where the verifier actively invalidates "illegal" draft transitions—proved to be a major turning point in the project's reliability. It was observed that a well-defined state machine does not just prevent errors; it provides a framework for the code to "defend itself" against improper usage.

Philosophically, this cycle reinforced the value of mathematical purity over arbitrary "sanity checks." By allowing $0 \ge 0$ to succeed, the verifier remains a neutral arbiter of truth, leaving specific provisioning policies to the higher-level planner. This separation of concerns ensures that the core engine remains flexible for future use cases.

## Next Steps

Work will now pivot to the **Executioner** module to develop the logic for transforming `Ready` drafts into canonical LVM shell commands. This will include the implementation of an execution testing harness to verify string generation accuracy. Following the completion of the executioner, documentation will be finalized for the official v0.1 release, laying the groundwork for v0.2's End-to-End testing suite.
