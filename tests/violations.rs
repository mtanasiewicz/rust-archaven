use std::{
    panic,
    sync::{Arc, Mutex},
};

use archaven::{Access, Dependency, DependencyGraph, Location, ModulePath, Rule};

fn dep(source: &str, target: &str) -> Dependency {
    Dependency::new(
        ModulePath::parse(source).unwrap(),
        ModulePath::parse(target).unwrap(),
        Location::new("src/example.rs"),
    )
}

#[test]
fn assert_empty_reports_the_test_call_site_when_panicking() {
    let graph = DependencyGraph::from_dependencies([dep(
        "app::sales::orders::domain::order",
        "app::sales::orders::infrastructure::repository::sql_order_repository",
    )]);
    let violations = Rule::new()
        .deny(Access::from("app::**::domain::**").to("app::**::infrastructure::**"))
        .check(&graph)
        .unwrap();

    let panic_file = Arc::new(Mutex::new(None::<String>));
    let panic_file_for_hook = Arc::clone(&panic_file);
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if let Some(location) = info.location() {
            *panic_file_for_hook.lock().unwrap() = Some(location.file().replace('\\', "/"));
        }
    }));

    let result = panic::catch_unwind(|| violations.assert_empty());
    panic::set_hook(previous_hook);

    assert!(result.is_err());
    let file = panic_file.lock().unwrap().clone().unwrap();
    assert!(
        file.ends_with("tests/violations.rs"),
        "panic location should be the test call site, got {file}",
    );
}
