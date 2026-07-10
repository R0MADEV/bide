use std::path::Path;

/// Marker file → the command that runs that project's tests. Used to give the
/// default recipe a real verify step when there is no bide.toml.
const PROJECT_TESTS: &[(&str, &str)] = &[
    ("Cargo.toml", "cargo test"),
    ("package.json", "npm test"),
    ("go.mod", "go test ./..."),
    ("pyproject.toml", "pytest"),
    ("Gemfile", "bundle exec rake test"),
];

pub fn verify_command(dir: &Path) -> Option<String> {
    PROJECT_TESTS
        .iter()
        .find(|(marker, _)| dir.join(marker).exists())
        .map(|(_, command)| command.to_string())
}
