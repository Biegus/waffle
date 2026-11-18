use crate::objects::{App, Endpoint, Json, JsonMap, Parameter};
use itertools::Itertools;
use serde_json;

pub fn simple_serialize_endpoint(endpoint: &Endpoint) -> String {
    return format!("{}: {:?} {}", endpoint.name, endpoint.kind, endpoint.url);
}
pub fn _serializer_required(v: Option<bool>) -> &'static str {
    return match v {
        None => "?",
        Some(true) => "!",
        Some(false) => "o",
    };
}
pub fn serialize_parameter(parameter: &Parameter) -> String {
    return format!(
        "{} {}:{} <- {}",
        _serializer_required(parameter.required),
        parameter.name,
        parameter.param_type.as_deref().unwrap_or("?"),
        parameter.location
    );
}
pub fn serialize_endpoint_long(endpoint: &Endpoint) -> String {
    let schema = endpoint.schema.clone();
    let serialized_parameters = schema.parameters.map(|t| {
        t.iter()
            .sorted_by_key(|t| &t.location)
            .rev()
            .map(serialize_parameter)
            .join("\n")
    });
    let serialized_consumes = schema.consumes.map(|t| t.iter().join("\n\n"));
    let serialized_parameters = match serialized_parameters {
        Some(v) => {
            if v.is_empty() {
                "\n\n0 parameters\n".to_string()
            } else {
                format!("\n\nparameters:\n{v}")
            }
        }
        None => "".to_string(),
    };
    let serialized_consumes = match serialized_consumes {
        None => "".to_string(),
        Some(v) => {
            if v.len() == 0 {
                "".to_string()
            } else {
                format!("\n\nconsumes:{v}")
            }
        }
    };
    return format!(
        "{}:\n{:?} schema:{}{}{}",
        endpoint.name, endpoint.kind, endpoint.url, serialized_parameters, serialized_consumes
    );
}
