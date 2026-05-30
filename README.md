# Archaven

Archaven is a Rust library for checking dependency rules inside Rust projects.

It scans Rust source files, extracts dependencies between module paths, runs your
rules against those dependencies, and returns a printable list of violations.
The primary use case is an architecture test that fails when code crosses a
boundary it should not cross.

```rust
use archaven::{Access, Archaven, Rule};

#[test]
fn architecture_rules_are_respected() {
    let violations = Archaven::new()
        .rule(
            Rule::between("app::*")
                .named("bounded contexts")
                .deny_all()
                .allow(
                    Access::from("*::infrastructure::adapter::**")
                        .to_any([
                            "*::application::command::**",
                            "*::application::query::**",
                        ])
                        .because("bounded contexts communicate through adapters and command/query APIs"),
                ),
        )
        .check("./src")
        .unwrap();

    assert!(violations.is_empty(), "{violations}");
}
```

## What Archaven Checks

Archaven works with Rust module paths such as:

```text
app::sales::orders::domain::order
app::sales::orders::infrastructure::adapter::billing_client
app::billing::invoices::application::command::issue_invoice
```

For each dependency found in the source tree, Archaven asks:

```text
source path -> target path
```

Then every configured `Rule` decides whether that dependency is allowed. A rule
can describe boundaries between scopes, dependencies inside one scope, or a
global source-to-target policy.

## Installation

Add Archaven as a dev dependency:

```toml
[dev-dependencies]
archaven = "0.2"
```

## Core Concepts

### `Archaven`

`Archaven` is the checker. Add one or more rules, call `check`, and assert that
the returned `Violations` collection is empty.

```rust
let violations = Archaven::new()
    .rule(/* rule */)
    .check("./src")
    .unwrap();

violations.assert_empty();
```

`check` returns `Result<Violations, ArchavenError>`:

- `Err(...)` means scanning, parsing, or rule compilation failed.
- `Ok(violations)` means analysis completed and the returned list contains all
  architectural violations.

### `Rule::between`

`Rule::between(pattern)` checks dependencies between different instances matched
by the same scope pattern.

```rust
Rule::between("app::*")
```

With paths like `app::sales::...` and `app::billing::...`, this checks
cross-context dependencies from `sales` to `billing` and from `billing` to
`sales`. Dependencies inside `app::sales` are ignored by this rule.

Example:

```rust
Rule::between("app::*")
    .named("bounded contexts")
    .deny_all()
    .allow(
        Access::from("*::infrastructure::adapter::**")
            .to_any([
                "*::application::command::**",
                "*::application::query::**",
            ]),
    )
```

The `from` and `to` patterns are relative to the matched source and target
scopes. For a dependency from `app::sales` to `app::billing`, the rule above
allows:

```text
app::sales::*::infrastructure::adapter::**
    ->
app::billing::*::application::command::**
app::billing::*::application::query::**
```

### `Rule::within`

`Rule::within(pattern)` checks dependencies inside the same matched scope.

```rust
Rule::within("app::*::*")
```

With a scope like `app::sales::orders`, this checks dependencies from one path
under `orders` to another path under `orders`. Dependencies from `orders` to
`invoices` are ignored by this rule.

Example:

```rust
Rule::within("app::*::*")
    .named("module internals")
    .deny_all()
    .allow(Access::from("application::**").to("domain::**"))
    .allow(Access::from("infrastructure::**").to("application::**"))
    .allow(Access::from("infrastructure::**").to("domain::**"))
    .allow(Access::from("ui::**").to("application::**"))
```

If a file assembles a module and should not be evaluated by that rule, ignore it
with a file glob:

```rust
Rule::within("app::*::*")
    .named("module internals")
    .deny_all()
    .ignore_files(["**/mod.rs"])
    .allow(Access::from("application::**").to("domain::**"))
```

`ignore_files` is per-rule. Dependencies discovered in matching source files are
skipped for that rule before `deny`, `deny_all`, and `allow` are evaluated.

This can model layered, hexagonal, vertical-slice, plugin, or custom dependency
styles without adding architecture-specific types to the public API.

### `Rule::new`

`Rule::new()` checks absolute source and target patterns.

