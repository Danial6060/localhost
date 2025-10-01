use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SessionManager {
    sessions: HashMap<String, SessionData>,
}

#[derive(Clone)]
pub struct SessionData {
    pub id: String,
    pub data: HashMap<String, String>,
    pub created_at: u64,
    pub last_accessed: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: HashMap::new(),
        }
    }

    pub fn create_session(&mut self) -> String {
        let session_id = self.generate_session_id();
        let now = Self::current_timestamp();

        let session = SessionData {
            id: session_id.clone(),
            data: HashMap::new(),
            created_at: now,
            last_accessed: now,
        };

        self.sessions.insert(session_id.clone(), session);
        session_id
    }

    pub fn get_session(&mut self, session_id: &str) -> Option<&mut SessionData> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.last_accessed = Self::current_timestamp();
            Some(session)
        } else {
            None
        }
    }

    pub fn destroy_session(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    pub fn cleanup_expired(&mut self, max_age_seconds: u64) {
        let now = Self::current_timestamp();
        self.sessions.retain(|_, session| {
            now - session.last_accessed < max_age_seconds
        });
    }

    fn generate_session_id(&self) -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let s = RandomState::new();
        let mut hasher = s.build_hasher();
        
        Self::current_timestamp().hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        self.sessions.len().hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

pub fn parse_cookies(cookie_header: &str) -> HashMap<String, String> {
    let mut cookies = HashMap::new();
    
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(eq_pos) = cookie.find('=') {
            let key = cookie[..eq_pos].trim().to_string();
            let value = cookie[eq_pos + 1..].trim().to_string();
            cookies.insert(key, value);
        }
    }
    
    cookies
}

pub fn create_set_cookie(name: &str, value: &str, max_age: Option<u64>) -> String {
    let mut cookie = format!("{}={}; Path=/; HttpOnly", name, value);
    
    if let Some(age) = max_age {
        cookie.push_str(&format!("; Max-Age={}", age));
    }
    
    cookie
}