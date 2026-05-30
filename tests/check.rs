use archaven::{Access, Archaven, ArchavenError, DependencyGraph, Rule, RuleSet, Violations};
use std::fs;

#[test]
fn check_scans_rust_files_and_returns_printable_violations() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/sales/orders/domain")).unwrap();
    fs::create_dir_all(src.join("app/sales/orders/infrastructure/adapter")).unwrap();

    fs::write(
        src.join("app/sales/orders/domain/order.rs"),
        r"
            use crate::app::billing::invoices::application::command::issue_invoice::IssueInvoice;

            pub fn place_order(command: IssueInvoice) {
                let _ = command;
            }
        ",
    )
    .unwrap();

    fs::write(
        src.join("app/sales/orders/infrastructure/adapter/billing_client.rs"),
        r"
            pub fn call_billing() {
                crate::app::billing::invoices::application::query::invoice_status::get_status();
            }
        ",
    )
    .unwrap();

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
                        .because("bounded contexts may communicate only through adapters calling command/query APIs"),
                ),
        )
        .check(&src)
        .unwrap();

    assert_eq!(violations.len(), 1);

    let formatted = violations.to_string();
    assert!(formatted.contains("bounded contexts"));
    assert!(formatted.contains("src/app/sales/orders/domain/order.rs"));
    assert!(formatted.contains("adapters calling command/query APIs"));
}

#[test]
fn check_can_ignore_module_composition_roots_by_file_glob() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/sales/orders/application/command")).unwrap();
    fs::create_dir_all(src.join("app/sales/orders/infrastructure/repository")).unwrap();

    fs::write(
        src.join("app/sales/orders/mod.rs"),
        r"
            mod application;
            mod infrastructure;

            pub fn wire_module() {
                crate::app::sales::orders::infrastructure::repository::sql_order_repository::new();
            }
        ",
    )
    .unwrap();

    fs::write(
        src.join("app/sales/orders/application/command/create_order.rs"),
        r"
            pub fn create_order() {
                crate::app::sales::orders::infrastructure::repository::sql_order_repository::new();
            }
        ",
    )
    .unwrap();

    fs::write(
        src.join("app/sales/orders/infrastructure/repository/sql_order_repository.rs"),
        r"
            pub fn new() {}
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .rule(
            Rule::within("app::*::*")
                .named("module internals")
                .deny_all()
                .ignore_files(["**/mod.rs"])
                .allow(Access::from("application::**").to("domain::**")),
        )
        .check(&src)
        .unwrap();

    assert_eq!(violations.len(), 1);

    let formatted = violations.to_string();
    assert!(formatted.contains("application/command/create_order.rs"));
    assert!(!formatted.contains("app/sales/orders/mod.rs"));
}

#[test]
fn check_can_allow_only_module_roots_in_matching_directories() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/sales/orders/application")).unwrap();
    fs::create_dir_all(src.join("app/sales/orders/domain")).unwrap();
    fs::create_dir_all(src.join("app/billing/invoices")).unwrap();

    fs::write(src.join("app/sales/orders/mod.rs"), "").unwrap();
    fs::write(src.join("app/sales/orders/application.rs"), "").unwrap();
    fs::write(src.join("app/sales/orders/domain.rs"), "").unwrap();
    fs::write(src.join("app/sales/orders/helper.rs"), "").unwrap();
    fs::write(src.join("app/billing/invoices.rs"), "").unwrap();

    let violations = Archaven::new()
        .rule(
            Rule::directories("app::*::*")
                .named("module root files")
                .allow_only_module_roots(),
        )
        .check(&src)
        .unwrap();

    assert_eq!(violations.len(), 1);

    let formatted = violations.to_string();
    assert!(formatted.contains("module root files"));
    assert!(formatted.contains("app/sales/orders/helper.rs"));
    assert!(formatted.contains("only module root files are allowed"));
}

#[test]
fn directory_rule_rejects_patterns_without_single_wildcards() {
    let graph = archaven::DependencyGraph::new();

    let violations = Rule::directories("app::*::*")
        .allow_only_module_roots()
        .check(&graph)
        .unwrap();

    assert!(violations.is_empty());

    let error = Rule::directories("app::**")
        .allow_only_module_roots()
        .check(&graph)
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("directory rules do not support `**`"));

    let error = Rule::directories("app::sales")
        .allow_only_module_roots()
        .check(&graph)
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("directory rules support at least one `*` segment"));
}

#[test]
fn check_can_ignore_module_roots_in_dependency_rules() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/sales/orders/application/command")).unwrap();
    fs::create_dir_all(src.join("app/sales/orders/infrastructure/repository")).unwrap();
    fs::create_dir_all(src.join("app/sales/orders/infrastructure")).unwrap();

    fs::write(
        src.join("app/sales/orders/mod.rs"),
        r"
            pub fn wire_mod() {
                crate::app::sales::orders::infrastructure::repository::sql_order_repository::new();
            }
        ",
    )
    .unwrap();

    fs::write(
        src.join("app/sales/orders/infrastructure.rs"),
        r"
            pub fn wire_new_style_root() {
                crate::app::sales::orders::infrastructure::repository::sql_order_repository::new();
            }
        ",
    )
    .unwrap();

    fs::write(
        src.join("app/sales/orders/application/command/create_order.rs"),
        r"
            pub fn create_order() {
                crate::app::sales::orders::infrastructure::repository::sql_order_repository::new();
            }
        ",
    )
    .unwrap();

    fs::write(
        src.join("app/sales/orders/infrastructure/repository/sql_order_repository.rs"),
        r"
            pub fn new() {}
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .rule(
            Rule::within("app::*::*")
                .named("module internals")
                .deny_all()
                .ignore_module_roots()
                .allow(Access::from("application::**").to("domain::**")),
        )
        .check(&src)
        .unwrap();

    assert_eq!(violations.len(), 1);

    let formatted = violations.to_string();
    assert!(formatted.contains("application/command/create_order.rs"));
    assert!(!formatted.contains("app/sales/orders/mod.rs"));
    assert!(!formatted.contains("app/sales/orders/infrastructure.rs"));
}

#[test]
fn custom_rule_sets_can_read_discovered_directories() {
    struct NoLooseHelperFiles;

    impl RuleSet for NoLooseHelperFiles {
        fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
            let has_helper = graph.directories().iter().any(|directory| {
                directory.module().to_string() == "app::orders"
                    && directory.files().iter().any(|file| file == "helper.rs")
                    && directory
                        .child_directories()
                        .iter()
                        .any(|directory| directory == "domain")
            });

            assert!(has_helper);
            Ok(Violations::new())
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/domain")).unwrap();
    fs::write(src.join("app/orders/helper.rs"), "").unwrap();

    let violations = Archaven::new()
        .rule(NoLooseHelperFiles)
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}
