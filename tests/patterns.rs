use archaven::{ModulePath, PathPattern};

#[test]
fn star_matches_exactly_one_segment() {
    let pattern = PathPattern::parse("app::*::anything").unwrap();

    assert!(pattern.matches(&ModulePath::parse("app::orders::anything").unwrap()));
    assert!(!pattern.matches(&ModulePath::parse("app::anything").unwrap()));
    assert!(!pattern.matches(&ModulePath::parse("app::orders::nested::anything").unwrap()));
}

#[test]
fn double_star_matches_one_or_more_segments() {
    let pattern = PathPattern::parse("app::**::anything").unwrap();

    assert!(pattern.matches(&ModulePath::parse("app::orders::anything").unwrap()));
    assert!(pattern.matches(&ModulePath::parse("app::orders::nested::anything").unwrap()));
    assert!(!pattern.matches(&ModulePath::parse("app::anything").unwrap()));
}

#[test]
fn trailing_double_star_matches_everything_below_prefix() {
    let pattern = PathPattern::parse("app::**").unwrap();

    assert!(pattern.matches(&ModulePath::parse("app::orders").unwrap()));
    assert!(pattern.matches(&ModulePath::parse("app::orders::domain").unwrap()));
    assert!(!pattern.matches(&ModulePath::parse("app").unwrap()));
}

#[test]
fn prefix_match_returns_boundary_and_remainder() {
    let pattern = PathPattern::parse("app::*").unwrap();
    let path = ModulePath::parse("app::sales::orders::domain::order").unwrap();

    let matched = pattern.match_prefix(&path).unwrap();

    assert_eq!(matched.matched().to_string(), "app::sales");
    assert_eq!(matched.remainder().to_string(), "orders::domain::order");
}
