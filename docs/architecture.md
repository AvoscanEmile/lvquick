# lvquick Architecture

## Philosophy

lvquick exists for one specific scenario:

You are deep into a shift.
You need to modify storage.
You cannot afford a mistake.

It is a transactional wrapper around LVM2 designed to eliminate ambiguity, arithmetic errors, and sequencing mistakes in high-risk storage workflows. It replaces the traditional “fire-and-forget” CLI model with a structured lifecycle:

**Plan → Verify → Confirm → Execute**

lvquick is not a replacement for LVM2. It is not a daemon. It is not a storage orchestrator.

It is a deterministic safety layer built for production systems.

It is designed for:

* **Transactional integrity**: Every operation is modeled as an immutable plan before execution.
* **Refusal over confusion**: If state is unclear or inconsistent, lvquick stops.
* **Deterministic recovery**: Execution is journaled and resumable.
* **Explicit responsibility**: Overrides exist (`--force -y`), but they are intentional and visible.
* **Boring reliability**: High-risk operations become predictable and repeatable.

lvquick is advisory about intent, but authoritative about transaction integrity. It does not attempt to be clever. It attempts to be correct.

## Why a Wrapper Over LVM2?

LVM2 is powerful and mature, but fundamentally imperative. Commands are executed directly against the system, and multi-step workflows require careful manual sequencing.

The risks are typically:

* Extent miscalculations
* Incorrect resize ordering (filesystem vs LV)
* Partial fstab updates
* Interrupted `pvmove`
* Inconsistent post-operation state

lvquick addresses these not by replacing LVM2 internals, but by:

* Parsing `lvm fullreport --reportformat json`
* Generating an internal action plan
* Verifying invariants before execution
* Journaling every step
* Validating final system state

All communication with LVM2 happens via CLI-to-JSON. No C bindings are used.

## Core Execution Model

Every lvquick command follows a strict lifecycle.

### 1. Ingestion & Validation

lvquick gathers:

* Full LVM snapshot via `lvm fullreport`
* `/etc/fstab`
* Active mount points
* Filesystem signatures
* Available VG space
* Active `pvmove` states

It checks for “blunder risks” such as:

* Existing signatures on new PVs
* Busy mount points
* Insufficient free extents
* Inconsistent fstab entries
* Conflicts with active transaction journals

If a journal exists and live system state differs from the journal’s expected intermediate state:

* lvquick refuses to plan.
* `--force` is required to proceed.

### 2. Transaction Planning

The planner generates an immutable:

```
Vec<LvmAction>
```

This internal DSL defines every command that will be executed.

The plan is frozen once generated.
No recalculation occurs during execution.

The planner verifies invariants such as:

* `LV_new_size ≥ FS_size`
* `VG_free ≥ required_extents`
* Cache pool ratio correctness
* Resize ordering safety

If current state already matches expected state:

* The plan is empty.
* lvquick exits successfully (idempotent no-op).

### 3. Confirmation

The user is shown:

* Current State
* Expected State
* Execution Plan

Default behavior requires explicit confirmation `[y/N]`.

Flags:

* `-y`: Skip confirmation
* `--force`: Override journal drift refusal

`--force` does not skip planning or confirmation.
It only allows proceeding when integrity boundaries are violated.

`--force -y` is allowed, but explicit.

### 4. Transaction Journal

Once a plan is generated, a transaction file is created at:

```
/var/lib/lvq/transactions/<datetime_id>.json
```

This file contains:

* Schema version
* Timestamp
* Detected LVM version
* Kernel version
* Initial state snapshot hash
* Immutable plan
* Execution log (step-by-step results)
* Transaction state

Transaction states:

* `planned`
* `executing`
* `drifted`
* `failed`
* `completed`
* `abandoned`

The journal is the authoritative record of the transaction.

It is not temporary.
It is durable.

### 5. Atomic Execution

Execution is:

* Sequential
* Deterministic
* Plan-driven only
* Logged step-by-step

