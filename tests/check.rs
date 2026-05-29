use archaven::{Access, Archaven, Rule};
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
