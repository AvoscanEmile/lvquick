# Devlog Entry 02 — Formalizing Storage Geometry through Type-Safe Domain Modeling

**Date**: 2026-03-02

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

To establish the core architectural skeleton for a high-integrity LVM orchestration tool. The primary goal was to define a domain-specific language (DSL) in Rust that translates fuzzy human storage requirements into deterministic, machine-executable operations. This phase focused on creating a "Logic-Tight" anatomy that prevents common storage pitfalls—such as rounding errors and invalid state transitions—at the type level.

### Implementation

The design was centered around a layered approach where user intent is refined into discrete, verifiable hardware calls.

#### Scalable Unit Representation

A comprehensive `SizeUnit` enum was implemented to handle the vast range of storage capacities.

By utilizing `u64` for variants ranging from `Bytes` to `Exabytes`, the system was inherently protected against negative storage values. A critical design decision was made to include an `Extents(u64)` variant. This allows the orchestrator to move past ambiguous size strings and communicate with the LVM subsystem using the "absolute truth" of physical extents, calculated via:


$$Extents = \lceil \frac{RequestedBytes}{PEBytes} \rceil$$

#### Structuring Intent vs. Execution

The relationship between user requests and system actions was formalized using `LvRequest` and `Call`.

* **`LvRequest`**: This struct was designed to capture the desired state of a Logical Volume, utilizing `Option<Filesystem>` and `Option<PathBuf>` to denote that filesystems and mount points are optional attributes of a volume rather than mandatory ones.
* **`Call`**: A discrete, atomic enum was created to represent the final "To-Do List" for the OS. By separating operations like `WipeSignatures`, `PvCreate`, and `Mkfs`, a clear path for granular execution and error reporting was established.

#### Atomic Action Modeling

A top-level `Action` struct was defined to encapsulate the `Command::Provision` intent. This structure ensures that the entire provisioning lifecycle—from identifying physical volumes (`pvs`) to setting the Physical Extent size (`pe_size`) and defining multiple logical volumes—is treated as a single, coherent unit of work.

### Challenges & Resolutions

**Challenge**: Ambiguity in how LVM handles rounding when users provide human-readable strings (e.g., "10G").
* **Resolution**: The `Extents(u64)` variant was integrated into the core `SizeUnit` enum. This ensures that the math is performed within the tool's logic rather than being delegated to the `lvcreate` binary, providing the author with total control over disk geometry.


**Challenge**: Representing optional hardware configurations (like a swap partition which has no filesystem or mount point) without cluttering the logic with error-handling types.
* **Resolution**: `Option<T>` was utilized within the `LvRequest` struct. This was done to reflect the physical reality that a volume can exist in various states of readiness, effectively making the type system the "source of truth" for the intended hardware state.


**Challenge**: Ensuring the tool could handle "dirty" disks that contain existing partition tables or signatures.
* **Resolution**: Specific atomic variants for `PartitionDisk` and `WipeSignatures` were added to the `Call` enum, ensuring these "pre-flight" safety checks are treated as first-class citizens in the execution pipeline.



### Testing & Validation

The architecture was validated through a "Type-Safety Audit." It was observed that by using Rust’s `enum` structures, invalid commands (such as attempting to format a volume with a non-existent filesystem) became unrepresentable in the code. Logical verification was performed to ensure that the `Call` enum could be mapped to a hypothetical "Rollback" function, confirming that the data-driven design supports transactional atomicity.

### Outcomes

* A complete, type-safe anatomy for LVM orchestration was produced.
* A deterministic path from human input to physical extents was established.
* The system was prepared for "Idempotency-by-Design," where the current state of a machine can be compared against the `Vec<Call>` log to determine if work is required.

## Reflection

The development of this skeleton demonstrates the power of "Logic-Tight" programming. By spending the necessary time to define the geometry of the data before writing a single line of execution logic, an entire category of "off-by-one" errors and invalid state bugs was eliminated. The decision to make the core logic a pure "Type" for the orchestrator, rather than the orchestrator itself, ensures that the tool can easily adapt to different interfaces—be it a CLI, a JSON-based automation plugin, or a future web-based dashboard.

This architecture is not merely a script; it is a formal specification of intent. It serves as a reminder that in systems engineering, the most important work happens in the type system. If the types accurately reflect the physical reality of the hardware, the implementation of the execution logic becomes a trivial mapping exercise.

**Next Step**: The implementation of the `FromStr` parser for `SizeUnit` and `Filesystem` will be initiated to bridge the gap between raw user input and this established anatomy.
