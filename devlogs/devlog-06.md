# Devlog Entry 06 — Implementing Warnings and Fstab / Swap handling. Preparing for the testing harnesses development.

**Date**: 2026-03-05

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

The primary focus of this development cycle was the transition of the `lvq` provisioner from a transient command generator into a robust, state-aware systems orchestrator. The overarching goal was to ensure that any provisioned storage—including logical volumes and swap space—attains "full lifecycle" status. This required engineering atomic configuration updates for `/etc/fstab`, integrating immediate swap activation, and implementing a sophisticated verification engine capable of detecting system-critical conflicts before execution.

## Implementation

### Atomic `fstab` Transaction Logic and Core Expansion

To support persistent configuration, the `Call` enum was expanded with an `Fstab` variant. The execution logic for this variant was engineered to prevent configuration corruption through an atomic rename pattern. Rather than a standard file append, the process follows a strict transactional sequence:

* **Step A**: A physical backup of the live configuration is created via `cp -p /etc/fstab /etc/fstab.bak`.
* **Step B**: A temporary working copy is generated specifically for the filesystem type (e.g., `/etc/fstab.xfs.tmp`).
* **Step C**: A shell-level fallback logic was implemented to resolve identifiers. The system attempts to invoke `blkid` to retrieve a UUID; if unsuccessful, it defaults to the absolute device path (e.g., `/dev/vg/lv`) to ensure bootability.
* **Step D**: The `mv` utility is used to atomically replace the live `/etc/fstab` with the verified temporary file.

### Comprehensive Swap Lifecycle and Path Logic

Support for `Filesystem::Swap` was implemented as a specialized state. The `Call::MkSwap` variant was expanded to include both `mkswap` (initialization) and `swapon` (immediate session activation). To satisfy Linux kernel requirements, the `Planner` was restricted to omit `Mkdir` and `Mount` calls for swap types, while the `Fstab` logic was hardcoded to utilize `none` as the mount point placeholder with the `sw` option.

### State-Aware Verification and Pre-flight Guardrails

The `verifier` module was overhauled with advanced probing capabilities, including `probe_swap_active`, `probe_fstab_exists`, and `probe_is_full_disk`. These tools allow the provisioner to detect if a target device is already referenced in `/etc/fstab` or active in `/proc/swaps`. A "Refusal over Confusion" policy was established: if a device intended for wiping is found to be a boot-critical component in the current `fstab`, the tool triggers a `Dirty` state and hard-blocks execution.

### Warning Propagation and Daemon Synchronization

To maintain a "Power Tool" philosophy without being a "nanny," a `warnings` vector was added to the `Draft` and `Exec` structs. This allows the verifier to flag non-standard but legal configurations—such as using a raw disk as a Physical Volume (PV) instead of a partition—and display them in a dedicated `--- WARNINGS ---` block during the user confirmation phase. Finally, a `systemctl daemon-reload` was integrated as the concluding step of the execution list to synchronize the OS manager with the new disk state.

## Challenges & Resolutions

**Challenge: UUID Resolution Failures**
There was a risk of `blkid` failing to return a UUID immediately after formatting, leading to invalid `fstab` entries.
* **Resolution**: A POSIX-compliant `if/else` shell routine was embedded into the command generation. This ensures that if the UUID variable is empty, the logic automatically substitutes the raw device path, maintaining a "it just works" reliability.

**Challenge: Swap Mount Path Conflicts**
There was a risk of the executor attempting to create directories or mount swap partitions to physical paths.
* **Resolution**: The `Planner` logic was modified to explicitly omit `Mkdir` and `Mount` calls when `Swap` is detected. The `Fstab` logic was synchronized to treat `none` as the valid placeholder, ensuring consistency between the planner and the verifier.

**Challenge: Systemd/Fstab Desync**
Standard LVM and mount commands often leave the OS service manager out of sync with the `/etc/fstab` file.
* **Resolution**: The addition of `systemctl daemon-reload` as a conditional final step (triggered by `has_fstab_calls`) resolved this, ensuring a clean handover from `lvq` to the operating system.

## Testing & Validation

The implementation was validated through a full-stack deployment on four loop devices.

* **Observation**: The tool successfully generated a plan covering PV creation, VG setup, and mixed-FS provisioning (XFS, Ext4, and Swap).
* **Idempotency**: Post-execution, the tool was re-run. The verifier correctly matched UUIDs in `/etc/fstab` and active mounts in `/proc`, resulting in an "Already in desired state" exit in **0.037s**.
* **Integrity**: `lsblk` and `swapon --show` confirmed that the `swap_space` LV was correctly flagged and active, proving the integration was successful.

## Outcomes

The direct result of this cycle is a transition from a helper script to a reliable orchestrator. `lvq` now handles the entire storage lifecycle with atomic safety and millisecond-level verification. By providing automated backups (`.bak` files) and detailed "INTENT/SUCCESS" logging, the tool offers a professional "flight recorder" for system administrators to audit or repair state transitions.

## Reflection

The shift toward atomic `/etc/fstab` updates represents a fundamental change in the project's reliability model. In systems programming, a "successful" execution that risks a corrupted boot state is an architectural failure. By moving to an atomic rename pattern, the window for file corruption during a system crash or power loss is effectively closed. This approach balances "hard" guardrails—such as blocking the destruction of active mount sources—with "soft" warnings for non-standard topologies like raw-device PVs. It respects the operator’s intent while ensuring the tool never becomes the cause of a catastrophic system state.

Furthermore, the "Refusal over Confusion" policy for `Dirty` states has simplified the state machine significantly. By enforcing a binary transition (Clean to Ready, or Ready to Done), we avoid the combinatorial complexity of trying to recover from partially applied or unknown configurations. This deterministic model makes the logic easier to reason about, simplifies the verification suite, and ensures that the system’s integrity is never a matter of guesswork.

## Next Steps

With the core provisioning and persistence logic verified, the focus shifts to the **Triad of Certainty**:

* Refactoring the LVM extent calculator into pure functions to facilitate **Kani** formal verification.

* Implementing an "Integration Proptest" to verify the entire `Parser -> Planner -> Verifier` pipeline.

* Developing a **Cargo Fuzz** harness to stress-test the whole parser module. 
