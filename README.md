# Archaven

Put your Rust module dependency rules in tests.

Archaven scans Rust source files, finds dependencies between module paths, checks
them against your rules, and returns printable violations. Use it when a project
has boundaries that should stay true over time: domain code should not import
infrastructure, HTTP handlers should not reach straight into the database, or one
business module should not depend on another module's internals.

Start with one rule:

```rust
use archaven::{Access, Archaven, Rule};

#[test]
fn domain_does_not_depend_on_infrastructure() {
    let violations = Archaven::new()
        .rule(
            Rule::new()
                .named("domain purity")
                .deny(
                    Access::from("app::**::domain::**")
                        .to("app::**::infrastructure::**")
                        .because("domain code must not depend on infrastructure"),
                ),
        )
        .check("./src")
        .unwrap();

    violations.assert_empty();
}
```

That test fails when Archaven finds a dependency like:

```text
app::orders::domain::order -> app::orders::infrastructure::database
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
archaven = "1.1.0"
```

## Simple Examples

Ban one direction globally:

```rust
Rule::new()
    .named("domain purity")
    .deny(
        Access::from("app::**::domain::**")
            .to("app::**::infrastructure::**")
            .because("domain code must not depend on infrastructure"),
    )
```

Keep handlers out of persistence details:

```rust
Rule::new()
    .named("handlers use application services")
    .deny(
        Access::from("app::**::http::**")
            .to("app::**::database::**")
            .because("HTTP handlers should go through application services"),
    )
```

Allow only a narrow dependency shape inside each feature:

```rust
Rule::within("app::*")
    .named("feature internals")
    .deny_all()
    .allow(Access::from("http::**").to("application::**"))
    .allow(Access::from("application::**").to("domain::**"))
    .allow(Access::from("infrastructure::**").to("application::**"))
    .allow(Access::from("infrastructure::**").to("domain::**"))
    .because("feature dependencies should point through application and domain code")
```

Keep selected directories limited to Rust module root files:

```rust
Rule::directories("app::*")
    .named("module roots")
    .allow_only_module_roots()
```

These examples are intentionally plain. Archaven does not require you to adopt a
specific architecture style; it checks source-to-target module path policies that
fit your codebase.

## Similar Tools

