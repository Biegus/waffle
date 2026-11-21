use std::collections::HashMap;
use std::iter::repeat;

use crate::objects::{App, Endpoint, EndpointSchema, Json, JsonMap, Parameter};
use crate::objects::{ParameterKind, ResponseSchema};
use itertools::Itertools;
use serde_json;

pub fn simple_serialize_endpoint(endpoint: &Endpoint) -> String {
    return format!("{}: {:?} {}", endpoint.name, endpoint.kind, endpoint.url);
}
pub fn _serializer_required(v: bool) -> &'static str {
    return match v {
        true => "!",
        false => "o",
    };
}
pub fn serialize_parameter(parameter: &Parameter) -> String {
    return format!(
        "{} {}:{}",
        _serializer_required(parameter.required),
        parameter.name,
        parameter.param_type.as_deref().unwrap_or("?"),
    );
}
pub fn serialize_parameters(params: &HashMap<ParameterKind, Vec<Parameter>>) -> String {
    return params
        .iter()
        .filter(|(kind, _)| **kind != ParameterKind::Path)
        .map(|(kind, parameters)| {
            format!(
                "{kind}:\n:{}",
                parameters.iter().map(serialize_parameter).join("\n")
            )
        })
        .join("\n");
}
pub fn serialize_response(response: &ResponseSchema) -> String {
    return format!(
        "{}: {}{}",
        response.status_code,
        response.description.as_ref().unwrap_or(&String::default()),
        match &response.schema_example {
            Some(v) => format!(
                "\n{}",
                serde_json::to_string_pretty(v).unwrap_or("Json Parser Error".to_string())
            ),
            None => String::default(),
        }
    );
}

pub fn serialize_schema(schema: &EndpointSchema) -> String {
    return format!(
        "{}{}{}",
        match &schema.params.len() {
            0 => String::default(),
            _ => format!("\n{}", serialize_parameters(&schema.params)),
        },
        match &schema.request_schema {
            Some(v) => format!(
                "\n{}",
                &serde_json::to_string_pretty(&v.schema_example)
                    .unwrap_or("Json Parsing Error".to_string()),
            ),
            None => {
                String::default()
            }
        },
        match &schema.responses_schema.len() {
            0 => String::default(),
            _ => format!(
                "\n{}",
                &schema
                    .responses_schema
                    .iter()
                    .map(serialize_response)
                    .join("\n"),
            ),
        }
    );
}

pub fn serialize_endpoint_long(endpoint: &Endpoint) -> String {
    return format!(
        "{} -> {:?} {}{}",
        endpoint.name,
        endpoint.kind,
        endpoint.url,
        match &endpoint.schema {
            Some(s) => format!("{}", serialize_schema(s)),
            None => String::default(),
        },
    );
}
