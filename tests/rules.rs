use archaven::{Access, Dependency, DependencyGraph, Location, ModulePath, Rule};

fn dep(source: &str, target: &str) -> Dependency {
    Dependency::new(
        ModulePath::parse(source).unwrap(),
        ModulePath::parse(target).unwrap(),
        Location::new("src/example.rs"),
    )
}

#[test]
fn between_rule_allows_only_configured_cross_scope_access() {
    let graph = DependencyGraph::from_dependencies([
        dep(
            "app::sales::orders::infrastructure::adapter::billing_client",
            "app::billing::invoices::application::command::issue_invoice",
        ),
        dep(
            "app::sales::orders::domain::order",
            "app::billing::invoices::application::command::issue_invoice",
        ),
        dep(
            "app::sales::orders::infrastructure::adapter::billing_client",
            "app::billing::invoices::domain::invoice",
        ),
        dep(
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
        dep(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::domain::order",
        ),
        dep(
            "app::sales::orders::application::command::create_order",
            "app::sales::orders::infrastructure::repository::sql_order_repository",
        ),
        dep(
            "app::sales::orders::ui::http_controller",
            "app::sales::orders::application::command::create_order",
        ),
        dep(
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
fn global_rule_can_deny_absolute_dependency_patterns() {
    let graph = DependencyGraph::from_dependencies([
        dep(
            "app::sales::orders::domain::order",
            "app::sales::orders::infrastructure::repository::sql_order_repository",
        ),
        dep(
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
fn invalid_patterns_are_returned_as_errors() {
    let graph = DependencyGraph::new();

    let error = Rule::between("app::***")
        .deny_all()
        .check(&graph)
        .unwrap_err();

    assert!(error.to_string().contains("invalid pattern"));
}
