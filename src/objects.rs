use derive_new::new;
use reqwest::{self};
use std::{any::Any, collections::HashMap};
use clap;
use serde;

use crate::helper;

pub type Json = serde_json::Value;


#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, Debug, clap::ValueEnum)]
pub enum HTTPMethod {
    GET,
    POST,
}
#[derive(new, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Parameter{
    pub required: bool,
    pub name: String,
}
#[derive(Clone, Debug, new, serde::Serialize, serde::Deserialize, Default)]
pub struct EndpointSchema{
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
pub struct App {
    pub name: String,
    pub endpoints: Vec<Endpoint>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Environment {
    pub name: String,
    pub url: String,
    pub auth: Option<AuthMethod>,
}
#[derive(new, Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Globals{
    pub default_app: String,
    pub default_env: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum AuthMethod{
    SetBearerToken(String),
    SimpleAuth{username: String, password: Option<String>},
}