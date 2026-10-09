# Repository Instructions

## Project shape

- `cstyle` is a single Rust 2024 crate. Use the recipes in `Justfile` rather than invoking Cargo directly.
- `src/main.rs` calls `cstyle::cli::run_from_env()`. The public API is in `src/api.rs`, options and config lookup in `src/config/`, formatting in `src/formatter/`, CLI and file handling in `src/cli/`, and shared input handling in `src/source/`.
- See `README.md` for usage and configuration. See `src/formatter/mod.rs` for the formatting pipeline and module groups.

## Commands

- List commands: `just --list`.
- Format Rust: `just fmt`; check formatting: `just fmt-check`.
- Run tests through nextest: `just test`; filter by name: `just test <filter>`.
- Run an integration-test target: `just test-target <target>` (`format`, `cli`, or `perf_bounded`).
- Run release-mode bounded-runtime tests: `just perf-bounded`.
- Build debug or release binaries: `just build` or `just build-release`.
- Run the full release gate: `just check`.
- For CLI checks, run `just build`, then `target/debug/cstyle ...`. Pass `--options=none --project=none` to isolate formatting checks from user and project configuration.

## Defect handling

- Treat divergences from declared compatibility behavior as defects. Evaluate the behavior and its fix, not authorship, timing, task boundaries, or which mechanism caused it; these are not grounds for deferral.
- Fix divergences discovered during the work, including adjacent styles, options, and inputs, in the same pass. Each requires its own failing regression test before a fix.
- Stop only for a required user decision, destructive action needing authorization, or a documented intentional divergence. State the exact blocker.

## Compatibility

- Compatibility expectations must be platform-neutral and reproducible. Local executables and temporary comparison output are evidence, not authoritative specifications.
- Match the intended, consistent behavior of the supported AStyle options. Depart from reference behavior only when it is self-contradictory or inconsistent across equivalent positions or inputs; otherwise, require strict parity.
- Pin every intentional divergence with a test asserting the cstyle invariant. Test names and comments describe behavior, not reference builds or investigation history.

## Formatting invariant

- Every source change must be governed by a concrete default, config, or CLI option. Otherwise, preserve the source byte-for-byte.
- Original whitespace is the baseline; options apply only their required changes. Padding adds missing spaces; unpadding removes them. Rebuilding output must not collapse alignment such as `int  value  =  3;`, multiple spaces, or tabs unless an option governs that change.
- A modification not explained by an option is a defect.

## Code style

- Keep the Rust implementation independent. Do not use upstream implementation names or source citations in code, comments, or identifiers; public compatibility names remain part of the interface.
- Keep committed content portable: no personal paths, private project details, local installations, temporary working documents, or investigation history. Names and comments describe public behavior or durable invariants.
- Prefer self-documenting code and the existing low comment density. Comments explain non-obvious invariants, not change history or obvious operations.

## Formatter architecture

- Compute whitespace before emission from token and line structure, parser state, and indentation or continuation frames. Do not repair emitted strings by matching narrow line shapes.
- Fix the general state transition, classification, frame, or indentation rule for equivalent syntax. Do not special-case snippets, token strings, or fixture shapes to satisfy a test.
- Add or correct structural state when a fix needs missing context, including continuation anchors and macro or preprocessor context.
- Limit post-processing to style-wide mechanical transforms, such as configured brace joining or splitting; never use it for scenario-specific syntax repairs.

## Regression tests

- For every regression or compatibility defect, write a failing test before changing the formatter. Confirm the failure reproduces the defect, implement the fix, then run the test and confirm it passes.
- Pin a neutral minimal input, exact options, and expected output. Use `tests/format/regression.rs` unless a more specific existing test module fits better.

## Tests and examples

- Use generic examples, not proprietary code or identifiers. Reduce reports from private code to minimal cases with names such as `value`, `result`, `helper`, `Config`, or `Item`; do not tailor formatting to private projects.
- Manually review touched tests and related source test modules for non-neutral names in test names, input strings, comments, and helpers; search alone is insufficient.
- Neutral renames must preserve syntax, casing, macro shape, namespace or member form, line-length thresholds, and recognized language or framework patterns. Run the focused test after every semantic rename.
