use derive_more::Display;
use itertools::Itertools;
use jq::JsonFilter;
use std::{collections::HashMap, net::ToSocketAddrs, str::FromStr};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    helper::{self, ResultExtension, to_json_or_nice_error},
    objects::{HTTPMethod, Json, JsonMap},
};

#[derive(Clone, Debug)]
pub enum PayloadInput {
    File(String),
    Template(String),
    // We can't use JsonFilter type here because it doesn't implement debug :/
    // todo: wrapper should be added
    Jq(String),
    Null,
}
impl FromStr for PayloadInput {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s == "null" {
            return Ok(PayloadInput::Null);
        }
        if !s.contains(":") {
            return Ok(Self::File(s.to_string()));
        }
        let parts: Vec<_> = s.splitn(2, ":").collect();
        match parts[0] {
            "t" => return Ok(Self::Template(parts[1].to_string())),
            "jq" => {
                // PayloadInput needs to be debug, so it cannot contained Json filter, we still try whether it parsers
                JsonFilter::from_str(parts[1])
                    .str_er_with_context("Failed to parse payload input jq")?;
                return Ok(Self::Jq(parts[1].to_string()));
            }
            _ => return Err(format!("Invalid type: {}", parts[0])),
        }
    }
}

#[derive(Clone)]
struct CliMap(pub HashMap<String, String>);

#[derive(Clone, ValueEnum, Debug, PartialEq, Copy, Display)]
pub enum ColorMode {
    Always,
    Auto,
    NoColor,
}

#[derive(Clone, Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct MainArgs {
    #[command(subcommand)]
    pub command: MasterCommand,
    #[arg(long, short('v'))]
    pub logs: bool,
    #[arg(long)]
    pub quiet: bool,
    #[arg(long)]
    pub mute: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long, default_value = "auto")]
    pub color: ColorMode,
    #[arg(long, short)]
    pub app: Option<String>,
    #[arg(long, short)]
    pub env: Option<String>,
}

#[derive(Clone, Subcommand, Debug)]
pub enum MasterCommand {
    #[clap(visible_alias = "envs")]
    Environments(EnvironmentsSetup),
    #[clap(visible_alias = "env")]
    Environment(EnvironmentSetup),
    #[clap(visible_alias = "as")]
    Apps(AppsSetup),
    App(AppSetup),
    #[clap(visible_alias = "s")]
    Send(Send),

    #[clap(visible_alias = "es")]
    Endpoints(EndpointsSetup),
    #[clap(visible_alias = "e")]
    Endpoint(EndpointSetup),
    #[clap(visible_alias = "h")]
    Hook(HookSetup),
}

#[derive(Clone, Args, Debug)]
pub struct HookSetup {
    #[command(subcommand)]
    pub command: HookCommand,
}
#[derive(Clone, Subcommand, Debug)]
pub enum HookCommand {
    #[clap(visible_alias = "da")]
    DefaultApp(DefaultApp),
    #[clap(visible_alias = "de")]
    DefaultEnv(DefaultEnv),
}

#[derive(Clone, Args, Debug)]
pub struct DefaultApp {
    pub value: Option<String>,
}

#[derive(Clone, Args, Debug)]
pub struct DefaultEnv {
    pub value: Option<String>,
}

#[derive(Clone, Args, Debug)]
pub struct EndpointsSetup {
    #[command(subcommand)]
    pub command: EndpointsCommand,
}
#[derive(Clone, Args, Debug)]
pub struct EndpointSetup {
    pub name: String,
    #[command(subcommand)]
    pub command: Option<EndpointCommand>,
    #[arg(long, short, default_value = "full")]
    pub selector: EndpointGetPart,
}
#[derive(Clone, Subcommand, Debug)]
pub enum EndpointsCommand {
    #[clap(visible_alias = "c")]
    Create(EndpointCreate),
    #[clap(visible_alias = "l")]
    List(EndpointsList),

    LoadSchema(LoadSchema),
}
#[derive(Clone, Subcommand, Debug)]
pub enum EndpointCommand {
    Rename(EndpointRename),
}
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum EndpointGetPart {
    Response,
    Request,
    Short,
    Full,
}
#[derive(Clone, Args, Debug)]
pub struct EndpointRename {
    pub to: String,
}
#[derive(Clone, Args, Debug)]
pub struct TemplatesSetup {
    #[command(subcommand)]
    command: TemplatesCommand,
}
#[derive(Clone, Subcommand, Debug)]
pub enum TemplatesCommand {
    Create(TemplateCreate),
}

#[derive(Clone, Args, Debug)]
pub struct TemplateCreate {
    pub name: String,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long, value_parser=parse_map_entry)]
    pub insert: Option<Vec<(String, String)>>,
    #[arg(short, long)]
    pub jq: Option<String>,
}
#[derive(Clone, ValueEnum, Debug, PartialEq, Copy)]
pub enum RetryType {
    Net,
    AuthenticationMisbehaved,
}

#[derive(Clone, Args, Debug)]
pub struct Send {
    pub endpoint: String,
    pub pparams: Vec<String>,

