const DENIED: &[&str] = &[
    "rm -rf",
    "git reset --hard",
    "git clean -f",
    "mkfs",
    "dd if=",
    ":(){",
    "> /dev/sd",
];

const NEEDS_APPROVAL: &[&str] = &["rm ", "git push --force", "truncate "];

pub(super) fn denied_reason(command: &str) -> Option<&'static str> {
    DENIED.iter().find(|pattern| command.contains(**pattern)).copied()
}

pub(super) fn approval_reason(command: &str) -> Option<&'static str> {
    NEEDS_APPROVAL
        .iter()
        .find(|pattern| command.contains(**pattern))
        .copied()
}
