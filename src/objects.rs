use chrono::{DateTime, NaiveDateTime, Utc};
use clap;
use derive_new::new;
use reqwest::{self};
use serde;
use std::{any::Any, collections::HashMap, default, time::Duration};

use crate::helper;

pub type Json = serde_json::Value;
pub type JsonMap = serde_json::Map<String, Json>;

#[derive(
    Clone, Copy, serde::Serialize, serde::Deserialize, Debug, clap::ValueEnum, Eq, PartialEq, Hash,
)]
pub enum HTTPMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

#[derive(
    PartialEq, Eq, Hash, Clone, Debug, serde::Serialize, serde::Deserialize, derive_more::Display,
)]
pub enum ParameterKind {
    Query,
    Header,
    Path,
    Cookie,
}
#[derive(new, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Parameter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub param_type: Option<String>,
}
#[derive(new, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct PayloadSchema {
    pub schema_example: Json,
}
#[derive(new, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ResponseSchema {
    pub status_code: String,
    pub schema_example: Option<Json>,
    pub description: Option<String>,
}
#[derive(Clone, Debug, new, serde::Serialize, serde::Deserialize, Default, PartialEq, Eq)]
pub struct EndpointSchema {
    pub params: HashMap<ParameterKind, Vec<Parameter>>,
    pub request_schema: Option<PayloadSchema>,
    #[serde(default)]
    pub responses_schema: Vec<ResponseSchema>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub url: String,
    pub kind: HTTPMethod,
    pub name: String,
    pub schema: Option<EndpointSchema>,
}

#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct Environment {
    pub name: String,
    pub url: String,
    pub auth: Option<AuthMethod>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct App {
    pub name: String,
    pub environments: Vec<Environment>,
    pub endpoints: Vec<Endpoint>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug, Default, PartialEq, Eq)]
pub struct Hook {
    pub default_app: Option<String>,
    pub default_envs: HashMap<String, String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum AuthMethod {
    SetBearerToken(String),
    AutoBearerToken(AutoTokenMethod, Option<String>),
    SimpleAuth {
        username: String,
        password: Option<String>,
    },
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, derive_new::new, PartialEq, Eq)]
pub struct AutoTokenMethod {
    pub method: HTTPMethod,
    pub endpoint: String,
    pub payload: Json,
    pub output_jq: String,
}

impl Into<reqwest::Method> for HTTPMethod {
    fn into(self) -> reqwest::Method {
        return reqwest::Method::from_bytes(format!("{self:?}").as_bytes()).unwrap();
    }
}
#[derive(Clone, Debug, derive_new::new, PartialEq, Eq)]
pub struct Snapshot {
    pub apps: Vec<App>,
    pub hook: Hook,
}
