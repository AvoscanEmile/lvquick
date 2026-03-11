# Devlog Entry [07] — Establishing Formal and Property-Based Verification for the `core` module

**Date**: 2026-03-10

**Author**: Emile Avoscan

**Target Version**: 0.1.0

## Main Objective

The primary goal of this development cycle was to transform the **`LvRequest`** and its constituent types—such as **`SizeUnit`**—from isolated parsers into a **reflexive, high-assurance ingestion model**. By implementing a robust, colon-delimited parser and establishing mathematical guarantees against arithmetic overflow using **Kani formal verification**, the system now ensures total structural integrity. Through a hybrid strategy of property-based fuzzing and **reflexive round-trip testing**, the core was hardened into a **Trusted Computing Base (TCB)**, ensuring that user intent is captured without data loss before being transitioned to the provisioning planner.

## Implementation

The implementation was executed through a series of modular refinements designed to separate string orchestration from core logic, thereby enabling targeted verification.

#### Module Tree Integration

Initial attempts to run Kani proofs failed because the `tests.rs` file was not reachable by the compiler. To resolve this, the module was explicitly declared in `src/core/mod.rs` using `#[cfg(kani)] mod kani_proofs;` and `#[cfg(test)] mod tests;`. This separation ensured that heavy testing dependencies like `proptest` did not interfere with the formal verification environment.

#### Refactoring `FromStr` for Testability

The `FromStr` implementation for `SizeUnit` was refactored to delegate logic to two private helper functions: `parse_percentage` and `parse_absolute`. This architectural decision allowed the dispatcher to remain a simple "router" while the complex parsing logic could be scrutinized independently. Trimming and case-normalization were centralized in the main `from_str` entry point to ensure consistent input handling.

#### Exhaustive Domain Validation

For the `ValidPercentage` type (a wrapper around `u8`), an exhaustive unit test was implemented. Given that the state space was limited to 256 possible values, a linear loop was utilized to verify every possible input. This provided a "Proof by Exhaustion," confirming that the type correctly enforced the $1..=100$ range.

#### Arithmetic overflow-proof `to_bytes`

The `to_bytes` method was implemented to convert all absolute `SizeUnit` variants into `u128` byte counts. Constants for Sectors ($512$), KiB ($2^{10}$), MiB ($2^{20}$), GiB ($2^{30}$), TiB ($2^{40}$), PiB ($2^{50}$), and EiB ($2^{60}$) were verified for accuracy. The implementation returns a `Result` to gracefully handle variants like `Percentage` that require external context (Volume Group size) to calculate bytes.

#### Formal Verification with Kani

To eliminate the risk of arithmetic overflow during byte conversion, formal symbolic execution was employed via Kani. A proof was constructed to demonstrate that even in the "worst-case" scenario (Exabytes), a `u64` input could be safely converted to a `u128` byte count without wrapping. This provides a mathematical guarantee of safety for the storage backend.

#### Orchestration of the LvRequest Parser

The `LvRequest` structure was implemented with an "Airtight" `FromStr` logic. A colon-delimited parser was designed to handle a variable number of segments (name:size:fs:mount). Guardrails were integrated to fail fast in the event of invalid LVM naming conventions (e.g., names starting with hyphens) or structural logical errors (e.g., providing a mount path without a filesystem).

#### Reflexive Modeling via Display Trait

To facilitate exhaustive round-trip testing, the `Display` trait was implemented for all core enums and structs. While these implementations are primarily utilized for verification within the test harnesses rather than the final execution path, they allow the system to prove that its internal representation can be serialized and deserialized without the loss of a single bit of information.

## Challenges & Resolutions

**Kani Reachability Failure**: Proof harnesses were initially invisible to the Kani cargo plugin.
* **Resolution**: The `tests.rs` and `kani_proofs.rs` files were manually added to the crate's module tree in `mod.rs`, ensuring they were "reachable" during the symbolic execution analysis.

