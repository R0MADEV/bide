use reqwest::blocking::RequestBuilder;

/// Sends a prepared request and extracts the reply text. Shared by the HTTP
/// agent backends; any transport, status or parse failure is a failed call.
pub(super) fn send_and_extract(
    request: RequestBuilder,
    extract: fn(&str) -> Option<String>,
) -> (bool, String) {
    let Ok(response) = request.send() else {
        return (false, String::new());
    };
    if !response.status().is_success() {
        return (false, String::new());
    }
    let Ok(text) = response.text() else {
        return (false, String::new());
    };
    match extract(&text) {
        Some(content) => (true, content),
        None => (false, String::new()),
    }
}
