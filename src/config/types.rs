/// Server configuration container
#[derive(Debug)]
pub struct ServerConfig {
    pub listen_addresses: Vec<String>,
    pub client_body_size_limit: usize,
    pub routes: Vec<RouteConfig>,
    pub error_path: String,
    pub vhosts: Vec<VHost>,
}

/// Virtual host configuration
#[derive(Debug)]
pub struct VHost {
    pub name: String,
    pub error_path: String,
    pub routes: Vec<RouteConfig>,
}

/// Route configuration
#[derive(Debug, Clone)]
pub struct RouteConfig {
    pub path: String,
    pub methods: Vec<String>,
    pub root: String,
    pub default_file: Option<String>,
    pub autoindex: bool,
    pub cgi: Option<String>,
    pub redirect: Option<String>,
}
