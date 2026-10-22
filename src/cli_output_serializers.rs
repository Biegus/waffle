use crate::objects::{App, Endpoint, Json, JsonMap};
use serde_json;

pub fn simple_serialize_endpoint(endpoint: &Endpoint) -> String {
    return format!("{}: {:?} {}", endpoint.name, endpoint.kind, endpoint.url);
}
