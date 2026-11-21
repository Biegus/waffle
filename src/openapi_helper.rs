use std::collections::HashMap;

use itertools::Itertools;
use map_tuple::TupleMap1;
use openapiv3;
use serde_json::json;

use crate::objects::{
    EndpointSchema, Json, JsonMap, Parameter, ParameterKind, PayloadSchema, ResponseSchema,
};

fn _build_params_schema_map(
    spec: &openapiv3::OpenAPI,
    op: &openapiv3::Operation,
) -> HashMap<ParameterKind, Vec<Parameter>> {
    use crate::objects::ParameterKind::*;
    return op
        .parameters
        .iter()
        .filter_map(|t| t.resolve(&spec).ok())
        .chunk_by(|param| match param.kind.clone() {
            openapiv3::ParameterKind::Query {
                allow_reserved: _,
                style: _,
                allow_empty_value: _,
            } => Query,
            openapiv3::ParameterKind::Header { style: _ } => Header,
            openapiv3::ParameterKind::Path { style: _ } => Path,
            openapiv3::ParameterKind::Cookie { style: _ } => Cookie,
        })
        .into_iter()
        .map(|c| {
            c.map1(|v| {
                v.map(|param| Parameter::new(param.name.clone(), param.required, None))
                    .collect()
            })
        })
        .collect();
}

fn _construct_example_from_shema(
    spec: &openapiv3::OpenAPI,
    schema: &openapiv3::Schema,
) -> Option<Json> {
    let properties = schema.get_properties()?;
    let mut map = JsonMap::new();
    for (name, prop) in properties {
        let prop = prop.resolve(spec);
        let value = match &prop.kind {
            openapiv3::SchemaKind::Type(v) => match v {
                openapiv3::Type::String(string_type) => json!("string"),
                openapiv3::Type::Number(number_type) => json!(0.0),
                openapiv3::Type::Integer(integer_type) => json!(0),
                openapiv3::Type::Object(object_type) => json!({}),
                openapiv3::Type::Array(array_type) => json!([]),
                openapiv3::Type::Boolean {} => json!(false),
            },
            _ => json!("uknown"),
        };
        map.insert(name.clone(), value);
    }
    return Some(Json::Object(map));
}
fn _build_request_schema(
    spec: &openapiv3::OpenAPI,
    op: &openapiv3::Operation,
) -> Option<PayloadSchema> {
    let request = op.request_body.as_ref()?;
    let request = request.resolve(spec).ok()?;
    let media = request.content.get("application/json")?;
    let schema = media.schema.as_ref()?.resolve(spec);

    return _construct_example_from_shema(spec, schema).map(PayloadSchema::new);
}

fn _build_responses_schema(
    spec: &openapiv3::OpenAPI,
    op: &openapiv3::Operation,
) -> Vec<ResponseSchema> {
    return op
        .responses
        .responses
        .iter()
        .map(|(status, response)| {
            let response = response.resolve(spec).ok();
            let schema = response
                .and_then(|t| t.content.get("application/json"))
                .and_then(|t| t.schema.as_ref())
                .map(|t| t.resolve(spec));
            return ResponseSchema::new(
                status.to_string(),
                schema.and_then(|schema| _construct_example_from_shema(spec, schema)),
                response.map(|t| t.description.clone()),
            );
        })
        .collect_vec();
}

pub fn build_endpoint_schema_from_openapi_schema(
    spec: &openapiv3::OpenAPI,
    op: &openapiv3::Operation,
) -> EndpointSchema {
    let payload_schema = _build_request_schema(spec, op);
    let responses_schema = _build_responses_schema(spec, op);
    let schema_parameters = _build_params_schema_map(spec, op);

    return EndpointSchema::new(schema_parameters, payload_schema, responses_schema);
}
