use std::{collections::HashMap, net::ToSocketAddrs, str::FromStr};
use jq::JsonFilter;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{helper::{self, ResultExtension, to_json_or_nice_error}, objects::{HTTPMethod, Json}};

#[derive(Debug,Clone)]
pub enum PayloadInput{
    File(String),
    Template(String),
    Inline(Json), 
    // We can't use JsonFilter type here because it doesn't implement debug :/
    Jq(String),
    Null,

}
impl FromStr for PayloadInput{
    type Err= String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "null"{
            return Ok(PayloadInput::Null);  
        }
        if !s.contains(":"){
            return Ok(Self::File(s.to_string()))
        }
        let parts: Vec<_> = s.splitn(2, ":").collect();
        match parts[0]{
            "t" => return Ok(Self::Template(parts[1].to_string())),
            "i" => return Ok(Self::Inline(to_json_or_nice_error(parts[1])?)),
            "jq" => {
                // PayloadInput needs to be debug, so it cannot contained Json filter, we still try whether it parsers
                JsonFilter::from_str(parts[1]).str_er_with_context("Failed to parse payload input jq")?;
                return Ok(Self::Jq(parts[1].to_string()))
            }
            _ => return Err(format!("Invalid type: {}", parts[0]))
        }
    }
}


#[derive(Clone)]
struct CliMap(pub HashMap<String, String>);

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct MainArgs {
    #[command(subcommand)]
    pub command: MasterCommand,
    #[arg(long)]
    pub logs: bool,
}

#[derive(Subcommand, Debug)]
pub enum MasterCommand {
    Environments(EnvironmentsSetup),
    Environment(EnvironmentSetup),
    Apps(AppSetup),
    Endpoints(EndpointsSetup),
    Endpoint(EndpointSetup),
}

#[derive(Args, Debug)]
pub struct EndpointsSetup {
    #[command(subcommand)]
    pub command: EndpointsCommand,
}
#[derive(Args, Debug)]
pub struct EndpointSetup {
    pub name: String,
    #[command(subcommand)]
    pub command: EndpointCommand,
}
#[derive(Subcommand, Debug)]
pub enum EndpointsCommand {
    Create(EndpointCreate),
    List(EndpointsList),
}
#[derive(Subcommand, Debug)]
pub enum EndpointCommand {
    Send(EndpointSend),
    Templates(TemplatesSetup),
}
#[derive(Args, Debug)]
pub struct TemplatesSetup {
    #[command(subcommand)]
    command: TemplatesCommand,
}
#[derive(Subcommand, Debug)]
pub enum TemplatesCommand {
    Create(TemplateCreate),
}


#[derive(Args, Debug)]
pub struct TemplateCreate {
    pub name: String,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long, value_parser=validate_map_in_vec)]
    pub insert: Option<Vec<(String, String)>>,
    #[arg(short, long)]
    pub jq: Option<String>,
}

#[derive(Args, Debug)]
pub struct EndpointSend {
    #[arg(short, long, value_parser=validate_map_in_vec)]
    pub params: Option<Vec<(String, String)>>,
    #[arg(long)]
    pub pparams: Option<Vec<String>>,

    //entries
    #[arg(long)]
    pub input: Option<PayloadInput>,

    // mods
    #[arg(long, value_parser=validate_map_in_vec)]
    pub mod_insert: Option<Vec<(String, String)>>,
    #[arg(long)]
    pub mod_jq: Option<String>,

    // outs
    #[arg(long)]
    pub out_get: Option<String>,
}

#[derive(Args, Debug)]
pub struct EndpointCreate {
    pub name: String,
    pub url: String,
    pub kind: HTTPMethod,
}
#[derive(Args, Debug)]
pub struct EndpointsList {}
#[derive(Args, Debug)]
pub struct EndpointWindow {
    pub filter: String,
    pub command: EndpointWindowCommand,
}
#[derive(ValueEnum, Debug, Clone)]
pub enum EndpointWindowCommand {
    Send,
}
#[derive(Args, Debug)]
pub struct AppSetup {
    #[command(subcommand)]
    pub command: AppCommand,
}
#[derive(Subcommand, Debug)]
pub enum AppCommand {
    Create(AppCreate),
}
#[derive(Args, Debug)]
pub struct AppCreate {
    pub name: String,
}
#[derive(Args, Debug)]
pub struct EnvironmentSetup {
    pub name: String,
    #[command(subcommand)]
    pub command: EnvironmentCommand,
}
#[derive(Subcommand, Debug)]
pub enum EnvironmentCommand {
    SetAuthToken(SetAuthToken),
    SetAuthSimple(SetAuthSimple),
}
#[derive(Args, Debug)]
pub struct SetAuthToken {
    pub token: String,
}
#[derive(Args, Debug)]
pub struct SetAuthSimple {
    pub username: String,
    pub password: Option<String>,
}
#[derive(Args, Debug)]
pub struct EnvironmentsSetup {
    #[command(subcommand)]
    pub command: EnvironmentsCommand,
}
#[derive(Subcommand, Debug)]
pub enum EnvironmentsCommand {
    Create(EnvironmentCreate),
}
#[derive(Args, Debug)]
pub struct EnvironmentCreate {
    pub name: String,
    pub url: String,
}
fn validate_map_in_vec(input: &str) -> Result<(String, String), String> {
    let [left, right] = input.split("=").collect::<Vec<_>>().try_into().map_err(
        |_| "Parameters didn't follow \"x=y\" pattern. There needs to be exactly one \"=\"",
    )?;
    return Ok((left.to_string(), right.to_string()));
}
