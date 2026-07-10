use reqwest::blocking::RequestBuilder;

/// Sends a prepared request and extracts the reply text. Shared by the HTTP
/// agent backends; any transport, status or parse failure is a failed call.
pub(super) fn send_and_extract(
    request: RequestBuilder,
    extract: fn(&str) -> Option<String>,
) -> (bool, String) {
    let response = match request.send() {
        Ok(response) => response,
        Err(error) => return (false, format!("request failed: {error}")),
    };

    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return (false, format!("HTTP {status}: {}", body.trim()));
    }
    match extract(&body) {
        Some(content) => (true, content),
        None => (false, format!("unexpected response: {}", body.trim())),
    }
}