For each `LvmAction`:

* Execute underlying command
* Record command, args, exit code, timestamp
* Append to journal

If a step fails:

* Failure is classified (validation, transient, destructive, partial mutation)
* User is offered rollback
* Rollback is implemented as a compensating plan derived from initial state

Rollback is best-effort logical reversal, not guaranteed byte-identical restoration.

### 6. Post-Condition Verification

After all actions execute:

* lvquick performs a fresh ingestion
* Compares actual state with expected state

A transaction is considered complete only when:

Expected State == Actual State

Command success alone is insufficient.

If mismatch exists, the transaction is marked `failed`.

## Drift and Integrity Boundaries

During the planification phase a system state hash is stored by the program, right before execution this hash is recalculated to verify it matches. If live system state conflicts with a transaction journal:

Default behavior:

* Refuse to act
* Mark transaction as `drifted` (From initial state). 

With `--force`:

* Re-ingest
* Display detected drift
* Ask confirmation
* Proceed using journal as authoritative

lvquick assumes you are not simultaneously running raw LVM2 commands and lvquick, nor two or more lvquick commands at the same time. However, it verifies post-conditions to guard against drift.

Integrity is prioritized over convenience.

## Command Suite

lvquick focuses exclusively on operations that are:

• Multi-step  
• Arithmetic-sensitive   
• Order-dependent  
• Operationally dangerous under fatigue   

These are the workflows that cause real outages — not because LVM2 is flawed, but because humans are.

Each command below is implemented as a deterministic plan generator. It expands a high-level intent into a fully validated, journaled, ordered execution plan.

### `provision`

**Workflow:**
Partitioning (if needed) → PV → VG → LV → Filesystem → Mount → fstab entry

This is the “from raw disk to mounted storage” path.

It takes an uninitialized block device and turns it into a fully usable, persistent filesystem in one transactional operation. Internally, it:

• Verifies no conflicting signatures exist  
• Initializes the Physical Volume  
• Creates or extends the Volume Group  
• Allocates a Logical Volume with validated extent math  
• Formats the filesystem (with safe defaults)  
• Generates a UUID-based `/etc/fstab` entry  
• Mounts and verifies accessibility  
• UUID resolution instead of device paths  
• Safe mount options chosen automatically  
• Atomic fstab modification  
• Post-condition verification of mount + FS  

This eliminates the “it worked until reboot” class of errors.

### `decommission`

**Workflow:**
Unmount → Remove fstab → Remove LV → Remove VG (optional) → Remove PV

This is the controlled teardown path.

It safely dismantles storage while ensuring no ghost references remain. It:

• Verifies mount state  
• Cleanly unmounts  
• Removes fstab entries before destructive operations  
• Deletes LV/VG/PV in dependency order  
• Validates no dangling references remain  
• Guarantees `/etc/fstab` consistency, preventing emergency shell drops at boot  

This command prevents the “forgot the fstab line” outage that only appears on the next restart.

### `replace-disk`

**Workflow:**
Add new PV → Extend VG → `pvmove` → Reduce old PV → Remove old PV

This handles live disk replacement inside a Volume Group.

Internally, it:

• Verifies the VG can temporarily span both disks  
• Adds the replacement PV  
• Initiates and monitors `pvmove`  
• Journals long-running move state  
• Validates full extent migration  
• Safely removes the old PV  
• Manages long-running `pvmove` transactions safely and resumably  

This turns a high-anxiety maintenance window operation into a deterministic workflow.

### `accelerate`

**Workflow:**
Create cache-pool → Calculate metadata size → Attach cache → Verify mode

This enables SSD caching for HDD-backed logical volumes.

The difficult part of LVM caching is not the command — it is the metadata sizing math and ratio correctness. It:

• Calculates correct cache pool sizing  
• Determines appropriate metadata LV size  
• Verifies SSD capacity constraints  
• Attaches cache in the desired mode (writeback/writethrough)  
• Validates final cache status  
• Eliminates human error in SSD-to-HDD ratio math  