```rust
Rule::new()
    .named("domain purity")
    .deny(
        Access::from("app::**::domain::**")
            .to("app::**::infrastructure::**")
            .because("domain code must not depend on infrastructure"),
    )
```

Use this when a rule is not scoped by `between` or `within`.

## Path Patterns

Archaven matches module paths by `::` segments.

```text
*  = exactly one segment
** = one or more segments
```

Examples:

```text
app::*::anything       matches app::orders::anything
app::*::anything       does not match app::orders::nested::anything
app::**::anything      matches app::orders::nested::anything
app::**::anything      does not match app::anything
app::**                matches app::orders and app::orders::domain
app::**                does not match app
```

Patterns are intentionally segment-based. `app::*::domain` is different from
`app::**::domain`, and `**` never means "zero segments".

## Modular Monolith Example

Given this source layout:

```text
src/app/sales/orders/domain/order.rs
src/app/sales/orders/application/command/create_order.rs
src/app/sales/orders/infrastructure/adapter/billing_client.rs
src/app/sales/orders/ui/http_controller.rs
src/app/billing/invoices/application/command/issue_invoice.rs
src/app/billing/invoices/application/query/get_invoice.rs
```

Check cross-context communication:

```rust
use archaven::{Access, Archaven, Rule};

#[test]
fn bounded_contexts_are_isolated() {
    let violations = Archaven::new()
        .rule(
            Rule::between("app::*")
                .named("bounded contexts")
                .deny_all()
                .allow(
                    Access::from("*::infrastructure::adapter::**")
                        .to_any([
                            "*::application::command::**",
                            "*::application::query::**",
                        ])
                        .because("cross-context calls go through adapters and command/query APIs"),
                ),
        )
        .check("./src")
        .unwrap();

    assert!(violations.is_empty(), "{violations}");
}
```

Check dependencies inside each module:

```rust
use archaven::{Access, Archaven, Rule};

#[test]
fn module_internal_dependencies_are_valid() {
    let violations = Archaven::new()
        .rule(
            Rule::within("app::*::*")
                .named("module internals")
                .deny_all()
                .allow(Access::from("application::**").to("domain::**"))
                .allow(Access::from("infrastructure::**").to("application::**"))
                .allow(Access::from("infrastructure::**").to("domain::**"))
                .allow(Access::from("ui::**").to("application::**"))
                .because("dependencies inside a module must point inward"),
        )
        .check("./src")
        .unwrap();

    assert!(violations.is_empty(), "{violations}");
}
```

## Violation Output

`Violations` implements `Display`, so it can be used directly in assertions:

```rust
assert!(violations.is_empty(), "{violations}");
```

For test assertions, `assert_empty` panics with the formatted violation list and
reports the panic at the assertion call site:

```rust
violations.assert_empty();
```

You can also format violations yourself:

```rust
for violation in &violations {
    println!(
        "{}: {} -> {}",
        violation.location().file().display(),
        violation.source(),
        violation.target(),
    );
}
```

A violation contains:

- rule name
- reason
- source module path
- target module path
- file path
- line and column when available

## How Scanning Works

Archaven derives source module paths from Rust file paths under the directory
passed to `check`.

For example:

```text
src/app/sales/orders/domain/order.rs
```

becomes:

```text
app::sales::orders::domain::order
```

The scanner parses Rust files with `syn` and records dependencies from:

- `use crate::...`
- `crate::...`
- `self::...`
- `super::...`
- local root paths such as `app::...`

Use `Rule::ignore_files(["**/mod.rs"])` when module composition root files
intentionally wire internals that the rule should not evaluate.

This keeps Archaven fast and usable from regular tests. It also means the first
version is a source-level checker, not a full Rust compiler front-end. Macro
generated paths, complex re-exports, trait dispatch, and every possible aliasing
pattern may require explicit paths in code or future scanner improvements.

## Custom Rule Sets

The built-in `Rule` type is enough for many projects, but Archaven also exposes
`RuleSet` for custom policies.

```rust
use archaven::{ArchavenError, DependencyGraph, RuleSet, Violations};

struct MyRule;

impl RuleSet for MyRule {
    fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
        let _ = graph;
        Ok(Violations::new())
    }
}
```

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT license

at your option.
