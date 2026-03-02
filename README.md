# lvq

**lvq (lvquick)** is a Rust-based transactional wrapper for LVM2 designed to make high-risk storage operations safe, deterministic, and auditable. It is built for scenarios where mistakes are costly, providing a structured lifecycle for every operation:

**Plan → Verify → Confirm → Execute**

lvquick is not a replacement for LVM2, a daemon, or a storage orchestrator. It is a deterministic safety layer for production systems, focused on operational correctness and transaction integrity.

## Key Principles

* **Transactional integrity:** Every operation is modeled as an immutable plan before execution.
* **Refusal over confusion:** If the system state is unclear, lvquick stops.
* **Deterministic recovery:** Execution is journaled and resumable.
* **Explicit responsibility:** Overrides exist (`--force -y`), but they are intentional and visible.
* **Boring reliability:** High-risk operations become predictable and repeatable.

lvquick is advisory about intent but authoritative about transaction integrity. It does not attempt to be clever—only correct.

## Why lvquick Wraps LVM2

LVM2 is powerful but imperative: commands execute immediately, and multi-step workflows are prone to human error. Risks include:

* Extent miscalculations
* Incorrect resize ordering (filesystem vs LV)
* Partial `/etc/fstab` updates
* Interrupted `pvmove`
* Inconsistent post-operation state

lvquick addresses these by:

1. Parsing `lvm fullreport --reportformat json`
2. Generating an immutable action plan
3. Verifying invariants before execution
4. Journaling every step
5. Validating final system state

All communication with LVM2 is via CLI-to-JSON; no C bindings are used.

## Core Execution Lifecycle

Every lvquick command follows six phases:

1. **Ingestion & Validation**
   Collects LVM state, mount points, `/etc/fstab`, filesystem signatures, and active transactions. Detects “blunder risks” such as busy mounts or inconsistent fstab entries.

2. **Transaction Planning**
   Generates an immutable `Vec<LvmAction>` plan. Ensures invariants like `LV_new_size ≥ FS_size` and `VG_free ≥ required_extents`. Idempotent if state already matches the plan.

3. **Confirmation**
   Displays current and expected state with the execution plan. Requires explicit `[y/N]` confirmation. Flags:

   * `-y`: skip confirmation
   * `--force`: override journal drift refusal

4. **Transaction Journal**
   Each transaction is stored in `/var/lib/lvq/transactions/<datetime_id>.json`, including plan, logs, state, and metadata. Transaction states: `planned`, `executing`, `drifted`, `failed`, `completed`, `abandoned`.

5. **Atomic Execution**
   Executes actions sequentially and deterministically. Logs results step-by-step and supports best-effort rollback if a step fails.

6. **Post-Condition Verification**
   After execution, the system is re-ingested to ensure Expected State == Actual State. Transactions failing this check are marked `failed`.

## Core Commands

lvquick focuses exclusively on operations where **sequence, arithmetic, and human fatigue matter**. Each command generates a fully validated, journaled, and post-verified plan.

* **`provision`** – From raw disk → mounted filesystem, including fstab entries.
* **`decommission`** – Safe teardown of LV/VG/PV without dangling references.
* **`replace-disk`** – Live replacement of PVs with deterministic `pvmove`.
* **`accelerate`** – SSD caching for HDD-backed LVs with safe metadata sizing.
* **`shrink`** – Safe filesystem → LV reduction where supported.
* **`shrink-xfs`** – XFS shrink via canonical migration with atomic mount updates.
* **`snap-back`** – Application-consistent snapshots with optional read-only mounts.
* **`evacuate`** – Remove a PV from a VG safely, ensuring all data migrates.

**Only these eight commands** are implemented, focusing on deterministic, high-risk operations. lvquick does not aim to wrap all LVM commands.

## Internal Models

* **LvmAction:** Defines underlying command, idempotency, destructiveness, reversibility, and verification logic. This drives all execution behavior.
* **fstab Safety Model:** Updates use a Temp → Sync → Atomic Rename pattern to prevent partial writes.
* **Idempotency:** No actions are generated if the system already matches intent.
* **Exit Codes:**

  * 0 — Success / No-op
  * 1 — Validation failure
  * 2 — Reversible execution failure
  * 3 — Non-reversible execution failure
  * 4 — Drift detected

## Design Constraints

* Single statically linked Rust binary
* No runtime dependencies beyond LVM2
* Targeted at RHEL 10+, usable on any LVM2 system
* Air-gapped compatible
* No daemon, distributed locking, hidden retries
* Explicit, transparent, predictable behavior

## Roadmap v1.0

lvquick is developed in phased milestones, from foundational provisioning to full operational readiness:

| Phase | Focus Area                      | Target Version |
| ----- | ------------------------------- | -------------- |
| 1     | `provision`                     | 0.1            |
| 2     | `decommission`                  | 0.2            |
| 3     | `shrink`                        | 0.3            |
| 4     | `evacuate`                      | 0.4            |
| 5     | `replace-disk`                  | 0.5            |
| 6     | `shrink-xfs`                    | 0.6            |
| 7     | `accelerate`                    | 0.7            |
| 8     | `snap-back`                     | 0.8            |
| 9     | CLI & automation enhancements   | 0.9            |
| 10    | Full operational suite (`v1.0`) | 1.0            |

Each phase includes ingestion, plan generation, deterministic execution, journaling, post-condition verification, and rollback support.

## Considered Future Directions

* Structured JSON plan output for automation pipelines
* Transaction inspection commands: `lvq history`, `lvq continue`, `lvq repair`
* Snapshot hash enforcement
* Enhanced failure classification
* LVM version capability detection
* Ansible plugin integration

All future features will **preserve immutable plans, journaled execution, post-condition verification, and explicit refusal on ambiguity**.
