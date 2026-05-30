fn main() {}

#[cfg(test)]
mod tests {
    use archaven::{Access, Archaven, Rule};

    #[test]
    fn domain_does_not_depend_on_infrastructure() {
        let violations = Archaven::new()
            .rule(
                Rule::new().named("domain purity").deny(
                    Access::from("app::**::domain::**")
                        .to("app::**::infrastructure::**")
                        .because("domain code must not depend on infrastructure"),
                ),
            )
            .check("examples/sample_app/src")
            .unwrap();

        violations.assert_empty();
    }

    #[test]
    fn feature_dependencies_follow_the_expected_direction() {
        let violations = Archaven::new()
            .rule(
                Rule::within("app::*")
                    .named("feature internals")
                    .deny_all()
                    .allow(Access::from("http::**").to("application::**"))
                    .allow(Access::from("application::**").to("domain::**"))
                    .allow(Access::from("infrastructure::**").to("application::**"))
                    .allow(Access::from("infrastructure::**").to("domain::**"))
                    .because(
                        "feature dependencies should point through application and domain code",
                    ),
            )
            .check("examples/sample_app/src")
            .unwrap();

        violations.assert_empty();
    }
}
