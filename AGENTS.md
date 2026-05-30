# AGENTS.md

Instructions for AI coding agents working in the Archaven repository.

For consumer-facing guidance on using Archaven from another project, read
`llms.txt`. This file is for contributors editing this repository.

## Project Summary

Archaven is a public Rust crate for architecture tests over Rust module paths.
It scans Rust source files, extracts source-to-target dependencies, checks them
against user-defined rules, and returns printable violations.

The public API should stay small and neutral:

- `Archaven`
- `Rule`
- `Access`
- `Violations`
- `Violation`
- `RuleSet`

Do not add architecture-specific public types such as `Layers`, `Boundary`,
`OnionArchitecture`, or `ModuleIsolation` unless the design is explicitly
reopened and approved.

## Repository Shape

- `src/lib.rs`: public entry point and re-exports.
- `src/path.rs`: segment-based Rust module path and pattern matching.
- `src/rule.rs`: `Rule`, `Access`, and `RuleSet` evaluation.
- `src/graph.rs`: dependency graph model.
- `src/scanner.rs`: `syn`-based source scanner.
- `src/violation.rs`: violation data and display formatting.
- `src/error.rs`: crate error type.
- `tests/patterns.rs`: matcher behavior.
- `tests/rules.rs`: rule behavior on explicit graphs.
- `tests/check.rs`: end-to-end source scanning behavior.
- `README.md`: human-facing crate documentation.
- `llms.txt`: consumer-facing guide for AI agents using Archaven.

## Design Principles

- Keep the crate library-first. Do not add a CLI, YAML config, visualizer, or
  framework-specific presets without an explicit design discussion.
- Keep rule naming neutral. The crate is a dependency policy engine over Rust
  module paths, not an opinionated onion/layer/modular-monolith framework.
- Keep APIs ergonomic for tests. `check` returns `Result<Violations,
  ArchavenError>`; architecture violations are data, not technical errors.
- Keep path matching segment-based. `*` matches exactly one segment, and `**`
  matches one or more segments.
- Prefer clear error and violation messages over clever abstractions.

## Implementation Notes

- The scanner is intentionally source-level, not a full Rust compiler front-end.
- It derives source module paths from file paths under the directory passed to
  `Archaven::check`.
- It currently records dependencies from `use crate::...`, `crate::...`,
  `self::...`, `super::...`, and local root paths such as `app::...`.
- Be careful with macro-generated paths, re-exports, trait dispatch, and aliases;
  document scanner limitations when touching scanner behavior.

## Development Workflow

Use test-driven changes for behavior. Add or update tests before changing
production code.

Run these before claiming work is complete:

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo package --allow-dirty
```

Do not use `--allow-dirty` for real `cargo publish`. It is acceptable for local
package verification during development.

For library hygiene:

- Do not commit `Cargo.lock`.
- `AGENTS.md` is excluded from crates.io packaging.
- `llms.txt` is included in the package for downstream AI-agent guidance.

## Release Checklist

Before publishing a new version:

1. Update `version` in `Cargo.toml`.
2. Run the full verification commands above.
3. Run `cargo publish --dry-run` from a clean git state.
4. Commit the release change.
5. Tag the commit as `vX.Y.Z`.
6. Push commits and tags.
7. Run `cargo publish` without `--allow-dirty`.