Archaven follows the same general idea as
[ArchUnit](https://www.archunit.org/) in the Java ecosystem: architecture rules
should be executable tests, not comments in a diagram. ArchUnit works over Java
classes, packages, and bytecode-level concepts. Archaven keeps the same testing
habit, but applies it to Rust source files and Rust module paths.

It is also close in spirit to [Deptrac](https://deptrac.github.io/deptrac/) in
the PHP ecosystem. Deptrac groups code into layers and checks which layers may
depend on which other layers. Archaven does not use a separate YAML layer model;
rules are written directly in Rust tests with `Rule`, `Access`, and module path
patterns.

The goal is intentionally small: make dependency boundaries visible in normal
Rust test suites and CI.

## More Boundary Examples

Keep one feature from reaching into another feature's internals:

```rust
Rule::between("app::*")
    .named("feature boundaries")
    .deny_all()
    .allow(
        Access::from("**")
            .to("api::**")
            .because("features expose only their public API modules"),
    )
```

Keep plugin code from depending on the application shell:

```rust
Rule::new()
    .named("plugins stay independent")
    .deny(
        Access::from("plugins::**")
            .to("app::shell::**")
            .because("plugins should not depend on application shell internals"),
    )
```

Keep tests and support code from leaking into production modules:

```rust
Rule::new()
    .named("production does not use test support")
    .deny(
        Access::from("app::**")
            .to("test_support::**")
            .because("test helpers must stay out of production code"),
    )
```

Protect a shared kernel from depending on product-specific modules:

```rust
Rule::new()
    .named("shared kernel is product-neutral")
    .deny(
        Access::from("app::shared::**")
            .to_any(["app::billing::**", "app::sales::**"])
            .because("shared code should not depend on product-specific modules"),
    )
```

## Example Project

The repository includes a small runnable example in
[`examples/basic_architecture_test.rs`](examples/basic_architecture_test.rs).
It scans the sample source tree under `examples/sample_app/src` and demonstrates
the simple rules from this README.

Run it with:

```bash
cargo test --example basic_architecture_test
```

The example is deliberately small. It is meant to show how an architecture test
looks in a normal Rust project before you move on to larger modular-monolith
rules.

## Who This Is For

Archaven is most useful once a Rust codebase has boundaries that people can name:
features, bounded contexts, application/domain/infrastructure folders, plugins,
adapters, or shared modules. It is a good fit for teams that already review
module dependencies by convention and want those conventions to run in CI.

It is probably too much for tiny crates with only a few modules. In that case,
regular Rust visibility, module organization, and code review may be enough.

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

By default, scanning records only dependencies that resolve to local project
modules. This keeps small rules quiet: `use std::sync::Arc;` or
`use serde::Deserialize;` will not appear in the dependency graph unless you ask
Archaven to include external dependencies.

Use `include_external_dependencies` when architecture rules should also see
imports from crates outside the current project:

```rust
let violations = Archaven::new()
    .include_external_dependencies()
    .rule(/* rule */)
    .check("./src")
    .unwrap();
```

With that option enabled, Archaven compares each path root with the roots found
in the scanned source tree. If the root is not local, it is treated as external.

For a project with local roots `app` and `shared`, this code:

```rust
use crate::app::orders::domain::Order;
use shared::error::AppError;
use sea_orm::ConnectionTrait;
use axum::Json;
use std::sync::Arc;
```

records dependencies like:

```text
app::orders::domain::Order
shared::error::AppError
sea_orm::ConnectionTrait
axum::Json
```

`std`, `core`, `alloc`, and `proc_macro` are ignored by this external-dependency
mode because they are Rust standard roots and usually add noise to architecture
tests.

The scanner also handles simple `use ... as ...` aliases in the same file:

```rust
use sea_orm as orm;
use sea_orm::DatabaseConnection as Db;

fn run(connection: Db) {
    orm::Statement::from_sql_and_values;
}
```

is recorded as:

```text
sea_orm
sea_orm::DatabaseConnection
sea_orm::Statement::from_sql_and_values
```

This alias handling is intentionally simple. It is meant to keep common imports
readable in architecture tests, not to replace Rust's compiler resolver.

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

Use `ignore_module_roots` when both Rust module root styles should be skipped:

```rust
Rule::within("app::*::*")
    .named("module internals")
    .deny_all()
    .ignore_module_roots()
    .allow(Access::from("application::**").to("domain::**"))
```

Module root files are `mod.rs`, `lib.rs`, and files named after a child
directory, such as `application.rs` when an `application/` directory exists.

### `Rule::directories`

`Rule::directories(pattern)` checks the physical Rust files directly inside
directories whose module path matches the pattern.

```rust
Rule::directories("app::*")
    .named("module roots")
    .allow_only_module_roots()
```

Directory rules intentionally support one or more `*` segments and do not
support `**`. The full pattern chooses the directory level to check, so the last
`*` is the checked level. For example, `app::*` checks directories such as
`app::orders` and `app::billing`, while `app::*::*` checks directories such as
`app::sales::orders`.

`allow_only_module_roots` allows only these `.rs` files in each matching
directory:

- `mod.rs`
- `lib.rs`
- `<child-directory>.rs`

For example, this layout is allowed:

```text
src/app/orders/
  mod.rs
  application.rs
  domain.rs
  application/
    command.rs
  domain/
    order.rs
```

but `src/app/orders/helper.rs` is reported unless an `orders/helper/` directory
also exists.

### Multiple Rules

Rules are independent and can be combined in one checker:

```rust
let violations = Archaven::new()
    .rule(
        Rule::directories("app::*")
            .named("module roots")
            .allow_only_module_roots(),
    )
    .rule(
        Rule::between("app::*")
            .named("bounded contexts")
            .deny_all()
            .allow(
                Access::from("*::infrastructure::adapter::**")
                    .to_any([
                        "*::application::command::**",
                        "*::application::query::**",
                    ]),
            ),
    )
    .rule(
        Rule::within("app::*::*")
            .named("module internals")
            .deny_all()
            .ignore_module_roots()
            .allow(Access::from("application::**").to("domain::**"))
            .allow(Access::from("infrastructure::**").to("application::**"))
            .allow(Access::from("infrastructure::**").to("domain::**"))
            .allow(Access::from("ui::**").to("application::**")),
    )
    .check("./src")?;

violations.assert_empty();
```

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

### `Access`

`Access::from(...).to(...)` describes a source-to-target dependency shape used
by `deny`, `allow`, and `deny_all` allowlists. Use `to_any` when one access
shape has several target patterns.

Use `except_to` to exclude narrower target patterns from that access match:

```rust
Rule::new()
    .named("application database boundary")
    .deny(
        Access::from("app::**::application::**")
            .to("sea_orm::**")
            .except_to(["sea_orm::DatabaseConnection"])
            .because("application code may use only the approved SeaORM boundary type"),
    )
```

`except_to` applies to both explicit denies and allows. Under `deny_all`, an
allowed access with `except_to` does not allow the excepted targets.

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

## Larger Modular Monolith Example

The same primitives scale to a modular monolith. Start by describing the module
paths that matter, then decide which source-to-target dependencies are allowed.

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
- target module path for dependency violations
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
- non-local root paths such as `sea_orm::...` or `axum::...` when
  `Archaven::include_external_dependencies()` is enabled
- simple aliases from top-level `use ... as ...` items, such as
  `use sea_orm as orm;` followed by `orm::ConnectionTrait`

When external dependencies are included, Archaven deliberately does not record
single-segment paths such as `Order`, `Result`, or `DatabaseConnection` as
external dependencies. Those names are often local types, generics, or items
already recorded by their `use` statement. Multi-segment unknown roots such as
`sea_orm::ConnectionTrait` are treated as external.

Archaven ignores `std`, `core`, `alloc`, and `proc_macro` in external-dependency
mode. Rules can still match local modules with those names if they are actual
source roots in the scanned project.

Use `Rule::ignore_module_roots()` when module root files intentionally wire
internals that the rule should not evaluate. Use
`Rule::directories("app::*").allow_only_module_roots()` when matching
directories should contain only Rust module root files.

This keeps Archaven fast and usable from regular tests. It also means Archaven
is a source-level checker, not a full Rust compiler front-end. Macro-generated
paths, complex re-exports, trait dispatch, scoped/shadowed aliases, and every
possible Rust name-resolution pattern may require explicit paths in code or
future scanner improvements.

## Custom Rule Sets

The built-in `Rule` type is enough for many projects, but Archaven also exposes
`RuleSet` for custom policies. Custom rules can inspect discovered dependencies
and source directories through `DependencyGraph`.

```rust
use archaven::{ArchavenError, DependencyGraph, RuleSet, Violations};

struct MyRule;

impl RuleSet for MyRule {
    fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
        for dependency in graph.dependencies() {
            let _ = (dependency.source(), dependency.target());
        }

        for directory in graph.directories() {
            let _ = (
                directory.path(),
                directory.module(),
                directory.files(),
                directory.child_directories(),
            );
        }

        Ok(Violations::new())
    }
}
```

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT license

at your option.
