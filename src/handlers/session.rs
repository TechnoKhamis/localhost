pub fn create_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    
    format!("SID_{}", timestamp)
}

pub fn get_session_id(cookie_header: Option<&str>) -> Option<String> {
    cookie_header?
        .split(';')
        .find(|c| c.trim().starts_with("SID="))?
        .split('=')
        .nth(1)
        .map(|s| s.trim().to_string())
}