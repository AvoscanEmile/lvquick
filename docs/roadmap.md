# **lvquick Roadmap to v1.0**

## **Introduction**

This document outlines the official development roadmap for `lvquick` (`lvq`), a Rust-based transactional wrapper for LVM2. The project's mission is to eliminate high-risk storage mistakes by enforcing a deterministic **Plan → Verify → Confirm → Execute** lifecycle. Each phase below represents a major development milestone corresponding to a core operation, ensuring that every high-risk storage modification is fully journaled, validated, and recoverable.

Moving away from the fragility of automatic runtime rollbacks, `lvq` embraces a highly observable, append-only journaling system. Failures result in a strict halt, allowing administrators to inspect the transaction log and utilize `lvq repair` or `lvq continue` to safely resolve partial states.

The roadmap is structured from foundational operations to advanced workflows, culminating in an enterprise-ready `v1.0` release, with an explicit eye toward post-v1.0 ecosystem integrations like Ansible and Kubernetes.

## **Phase 1: `provision**`

**High-Level Goal:** Implement the foundational workflow to safely provision storage from raw disks to mounted filesystems.

**Breadth and Depth of Tasks:**

* Ingest and validate system state (LVM, fstab, mounts, filesystem signatures).
* Generate immutable `Vec<LvmAction>` plans.
* Transition execution engine to use robust `Vec<std::process::Command>` structures internally to eliminate shell escaping/quoting vulnerabilities, while printing human-readable strings for user confirmation.
* Require explicit confirmation before execution.
* Sequential, journaled execution with fail-fast halts on error.
* Post-condition verification of mounted filesystems.

**Success Metric:** A raw block device can be provisioned into a fully mounted, fstab-consistent filesystem in a single deterministic transaction.

## **Phase 2: `decommission` & E2E Reflexive Testing**

**High-Level Goal:** Safely remove storage without leaving dangling references, and establish the automated End-to-End (E2E) testing baseline.

**Breadth and Depth of Tasks:**

* Unmount filesystems cleanly.
* Remove LV, VG, and optionally PV in the strict, proper order.
* Update `/etc/fstab` atomically.
* Validate no dangling references or ghost mounts remain.
* **Implement CI/CD E2E Testing:** Spin up ephemeral VMs, generate random sets of loop devices, and execute reflexive command chains (e.g., `provision` followed by `decommission`).
* Verify that `Original State == Final State` post-execution.

**Success Metric:** Storage can be fully decommissioned deterministically, and the CI/CD pipeline mathematically proves the system returns to its exact original state without leaks.

## **Phase 3: `shrink**`

**High-Level Goal:** Enable safe shrinking of logical volumes where supported, strictly avoiding data loss.

**Breadth and Depth of Tasks:**

* Enforce strict operational order: filesystem shrink → LV shrink.
* Detect minimum active filesystem sizes before planning.
* Generate and execute immutable action plans.
* Verify post-conditions and journal operations for manual `repair`/`continue` support on failure.

**Success Metric:** LV shrinking can be executed safely, reproducibly, and with full programmatic verification of final block boundaries.

## **Phase 4: `evacuate**`

**High-Level Goal:** Safely remove a PV from a Volume Group without replacement.

**Breadth and Depth of Tasks:**

* Calculate required free extents on remaining PVs.
* Perform deterministic `pvmove` operations.
* Reduce and remove the evacuated PV.
* Verify VG integrity and journal execution.

**Success Metric:** A PV can be evacuated safely, mathematically guaranteeing all data is migrated and the VG remains consistent.

## **Phase 5: `replace-disk**`

**High-Level Goal:** Enable live disk replacement in a Volume Group.

**Breadth and Depth of Tasks:**

* Add new PV and extend VG.
* Perform `pvmove` from old PV to new PV.
* Remove old PV safely.
* Journal all steps, providing clear state markers for `lvq continue` if the operation is interrupted.

**Success Metric:** A disk can be replaced live, with minimal downtime and a clear recovery path if interrupted.

## **Phase 6: `shrink-xfs**`

**High-Level Goal:** Implement XFS shrink through canonical migration, as XFS fundamentally cannot shrink in place.

**Breadth and Depth of Tasks:**

