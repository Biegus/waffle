use chrono::{DateTime, NaiveDateTime, Utc};
use clap;
use derive_new::new;
use reqwest::{self};
use serde;
use std::{any::Any, collections::HashMap, time::Duration};

use crate::helper;

pub type Json = serde_json::Value;
pub type JsonMap = serde_json::Map<String, Json>;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, Debug, clap::ValueEnum)]
pub enum HTTPMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}
#[derive(new, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Parameter {
    pub required: bool,
    pub name: String,
}
#[derive(Clone, Debug, new, serde::Serialize, serde::Deserialize, Default)]
pub struct EndpointSchema {
    pub parameters: Option<Vec<Parameter>>,
    pub consumes: Option<Vec<String>>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Endpoint {
    pub url: String,
    pub kind: HTTPMethod,
    pub name: String,
    pub schema: EndpointSchema,
}

#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Environment {
    pub name: String,
    pub url: String,
    pub auth: Option<AuthMethod>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct App {
    pub name: String,
    pub environments: Vec<Environment>,
    pub endpoints: Vec<Endpoint>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct Hook {
    pub default_app: Option<String>,
    pub default_envs: HashMap<String, String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum AuthMethod {
    SetBearerToken(String),
    AutoBearerToken(AutoTokenMethod, Option<String>),
    SimpleAuth {
        username: String,
        password: Option<String>,
    },
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, derive_new::new)]
pub struct AutoTokenMethod {
    pub method: HTTPMethod,
    pub endpoint: String,
    pub payload: Json,
    pub output_jq: String,
}


impl Into<reqwest::Method> for HTTPMethod{
    fn into(self) -> reqwest::Method {
        return reqwest::Method::from_bytes(format!("{self:?}").as_bytes()).unwrap();
    }
}

