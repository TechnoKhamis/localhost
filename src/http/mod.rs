mod request;
mod response;

pub use request::HttpRequest;
pub use response::HttpResponse;

#[derive(Debug, Clone)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
}
