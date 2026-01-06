pub fn create_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    
    format!("SID_{}", timestamp)
}

pub fn get_session_id(cookie_header: Option<&str>) -> Option<String> {
    let header = cookie_header?;
    
    for part in header.split(';') {
        let part = part.trim();
        if part.starts_with("SID") { 
            if let Some(value) = part.split('=').nth(1) {
                return Some(value.trim().to_string());
            }
        }
    }
    
    None
}