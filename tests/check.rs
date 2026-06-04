use archaven::{Access, Archaven, ArchavenError, DependencyGraph, Rule, RuleSet, Violations};
use std::fs;

struct ExpectTargets(&'static [&'static str]);
struct RejectTargets(&'static [&'static str]);

impl RuleSet for ExpectTargets {
    fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
        let targets = graph
            .dependencies()
            .iter()
            .map(|dependency| dependency.target().to_string())
            .collect::<Vec<_>>();

        for expected in self.0 {
            assert!(
                targets.iter().any(|target| target == expected),
                "expected target {expected} in {targets:?}"
            );
        }

        Ok(Violations::new())
    }
}

impl RuleSet for RejectTargets {
    fn check(&self, graph: &DependencyGraph) -> Result<Violations, ArchavenError> {
        let targets = graph
            .dependencies()
            .iter()
            .map(|dependency| dependency.target().to_string())
            .collect::<Vec<_>>();

        for rejected in self.0 {
            assert!(
                !targets.iter().any(|target| target == rejected),
                "rejected target {rejected} found in {targets:?}"
            );
        }

        Ok(Violations::new())
    }
}

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
fn check_records_external_dependencies_when_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("modules/portfolio/trade/application/command")).unwrap();
    fs::write(
        src.join("modules/portfolio/trade/application/command/create_trade.rs"),
        r"
            use axum::Json;
            use sea_orm::{ConnectionTrait, DatabaseConnection};

            #[derive(sea_orm::FromQueryResult)]
            struct TradeRow {
                id: i32,
            }

            fn uses_path<C: sea_orm::ConnectionTrait>() {}

            fn uses_associated_path() {
                sea_orm::Statement::from_sql_and_values;
            }
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&[
            "axum::Json",
            "sea_orm::ConnectionTrait",
            "sea_orm::DatabaseConnection",
            "sea_orm::FromQueryResult",
            "sea_orm::Statement::from_sql_and_values",
        ]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_can_apply_rules_to_external_dependencies_without_configuring_roots() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("modules/portfolio/trade/application/command")).unwrap();
    fs::write(
        src.join("modules/portfolio/trade/application/command/create_trade.rs"),
        r"
            use sea_orm::{ConnectionTrait, DatabaseConnection};

            fn create_trade<C: ConnectionTrait>(connection: DatabaseConnection, _client: C) {
                let _ = connection;
            }
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(
            Rule::new().deny(
                Access::from("modules::**::application::**")
                    .to("sea_orm::**")
                    .except_to(["sea_orm::DatabaseConnection"]),
            ),
        )
        .check(&src)
        .unwrap();

    assert_eq!(violations.len(), 1);
    assert!(violations.to_string().contains("sea_orm::ConnectionTrait"));
    assert!(!violations
        .to_string()
        .contains("sea_orm::DatabaseConnection"));
}

#[test]
fn check_records_grouped_and_renamed_external_use_items() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use sea_orm::{ConnectionTrait as SeaConnection, TransactionTrait};

            fn use_imports<T: SeaConnection, U: TransactionTrait>() {}
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&[
            "sea_orm::ConnectionTrait",
            "sea_orm::TransactionTrait",
        ]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_ignores_external_dependencies_unless_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use sea_orm::ConnectionTrait;
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .rule(RejectTargets(&["sea_orm::ConnectionTrait"]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_does_not_record_standard_library_roots_as_external_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use alloc::borrow::Cow;
            use core::fmt::Debug;
            use proc_macro::TokenStream;
            use std::sync::Arc;
            use sea_orm::ConnectionTrait;

            fn use_imports<T: Debug>(
                arc: Arc<T>,
                cow: Cow<'static, str>,
                stream: TokenStream,
                connection: impl ConnectionTrait,
            ) {
                let _ = (arc, cow, stream, connection);
            }
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&["sea_orm::ConnectionTrait"]))
        .rule(RejectTargets(&[
            "alloc::borrow::Cow",
            "core::fmt::Debug",
            "proc_macro::TokenStream",
            "std::sync::Arc",
        ]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_rewrites_simple_external_root_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use sea_orm as orm;

            fn use_alias<C: orm::ConnectionTrait>() {}
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&["sea_orm::ConnectionTrait"]))
        .rule(RejectTargets(&["orm::ConnectionTrait"]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_rewrites_simple_external_item_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use sea_orm::DatabaseConnection as Db;

            fn use_alias(connection: Db) {
                let _ = connection;
            }
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&["sea_orm::DatabaseConnection"]))
        .rule(RejectTargets(&["Db"]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_rewrites_grouped_self_and_item_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use sea_orm::{self as orm, DatabaseConnection as Db, TransactionTrait};

            fn use_aliases<C: orm::ConnectionTrait, T: TransactionTrait>(connection: Db) {
                let _ = connection;
            }
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&[
            "sea_orm::ConnectionTrait",
            "sea_orm::DatabaseConnection",
            "sea_orm::TransactionTrait",
        ]))
        .rule(RejectTargets(&[
            "orm::ConnectionTrait",
            "Db",
            "TransactionTrait",
        ]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_rewrites_simple_local_aliases_without_marking_them_external() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::create_dir_all(src.join("app/orders/service")).unwrap();
    fs::write(
        src.join("app/orders/application/command.rs"),
        r"
            use crate::app as local_app;

            fn use_alias() {
                local_app::orders::service::run();
            }
        ",
    )
    .unwrap();
    fs::write(src.join("app/orders/service/mod.rs"), "").unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&["app::orders::service::run"]))
        .rule(RejectTargets(&["local_app::orders::service::run"]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
}

#[test]
fn check_does_not_treat_single_segment_paths_as_external_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("src");

    fs::create_dir_all(src.join("app/orders/application")).unwrap();
    fs::write(
        src.join("app/orders/application/service.rs"),
        r"
            use sea_orm::DatabaseConnection;

            struct LocalType;

            fn use_import(connection: DatabaseConnection, local: LocalType) {
                let _ = (connection, local);
            }
        ",
    )
    .unwrap();

    let violations = Archaven::new()
        .include_external_dependencies()
        .rule(ExpectTargets(&["sea_orm::DatabaseConnection"]))
        .rule(RejectTargets(&["DatabaseConnection", "LocalType"]))
        .check(&src)
        .unwrap();

    assert!(violations.is_empty());
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