    #[arg(short, long, value_parser=parse_map_entry)]
    pub params: Option<Vec<(String, String)>>,

    //entry
    #[arg(long)]
    pub input: Option<PayloadInput>,

    // mods
    #[arg(long)]
    pub mod_jq: Option<String>,

    // outs
    #[arg(long)]
    pub out_get: Option<String>,

    #[arg(short, long)]
    pub retry_on: Vec<RetryType>,
}

#[derive(Clone, Args, Debug)]
pub struct EndpointCreate {
    pub name: String,
    pub kind: HTTPMethod,
    pub url: String,
}
#[derive(Clone, Args, Debug)]
pub struct EndpointsList {}
#[derive(Clone, Args, Debug)]
pub struct AppsSetup {
    #[command(subcommand)]
    pub command: AppsCommand,
}
#[derive(Clone, Args, Debug)]
pub struct AppSetup {
    pub app: String,
    #[command(subcommand)]
    pub command: AppCommand,
}
#[derive(Clone, Subcommand, Debug)]
pub enum AppCommand {}
#[derive(Clone, Args, Debug)]
pub struct LoadSchema {
    pub file: String,
}
#[derive(Clone, Subcommand, Debug)]
pub enum AppsCommand {
    Create(AppCreate),
}
#[derive(Clone, Args, Debug)]
pub struct AppCreate {
    pub name: String,
}
#[derive(Clone, Args, Debug)]
pub struct EnvironmentSetup {
    pub name: Option<String>,
    #[command(subcommand)]
    pub command: EnvironmentCommand,
}
#[derive(Clone, Subcommand, Debug)]
pub enum EnvironmentCommand {
    SetAuthToken(SetAuthToken),
    SetAutoAuthToken(SetAutoAuthToken),
    SetAuthSimple(SetAuthSimple),
}
#[derive(Clone, Args, Debug)]
pub struct SetAutoAuthToken {
    pub method: HTTPMethod,
    pub endpoint: String, // fixme: maybe link to normal endpoint or otherwise handle repetition while setting auth
    pub payload: String,  // fixme: handle json/jq validation at cli api level !!
    pub output_jq: String,
}
#[derive(Clone, Args, Debug)]
pub struct SetAuthToken {
    pub token: String,
}
#[derive(Clone, Args, Debug)]
pub struct SetAuthSimple {
    pub username: String,
    pub password: Option<String>,
}
#[derive(Clone, Args, Debug)]
pub struct EnvironmentsSetup {
    #[command(subcommand)]
    pub command: EnvironmentsCommand,
}
#[derive(Clone, Subcommand, Debug)]
pub enum EnvironmentsCommand {
    Create(EnvironmentCreate),
}
#[derive(Clone, Args, Debug)]
pub struct EnvironmentCreate {
    pub name: String,
    pub url: String,
}
fn parse_map_entry(input: &str) -> Result<(String, String), String> {
    let [left, right] = input.split("=").collect::<Vec<_>>().try_into().map_err(
        |_| "Parameters didn't follow \"x=y\" pattern. There needs to be exactly one \"=\"",
    )?;
    return Ok((left.to_string(), right.to_string()));
}

pub fn can_app_be_passed(t: &MasterCommand) -> bool {
    return !matches!(
        t,
        MasterCommand::Apps(_)
            | MasterCommand::Hook(HookSetup {
                command: HookCommand::DefaultApp(_)
            })
    ) && !matches!(t, MasterCommand::App(_));
}
pub fn can_env_be_passed(t: &MasterCommand) -> bool {
    return can_app_be_passed(t)
        && matches!(
            t,
            MasterCommand::Endpoint(EndpointSetup {
                command: Some(EndpointCommand::Rename(_)),
                ..
            }) | MasterCommand::Send(_),
        );
}

pub fn validate_high_level_valid(t: &MainArgs) -> Result<(), String> {
    if t.app.is_some() && !can_app_be_passed(&t.command) {
        return Err(format!("app parameter is not allowed in such context"));
    }
    if t.env.is_some() && !can_env_be_passed(&t.command) {
        return Err(format!("env parameter is not allowed in such context"));
    }
    return Ok(());
}

#[derive(Clone, Debug)]
pub struct EnvWarning(pub String);
pub fn add_context_from_given_env_vars(
    args: &mut MainArgs,
    vars: &HashMap<String, String>,
) -> Vec<EnvWarning> {
    let one_string: String = "1".to_string();
    let some_one = Some(&one_string);
    let coerce_logs = vars.get("WAFFLE_LOGS") == some_one;
    let coerce_retry = vars.get("WAFFLE_RETRY") == some_one;
    let coerce_no_colors =
        vars.get("NO_COLOR") == some_one || vars.get("WAFFLE_NO_COLOR") == some_one;
    args.logs = args.logs || coerce_logs;

    if coerce_retry && let MasterCommand::Send(send) = &mut args.command {
        send.retry_on.push(RetryType::Net);
        send.retry_on.dedup();
    }
    if coerce_no_colors {
        args.color = ColorMode::NoColor;
    }
    return vec![];
}