* Create a new LV with the correct target size.
* Format and copy data safely (canonical migration).
* Swap mounts and update `/etc/fstab` atomically via `.tmp` staging.
* Remove original LV only after strict data verification.
* Journal the full workflow.

**Success Metric:** XFS shrink operations can be performed reliably with zero data loss and an inspectable audit trail.

## **Phase 7: `accelerate**`

**High-Level Goal:** Enable SSD caching safely for existing HDD-backed LVs.

**Breadth and Depth of Tasks:**

* Calculate correct cache-pool and metadata sizes.
* Attach cache in writeback/writethrough mode.
* Verify mode and ratio correctness.
* Journal operations for safe detachment/repair.

**Success Metric:** Fast block caching can be applied reliably without risking underlying data corruption or LVM misconfiguration.

## **Phase 8: `snap-back**`

**High-Level Goal:** Create application-consistent snapshots safely.

**Breadth and Depth of Tasks:**

* Detect filesystem type and invoke appropriate freeze operations (e.g., `fsfreeze`).
* Create LVM snapshot.
* Optionally mount snapshot read-only.
* Verify snapshot consistency and unfreeze the origin.
* Journal long-running operations.

**Success Metric:** Snapshots are consistent, verifiable, and safely mountable, supporting enterprise backup and testing pipelines.

## **Phase 9: CLI & Automation Enhancements**

**High-Level Goal:** Make `lvquick` fully automation-ready, machine-readable, and recoverable.

**Breadth and Depth of Tasks:**

* Implement JSON plan output (`--output json`) for integration with external pipelines.
* Implement non-interactive execution mode (`-y`, `--force`).
* **Finalize Transaction Inspection:** Polish `lvq history`, `lvq continue`, and `lvq repair` to act as the primary interface for dealing with halted or dirty system states.

**Success Metric:** All core commands can be used seamlessly in automated workflows, with robust programmatic hooks for handling partial failures.

## **Phase 10: Full Operational Suite (v1.0)**

**High-Level Goal:** Deliver a complete, deterministic, journaled LVM safety layer.

**Breadth and Depth of Tasks:**

* Stabilize all eight core commands.
* Verify transaction journaling and `repair`/`continue` logic across all workflows.
* Comprehensive unit, fuzzing, Kani formal proofs, and E2E reflexive testing.
* Perform a final architectural audit to ensure strict Separation of Concerns (SoC) is maintained.

**Success Metric:** `lvquick 1.0` provides a deterministic, mathematically sound, auditable, and production-ready transactional storage engine.

## **Beyond v1.0: The Ecosystem**

While the primary goal of `lvq` is a robust local execution engine, the architecture is designed to eventually integrate into modern Infrastructure-as-Code (IaC) and cloud-native ecosystems.

* **Ansible Integration (`ansible-collection-lvq`):** Leveraging `lvq`'s native idempotency and JSON outputs, a first-party Ansible collection will be built as a fast-follow to v1.0. This will replace brittle shell scripts with declarative Playbook support (`state: present`, `state: absent`). It will follow a very important workflow, if it finds a dirty state it decomissions or cleans up, then it provisions whatever was requested. 
* **Kubernetes Native (CSI Driver):** As a long-term roadmap goal, `lvq` will be adapted into a Container Storage Interface (CSI) driver. This will require wrapping the core logic in a long-running gRPC daemon, implementing node-level locking (e.g., `lvmlockd`) to handle high-concurrency requests, and dynamically provisioning volumes based on Kubernetes PVCs.

## **Summary Table**

| Phase | Focus Area | Target Version |
| --- | --- | --- |
| 1 | `provision` | 0.1.0 |
| 2 | `decommission` & Reflexive E2E Testing | 0.2.0 |
| 3 | `shrink` | 0.3.0 |
| 4 | `evacuate` | 0.4.0 |
| 5 | `replace-disk` | 0.5.0 |
| 6 | `shrink-xfs` | 0.6.0 |
| 7 | `accelerate` | 0.7.0 |
| 8 | `snap-back` | 0.8.0 |
| 9 | CLI Automation, `repair`, & `continue` | 0.9.0 |
| 10 | Full Operational Suite (`v1.0`) | 1.0.0 |
| *Next* | *Ansible Collection & Kubernetes CSI Driver* | *Post-v1.0* |


