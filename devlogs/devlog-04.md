# Devlog Entry 04 — Architectural Refinement and Transactional Pipeline Decoupling

**Date**: 2026-03-04

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

The primary objective of this development cycle was the implementation of a draft generator. The original plan was to output a `Vec<Call>`, but it quickly evolved into a restructuring of the whole project in preparation for the implementation of the verifier module.   

## Implementation

### Definition of the Intermediate Representation (IR)

The concept of a `Draft` struct was introduced to serve as the project's Intermediate Representation. This structure wraps a `Vec<Call>` with a `draft_type` string, preserving the high-level intent (e.g., "provision") as the data passes from the Planner to the Verifier. This prevents context loss, allowing the Verifier to apply command-specific safety rules that would be impossible to infer from a raw list of low-level calls.

### Decoupling the "Dumb" Planner

The `plan_provision` logic was isolated into a specialized worker. Its responsibility was strictly limited to lowering a high-level `Command::Provision` into a sequence of `Call` enums. By removing verification logic from this stage, the Planner was kept "dumb" and deterministic, ensuring it remains easily testable and focused solely on instruction generation rather than system state validation.

### Establishment of the Atomic Verifier

A post-plan verification strategy was adopted over a monolithic system-state parser. Logic was designed to iterate through the `Vec<Call>` within a `Draft` and perform "surgical," targeted queries to the LVM CLI (e.g., `pvs --select`). This approach minimizes the parser surface area and reduces the risk of tool failure due to unexpected global system configurations.

### Simulation via ProvisionState

To facilitate verification of actions that haven't occurred yet, a `ProvisionState` struct was designed. This acts as a "Virtual Canvas" or simulation sandbox. It tracks "pending" changes (e.g., `pending_pvs`, `pending_vgs`) and performs arithmetic validation (PE rounding, extent allocation) to ensure that a sequence of calls is physically possible before execution is attempted.

### Horizontal Compartmentalization

A strict directory and module symmetry was implemented across the `parser`, `planner`, and `verifier` crates. Each module was organized into an orchestrator (`mod.rs`) and a command-specific specialist (`provision.rs`). This structure ensures that namespaces such as `crate::planner::provision` and `crate::parser::provision` remain distinct, providing a clear vertical slice for developers to follow a single feature's lifecycle through the entire stack.

## Challenges & Resolutions

**Challenge:** Verification of dependent calls (e.g., creating a VG on a PV that doesn't exist yet) would fail if the Verifier only checked the live system state.
* **Resolution:** The `ProvisionState` struct was implemented to record the "intended reality." The Verifier checks if a dependency is either already met in the live system or is marked as "pending" from a previous step in the same draft.


**Challenge:** Implementation of verification as a Trait on the `Call` enum was considered but found to be too rigid for context-dependent safety rules.
* **Resolution:** Verification was moved to command-specific functions (e.g., `verify_provision`). This allows the logic to access the full context of the user's intent rather than viewing each LVM call in isolation.


**Challenge:** Potential for name collisions between `provision.rs` files in different directories.
* **Resolution:** Rust’s module system was leveraged to create unique namespaces (`parser::provision` vs. `planner::provision`). This was confirmed to be a safe and idiomatic way to maintain consistent naming conventions across different processing stages.

## Outcomes

The tangible results of this cycle are strictly architectural and structural. The codebase was fully modularized into a hierarchy that supports a transactional pipeline. While the final `main.rs` orchestration logic remains in the conceptual/drafting stage, the underlying modules (`parser`, `planner`, and `verifier`) have been restructured into a mirrored specialist-orchestrator pattern. This includes the implementation of the `Draft` IR and the `ProvisionState` simulation model, providing the necessary scaffolding for an "Idempotency Loop" that will ensure proper machine state handling.

## Reflection

This development cycle reinforced the philosophy that in systems-level programming, the "Happy Path" is the least important part of the architecture. By spending significant effort on the Verifier and the `ProvisionState` simulation, the project has moved toward a model where errors are caught in memory rather than on disk. The realization that the Verifier could double as a post-condition validator was a critical breakthrough, simplifying the codebase while increasing systemic confidence.

The symmetry of the directory structure reflects a mature separation of concerns. It acknowledges that "Provisioning" is a domain-specific problem that requires specialized handling at the parsing, planning, and verification levels. By isolating these into vertical slices, the cognitive load for future maintenance will be significantly reduced, as the logic for any given command is always found in a predictable location.

## Next Steps

1. **Full Implementation of `verifier/provision.rs**`: The targeted LVM JSON queries and the `ProvisionState` arithmetic must be coded to transform the current architectural skeleton into a functional safety engine.
2. **Implementation of `executor/mod.rs**`: A journaled execution engine must be built to translate the `Verified Draft` into actual shell commands with proper error handling and logging.
3. **Interactive Confirmation UI**: A clear, scannable "Pre-flight Report" must be designed to present the verified plan to the user for final approval before execution.

