use std::collections::HashMap;
use std::convert::identity;
use std::fmt::Display;
use std::iter::repeat;

use crate::objects::{App, Endpoint, EndpointSchema, Json, JsonMap, Parameter, PayloadSchema};
use crate::objects::{ParameterKind, ResponseSchema};
use colored_json::ToColoredJson;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json;
#[derive(Debug, Clone, derive_new::new)]
pub struct SerializationContext {
    color: bool,
}

#[derive(Debug, Clone, derive_new::new)]
pub enum StringNode {
    Value(String),
    Inner(Vec<StringNode>),
    Nothing,
}

pub trait StringSerialize {
    fn serialize_flat(&self, context: &SerializationContext) -> String;
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return StringNode::Value(self.serialize_flat(context));
    }
}
#[typetag::serialize(tag = "type")]
pub trait FullSerialize: StringSerialize {}
#[typetag::serialize]
impl<T: StringSerialize + serde::Serialize> FullSerialize for T {}

impl<T: StringSerialize> StringSerialize for Option<T> {
    fn serialize_flat(&self, context: &SerializationContext) -> String {
        match self {
            Some(v) => v.serialize_flat(context),
            None => format!("None"),
        }
    }
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        match self {
            Some(v) => v.serialize_str_deep(context),
            None => StringNode::Nothing,
        }
    }
}

impl StringSerialize for String {
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return StringNode::Inner(
            self.split("\n")
                .map(str::to_string)
                .map(StringNode::Value)
                .collect(),
        );
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return self.replace("\n", "\\n");
    }
}
impl<T> StringSerialize for Vec<T>
where
    T: StringSerialize,
{
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return StringNode::Inner(
            self.iter()
                .map(|t| match StringSerialize::serialize_str_deep(t, context) {
                    StringNode::Value(v) => vec![StringNode::Value(v)],
                    StringNode::Inner(string_nodes) => string_nodes,
                    StringNode::Nothing => vec![],
                })
                .flat_map(identity)
                .collect(),
        );
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return format!("{} elements", self.len());
    }
}

impl<K, V> StringSerialize for HashMap<K, V>
where
    K: Display,
    V: StringSerialize,
{
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return StringNode::Inner(
            self.iter()
                .map(|(key, item)| {
                    vec![
                        StringNode::Value(format!("{}:", key.to_string())),
                        item.serialize_str_deep(context),
                    ]
                })
                .flat_map(identity)
                .collect(),
        );
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return format!("{} elements", self.len());
    }
}

impl<K, V> StringSerialize for (K, V)
where
    K: Display,
    V: StringSerialize,
{
    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return format!("{}:{}", self.0, self.1.serialize_flat(context));
    }
}

impl StringNode {
    pub fn into_string(self, identiation: usize, top: bool) -> String {
        let mut new_top = false;
        let mut new_identatation = if top { identiation } else { identiation + 1 };
        let iden: String = repeat(' ').take(identiation).collect();
        match self {
            StringNode::Value(v) => {
                format!("{iden}{v}\n")
            }
            StringNode::Inner(nodes) => {
                // skip dummy packing

                if nodes.len() <= 1 {
                    new_top = top;
                    new_identatation = identiation;
                }
                nodes
                    .into_iter()
                    .map(|t| t.into_string(new_identatation, new_top))
                    .join("")
            }
            StringNode::Nothing => String::default(),
        }
    }
    pub fn with(self, t: String) -> StringNode {
        match self {
            StringNode::Value(v) => StringNode::Value(format!("{t}: {v}")),
            StringNode::Inner(string_nodes) => {
                StringNode::Inner(vec![StringNode::Value(t), StringNode::Inner(string_nodes)])
            }
            StringNode::Nothing => StringNode::Value(format!("{t}: Unknown")),
        }
    }
}

pub fn _serializer_required(v: bool) -> &'static str {
    return match v {
        true => "!",
        false => "o",
    };
}

impl StringSerialize for Json {
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        let res = serde_json::to_string_pretty(self);
        if res.is_err() {
            return StringNode::Value("Invalid JSON".to_string());
        }
        let res = res.unwrap();
        if context.color {
            return res
                .to_colored_json(colored_json::ColorMode::On)
                .expect("Value was straight from deserialization")
                .serialize_str_deep(context);
        }
        return res.serialize_str_deep(context);
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return format!("JSON Value");
    }
}

impl StringSerialize for Parameter {
    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return format!(
            "{} {}:{}",
            _serializer_required(self.required),
            self.name,
            self.param_type.as_deref().unwrap_or("?"),
        );
    }
}
impl StringSerialize for PayloadSchema {
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return Json::from(self.schema_example.clone()).serialize_str_deep(context);
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        panic!()
    }
}
impl StringSerialize for ResponseSchema {
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return StringNode::Inner(vec![
            StringNode::Value(format!(
                "{}:{}",
                self.status_code,
                self.description.as_ref().unwrap_or(&"?".to_string())
            )),
            Json::from(self.schema_example.clone()).serialize_str_deep(context),
        ]);
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        panic!()
    }
}

impl StringSerialize for EndpointSchema {
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        return StringNode::Inner(vec![
            StringNode::Value("Parameters:".to_string()),
            self.params.serialize_str_deep(context),
            self.request_schema
                .serialize_str_deep(context)
                .with("Request".to_string()),
            StringNode::Value("Responses:".to_string()),
            self.responses_schema.serialize_str_deep(context),
        ]);
    }

    fn serialize_flat(&self, context: &SerializationContext) -> String {
        panic!()
    }
}
impl StringSerialize for Endpoint {
    fn serialize_flat(&self, context: &SerializationContext) -> String {
        return format!("{}: {:?} {}", self.name, self.kind, self.url);
    }
    fn serialize_str_deep(&self, context: &SerializationContext) -> StringNode {
        let top = StringNode::Value(self.serialize_flat(context));
        let schema = self.schema.serialize_str_deep(context);
        return StringNode::Inner(vec![top, schema]);
    }
}