This removes the “why did my cache pool explode?” class of mistakes.

### `shrink`

**Workflow:**
Filesystem Resize → Logical Volume Reduce

Shrinking is dangerous because ordering matters.

If LV is reduced before the filesystem, data loss occurs.

It enforces:

• Filesystem minimum size detection  
• Strict ordering: FS first, LV second  
• Verified size invariants before execution  
• Post-operation validation  
• Proven invariant: LV_size ≥ FS_size at all times  

The shrink command refuses to proceed if the relationship cannot be guaranteed.

### `shrink-xfs`

**Workflow:**
Create new LV → Format → Copy data → Update fstab → Swap mount → Delete old LV

XFS does not support in-place shrinking.

Instead of telling the user “you can’t,” lvquick automates the canonical workaround:

• Creates a correctly sized new LV  
• Formats it  
• Copies data safely  
• Verifies data integrity  
• Atomically updates mount references  
• Removes the original LV  
• Converts a multi-hour manual migration into a single atomic plan  

This is not a shortcut — it is a structured migration with rollback boundaries.

### `snap-back`

**Workflow:**
Freeze → Snapshot → Unfreeze → Mount snapshot (optional)

This command creates application-consistent snapshots.

Internally, it:

• Detects filesystem type  
• Freezes the filesystem  
• Creates LVM snapshot  
• Unfreezes safely  
• Optionally mounts snapshot read-only  
• Ensures snapshot consistency for databases and active workloads  

It prevents “crash-consistent but logically corrupt” backups.

### `evacuate`

**Workflow:**
Verify free space → `pvmove` extents → Reduce PV → Validate VG

This removes a disk from a VG without replacing it.

Internatlly it:

• Calculates required free extents in remaining PVs  
• Refuses to proceed if space is insufficient  
• Moves extents deterministically  
• Validates full migration before PV reduction  
• Guarantees the VG can absorb the data before moving anything  

This avoids mid-operation “out of space during pvmove” failures.

## Why Only These Eight?

lvquick does not aim to wrap every LVM command.

It targets operations where:

• The sequence matters  
• The math matters  
• The fatigue matters  
• The consequences matter  

Each command is opinionated, validated, journaled, and post-verified.

No shortcuts.
No hidden behavior.
No silent assumptions.

Only deterministic storage transitions.

## Internal Action Model

`LvmAction` is the core abstraction.

Each action defines:

* Underlying command and arguments
* Idempotent flag
* Destructive flag
* Reversible flag
* Verification logic

All execution behavior derives from this model.

It is the architectural kernel of lvquick.

## fstab Safety Model

Modifications to `/etc/fstab` use a:

Temp → Sync → Atomic Rename

pattern to prevent partial writes and boot failures.

fstab changes are included in the transaction journal.

## Idempotency

If the system already matches the declared intent:

* No actions are generated.
* Exit code 0.
* No mutation occurs.

Idempotency is intentional and foundational for future automation integration.

## Exit Code Discipline

Planned exit categories:

* 0 — Success / No-op
* 1 — Validation failure
* 2 — Reversible execution failure
* 3 — Non-reversible execution failure
* 4 — Drift detected

This supports future Ansible integration and machine-readable automation.

## Design Constraints

* Single statically linked Rust binary
* No runtime dependencies beyond LVM2
* Targeted at RHEL 10+, but usable across any system relying on LVM2. 
* Air-gapped compatible
* No daemon
* No distributed locking
* No hidden retries

lvquick must be predictable, explicit, and transparent.

## Long-Term Direction

Potential expansions:

* Structured JSON plan output
* `lvq continue`, `lvq repair`, `lvq history`
* Snapshot hash enforcement
* Enhanced failure classification
* Capability detection per LVM version
* Ansible plugin

All future features must preserve:

* Immutable plan model
* Journaled execution
* Refusal on ambiguity
* Post-condition verification
