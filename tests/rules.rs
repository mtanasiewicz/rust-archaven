use archaven::{Access, Dependency, DependencyGraph, Location, ModulePath, Rule};

const COMPOSITION_ROOT_GLOBS: &[&str] = &["**/mod.rs"];

fn dep(source: &str, target: &str, file: &str) -> Dependency {
    Dependency::new(
        ModulePath::parse(source).unwrap(),
        ModulePath::parse(target).unwrap(),
        Location::new(file),
    )
}

fn dep_in_example(source: &str, target: &str) -> Dependency {
    dep(source, target, "src/example.rs")
}

#[test]
fn between_rule_allows_only_configured_cross_scope_access() {
    let graph = DependencyGraph::from_dependencies([
        dep_in_example(
            "app::sales::orders::infrastructure::adapter::billing_client",
            "app::billing::invoices::application::command::issue_invoice",
        ),
        dep_in_example(
            "app::sales::orders::domain::order",
            "app::billing::invoices::application::command::issue_invoice",
        ),
        dep_in_example(
            "app::sales::orders::infrastructure::adapter::billing_client",
            "app::billing::invoices::domain::invoice",
        ),
        dep_in_example(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::domain::order",
        ),
    ]);

    let violations = Rule::between("app::*")
        .named("bounded contexts")
        .deny_all()
        .allow(
            Access::from("*::infrastructure::adapter::**")
                .to_any([
                    "*::application::command::**",
                    "*::application::query::**",
                ])
                .because("bounded contexts may communicate only through adapters calling command/query APIs"),
        )
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 2);
    assert!(violations
        .iter()
        .all(|violation| violation.rule_name() == "bounded contexts"));
}

#[test]
fn within_rule_checks_dependencies_inside_the_same_scope() {
    let graph = DependencyGraph::from_dependencies([
        dep_in_example(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::domain::order",
        ),
        dep_in_example(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::infrastructure::repository::sql_order_repository",
        ),
        dep_in_example(
            "app::sales::orders::ui::http_controller",
            "app::sales::orders::application::command::create_order",
        ),
        dep_in_example(
            "app::billing::invoices::application::command::issue_invoice",
            "app::sales::orders::domain::order",
        ),
    ]);

    let violations = Rule::within("app::*::*")
        .named("module internals")
        .deny_all()
        .allow(Access::from("application::**").to("domain::**"))
        .allow(Access::from("infrastructure::**").to("application::**"))
        .allow(Access::from("infrastructure::**").to("domain::**"))
        .allow(Access::from("ui::**").to("application::**"))
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 1);
    assert!(violations.to_string().contains("module internals"));
    assert!(violations.to_string().contains("infrastructure"));
}

#[test]
fn rule_can_ignore_dependencies_from_matching_files() {
    let graph = DependencyGraph::from_dependencies([
        dep(
            "app::sales::orders",
            "app::sales::orders::infrastructure::repository::sql_order_repository",
            "src/app/sales/orders/mod.rs",
        ),
        dep(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::infrastructure::repository::sql_order_repository",
            "src/app/sales/orders/application/command/create_order.rs",
        ),
    ]);

    let violations = Rule::within("app::*::*")
        .named("module internals")
        .deny_all()
        .ignore_files(COMPOSITION_ROOT_GLOBS)
        .allow(Access::from("application::**").to("domain::**"))
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 1);
    assert!(violations
        .to_string()
        .contains("application/command/create_order.rs"));
    assert!(!violations.to_string().contains("mod.rs"));
}

#[test]
fn ignored_file_patterns_are_rule_specific() {
    let graph = DependencyGraph::from_dependencies([dep(
        "app::sales::orders",
        "app::sales::orders::infrastructure::repository::sql_order_repository",
        "src/app/sales/orders/mod.rs",
    )]);

    let violations = Rule::within("app::*::*")
        .named("module internals")
        .deny_all()
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 1);
    assert!(violations.to_string().contains("mod.rs"));
}

#[test]
fn invalid_ignored_file_globs_are_returned_as_errors() {
    let graph = DependencyGraph::new();

    let error = Rule::within("app::*::*")
        .ignore_files(["["])
        .check(&graph)
        .unwrap_err();

    assert!(error.to_string().contains("invalid pattern"));
    assert!(error.to_string().contains('['));
}

#[test]
fn global_rule_can_deny_absolute_dependency_patterns() {
    let graph = DependencyGraph::from_dependencies([
        dep_in_example(
            "app::sales::orders::domain::order",
            "app::sales::orders::infrastructure::repository::sql_order_repository",
        ),
        dep_in_example(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::domain::order",
        ),
    ]);

    let violations = Rule::new()
        .named("domain purity")
        .deny(
            Access::from("app::**::domain::**")
                .to("app::**::infrastructure::**")
                .because("domain code must not depend on infrastructure"),
        )
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 1);
    assert!(violations
        .to_string()
        .contains("domain code must not depend on infrastructure"));
}

#[test]
fn explicit_deny_can_except_target_patterns() {
    let graph = DependencyGraph::from_dependencies([
        dep_in_example(
            "modules::portfolio::trade::application::command::create_trade",
            "sea_orm::DatabaseConnection",
        ),
        dep_in_example(
            "modules::portfolio::trade::application::command::create_trade",
            "sea_orm::ConnectionTrait",
        ),
    ]);

    let violations = Rule::new()
        .deny(
            Access::from("modules::**::application::**")
                .to("sea_orm::**")
                .except_to(["sea_orm::DatabaseConnection"]),
        )
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 1);

    let formatted = violations.to_string();
    assert!(formatted.contains("ConnectionTrait"));
    assert!(!formatted.contains("DatabaseConnection"));
}

#[test]
fn allow_access_can_except_target_patterns_under_deny_all() {
    let graph = DependencyGraph::from_dependencies([
        dep_in_example(
            "infrastructure::repository::sql_order_repository",
            "application::port::order_repository",
        ),
        dep_in_example(
            "infrastructure::repository::sql_order_repository",
            "application::service::create_order",
        ),
    ]);

    let violations = Rule::new()
        .deny_all()
        .allow(
            Access::from("infrastructure::**")
                .to("application::**")
                .except_to(["application::service::**"]),
        )
        .check(&graph)
        .unwrap();

    assert_eq!(violations.len(), 1);
    assert!(violations.to_string().contains("application::service"));
}

#[test]
fn invalid_patterns_are_returned_as_errors() {
    let graph = DependencyGraph::new();

    let error = Rule::between("app::***")
        .deny_all()
        .check(&graph)
        .unwrap_err();

    assert!(error.to_string().contains("invalid pattern"));
}