**String Unwinding in Kani**: Kani attempted to infinitely unroll loops within the Rust standard library (specifically `memchr` and UTF-8 validation) when processing symbolic strings.
* **Resolution**: After attempting to use `#[kani::unwind]`, it was determined that formal string verification was too computationally expensive for this cycle. The strategy was pivoted to use Proptest for string parsing, while Kani was reserved for pure arithmetic verification where no unwinding is required.

**Symbolic Type Constraints**: Kani's `kani::any()` function requires the `Arbitrary` trait, which was missing from custom types.
* **Resolution**: `#[cfg_attr(kani, derive(kani::Arbitrary))]` was added to `ValidPercentage` and `PercentTarget` to allow Kani to generate symbolic instances of these types without adding overhead to the production binary.

**Non-deterministic Chaos Tests**: In Proptest, a random string generator for "junk" inputs would occasionally generate a valid string (e.g., "50%FREE"), causing intermittent assertion failures.
* **Resolution**: The assertion logic was updated to be conditional; if a "chaos" string happened to be valid, the test verified it matched the expected variant rather than strictly asserting an error.

**Index Out of Bounds in Composite Parsing**: During the implementation of the `LvRequest` parser, direct indexing of the `parts` vector posed a panic risk for minimal inputs like `name:size`.
* **Resolution**: Defensive programming was applied using `.get()` and `unwrap_or("")` to safely handle optional segments, ensuring the parser returns a clean `Err` instead of crashing.

**Silent Data Loss in Structural Mismatches**: A concern was raised regarding the string format `name:size::/path`, where the filesystem is missing but a path is provided.
* **Resolution**: An explicit structural guardrail was added to return an error if a mount path is detected without a corresponding filesystem, enforcing "Correct-by-Construction" user intent.

**Grammar Ambiguity between E and EB**: It was discovered that the initial parser logic did not distinguish between "E" (standard LVM for Exabytes) and an empty string (standard for Extents).
* **Resolution**: The match arms were refactored to canonicalize both "E" and "EB" to Exabytes, reserving the empty suffix exclusively for Logical Extents.

## Testing & Validation

The suite was expanded to 13 high-assurance tests, combining different methodologies to cover the entire state space:

* **Exhaustive u8 Scanning**: The `ValidPercentage` logic was verified by looping through all 256 possible `u8` values to confirm strictly enforced 1–100 boundaries.
* **Proptest Fuzzing**: `SizeUnit` and `Filesystem` were hit with 100,000 randomized cases to verify casing normalization and whitespace handling.
* **Structural Chaos Testing**: The `LvRequest` parser was tested against 50,000 permutations of colon-delimited strings, ensuring that "too many" or "too few" segments were caught.
* **Reflexivity Invariants**: A reflexivity test was implemented where random `LvRequest` structs were generated, turned into strings via `Display`, and parsed back. The test confirmed that `original == from_str(original.to_string())` across 50,000 cases.

## Outcomes

The `core` module is now considered a **Trusted Computing Base (TCB)**. The system has shifted from a state of "probable correctness" to "proven robustness." The transformation from `LvRequest` to the `planner` can now proceed with the absolute certainty that the input data is sanitized, unambiguous, and mathematically safe.

## Reflection

The emergence of reflexive testing (round-trip verification) has fundamentally changed the development approach. It highlights that the most robust way to build an orchestrator is to ensure that the internal types and the external grammar are perfectly symmetrical. Even though the `Display` implementations were added solely for testing, they serve as a canonical definition of the system's "truth," which prevents the drift between what the user intends and what the code understands.

There is a distinct architectural satisfaction in seeing emergent behaviors—like the interaction between different units—being caught by the compiler before they ever reach a production environment. This cycle reinforced the "Airtight" philosophy: every line of parsing logic must be either mathematically proven or fuzzed to exhaustion.

## Next Steps

With the core logic validated and refactored into a reflexive model, the focus shifts to the **Parser Module**. This will involve extending the established testing harnesses to the external-facing components of the system. The objective will be to apply the same level of rigor to the parsing of system state and command-line arguments as was applied to the internal core.
