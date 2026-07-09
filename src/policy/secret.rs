use std::path::Path;

const SECRET_MARKERS: &[&str] = &[
    "id_rsa",
    "id_ed25519",
    ".ssh/",
    ".ssh\\",
    ".aws/credentials",
    ".pem",
    "credentials.json",
];

pub(super) fn secret_reason(path: &Path) -> Option<String> {
    if is_dotenv(path) {
        return Some("environment file (.env)".to_string());
    }
    let text = path.to_string_lossy();
    for &marker in SECRET_MARKERS {
        if text.contains(marker) {
            return Some(format!("secret path ({marker})"));
        }
    }
    None
}

fn is_dotenv(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name == ".env" || name.starts_with(".env.")
}
