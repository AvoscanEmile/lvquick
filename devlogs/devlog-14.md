# Devlog Entry 14 — Dual-Tier CI/CD Pipeline Implementation and Codebase Refactoring

**Date**: 2026-03-13

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

The primary objective of this cycle was to establish an automated Continuous Integration and Continuous Deployment (CI/CD) pipeline to enforce the high-assurance guarantees of the `lvq` project. Initially conceptualized as a straightforward automated test runner to execute `cargo test` on every push, the objective rapidly expanded into building a comprehensive, dual-tier release forge and refactoring the project structure to adhere to idiomatic Rust standards, ensuring true enterprise-grade software distribution.

## Implementation

### Initial CI Workflow Formation

A foundational `.github/workflows/ci.yml` file was established to trigger on all pushes and pull requests to the main branch. This initial implementation utilized an `ubuntu-latest` runner and included steps for installing a stable Rust toolchain, leveraging dependency caching (`Swatinem/rust-cache@v2`), and executing standard unit and property tests via `cargo test --verbose`.

### Integration of Formal Verification and Fuzzing

To ensure the project's rigorous mathematical and operational constraints were continuously validated, the CI pipeline was expanded into parallel jobs. A dedicated verification job was implemented using the `model-checking/kani-github-action@v1` to run symbolic execution proofs. Simultaneously, a fuzzing smoke test was introduced. This required configuring a parallel job to install the nightly Rust toolchain, install the `cargo-fuzz` binary, and execute the `full_pipeline` harness with a `-max_total_time=60` constraint to identify immediate panics without exhausting runner minutes.

### Structural Codebase Refactoring

To support the expanded testing infrastructure and adhere to idiomatic Rust patterns, a structural refactor was performed. The monolithic `src/main.rs` file was divided. A new `src/lib.rs` was created to expose the internal modules (`core`, `parser`, `planner`, `verifier`, `exec`) as a library crate. The `src/main.rs` file was subsequently reduced to a thin binary wrapper, responsible exclusively for verifying administrative privileges (`is_root()`), parsing CLI arguments, and routing execution flow through the newly established library crate.

### Proptest Configuration Refactoring

To allow dynamic scaling of property test iterations across different pipeline tiers, the hardcoded `cases = 100_000` configurations were removed from the `proptest!` macro blocks within the source code. Instead, the `PROPTEST_CASES` environment variable was implemented within the workflow files to dictate execution depth dynamically.

### Dual-Tier Pipeline Architecture Construction

The CI/CD strategy was bifurcated into an Integration Tier and a Release Tier to balance developer velocity with deployment safety. The existing `ci.yml` was maintained as a fast, integration gatekeeper with `PROPTEST_CASES` set to 10,000. A new `cd.yml` (Release Forge) was engineered to trigger exclusively on Git tags matching `v*`. This deployment workflow was configured with heavy-duty testing parameters, including an extended 600-second fuzzing campaign and a `PROPTEST_CASES=100000` environment variable for deep permutation validation. A final `release` job was added, utilizing the `needs:` array to enforce a strict dependency on the successful completion of all tests. Upon validation, this job compiles an optimized `--release` binary and utilizes `softprops/action-gh-release` to automatically publish the binary and generate release notes.

## Challenges & Resolutions

### Redundant Test Execution and Build Target Conflicts

* **Challenge:** The execution of `cargo test` resulted in the test suite running twice simultaneously, alongside numerous "dead code" warnings for `main` and `is_root` functions. It was discovered that `Cargo.toml` explicitly pointed both `[bin]` and `[lib]` targets to the identical `src/main.rs` file, causing Cargo to compile and test the entire project in two separate contexts.
* **Solution:** The explicit path directives were removed from `Cargo.toml`. The codebase was refactored into a standard binary/library split (as detailed in the Implementation section). This successfully isolated the test suite to the library context, eliminating redundant executions and suppressing dead code warnings.

### Proptest Environment Variable Precedence

* **Challenge:** An attempt was made to scale property testing in the CI environment by passing `PROPTEST_CASES=10000` as an environment variable. However, it was noted that hardcoded `cases = 100_000` declarations within the macro blocks in the source code would override the environment variable, potentially causing CI runs to exceed acceptable time limits.
* **Solution:** The hardcoded `with_cases` configurations were completely removed from the Rust codebase. This allowed the `PROPTEST_CASES` environment variable defined in the `ci.yml` and `cd.yml` files to successfully and dynamically control the testing depth, optimizing runtime based on whether the pipeline was executing standard integration or a full release forge.

## Testing & Validation

Validation was conducted by pushing the workflow configurations to the repository and monitoring the GitHub Actions runner environments. The parallel execution of Kani proofs, standard tests, and libFuzzer was observed to complete successfully within the allocated timeframes. Furthermore, the codebase refactor was validated locally by executing `cargo +nightly fuzz run full_pipeline`, confirming that the fuzzer correctly hooked into the newly established library crate without compilation errors or warnings.

## Outcomes

A self-verifying software repository was successfully established. The project now benefits from a fast, automated integration gatekeeper that runs on every commit, alongside a highly sophisticated release forge that automatically compiles and deploys production-ready binaries upon successful mathematical and chaotic validation. The codebase structure is now fully idiomatic and strictly separated between logic and execution.

## Reflection

The evolution of this CI/CD pipeline underscores a critical principle in high-assurance systems engineering: confidence must be automated, not assumed. What began as a superficial desire to achieve a "green checkmark" naturally progressed into a rigorous deployment forge, driven by the project's foundational commitment to the "Clean | Dirty | Done" state machine. By enforcing architectural constraints not just within the Rust code, but across the entire distribution infrastructure, the project transcends being a mere scripting utility and establishes itself as a verifiable, enterprise-grade LVM orchestration engine.

## Next Steps

With the logic mathematically proven, the architecture structurally sound, and the deployment pipeline fully automated, the immediate next step is the comprehensive documentation of the project. The final phase before officially cutting the v0.1.0 release will involve finalizing the `README.md`, polishing the `Architecture.md` to reflect the underlying design philosophies, and generating the final `CHANGELOG.md`.

