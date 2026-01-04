#[cfg(test)]
mod tests {
    use localhost::config::*;

    #[test]
    fn test_parse_simple_config() {
        let config_str = r#"
            listen = 127.0.0.1:8080
            client_max_body_size = 5000000
            
            route / {
                methods = GET,POST
                root = www
            }
        "#;

        let config = parse_config_string(config_str).unwrap();
        
        assert_eq!(config.bind_addresses, "127.0.0.1:8080");
        assert_eq!(config.max_body_size, 5000000);
        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.routes[0].path, "/");
        assert_eq!(config.routes[0].allowed_methods, vec!["GET", "POST"]);
    }

    #[test]
    fn test_parse_with_vhost() {
        let config_str = r#"
            listen = 127.0.0.1:8080
            
            vhost example.com {
                route / {
                    methods = GET
                    root = www_example
                }
            }
        "#;

        let config = parse_config_string(config_str).unwrap();
        
        assert!(config.virtual_hosts.contains_key("example.com"));
        let vhost_routes = &config.virtual_hosts["example.com"];
        assert_eq!(vhost_routes.len(), 1);
        assert_eq!(vhost_routes[0].path, "/");
    }

    #[test]
    fn test_parse_cgi_extensions() {
        let config_str = r#"
            listen = 127.0.0.1:8080
            cgi_ext = .py=cgi-bin,.php=php-cgi
        "#;

        let config = parse_config_string(config_str).unwrap();
        
        assert_eq!(config.cgi_extensions.get(".py"), Some(&"cgi-bin".to_string()));
        assert_eq!(config.cgi_extensions.get(".php"), Some(&"php-cgi".to_string()));
    }

    #[test]
    fn test_invalid_config() {
        let config_str = "invalid syntax here";
        assert!(parse_config_string(config_str).is_err());
    }
}