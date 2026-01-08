/// This holds the server configuration
#[derive(Debug)]
pub struct ServerConfig {
    pub listen_addresses: Vec<String>,
    pub client_body_size_limit: usize,
    pub routes: Vec<RouteConfig>,  //List of routes
    pub error_path:String,
    pub vhosts: Vec<VHost>,
}

#[derive(Debug)]
pub struct VHost {
    pub name: String,
    pub error_path: String,
    pub routes: Vec<RouteConfig>,
}

/// Configuration for ONE route
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// The URL path (e.g., "/upload", "/")
    pub path: String,
    
    /// Allowed HTTP methods (e.g., ["GET", "POST"])
    pub methods: Vec<String>,
    
    /// Where to serve files from (e.g., "www", "uploads")
    pub root: String,

    //default file to serve
    pub default_file:Option<String>,

     pub autoindex: bool,

    pub cgi: Option<String>,

     pub redirect: Option<String>,
}