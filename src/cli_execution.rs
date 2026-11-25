use itertools::Itertools;
use jq::JsonFilter;
use regex::Regex;
use serde::{Deserialize, Serialize, de};
use tap::Pipe;

use crate::cli_api::ColorMode::{self, Always};
use crate::cli_output_serializers::{FullSerialize, SerializationContext, StringSerialize};
use crate::helper::{
    FoldExpression, ResultExtension, StringResultExtension, to_json_or_nice_error,
};
use crate::objects::*;
use crate::runtime::{AppLink, EndpointLink, EnvironmentLink, Runtime, SendError};
use crate::{cli_api::*, helper};
use colored_json::{prelude::*, to_colored_json_auto};
use serde_json::{self, Map, json};
use std::collections::{BTreeMap, HashMap};
use std::convert::identity;
use std::fs;
use std::io::{IsTerminal, stdin};
use std::mem::take;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;
use std::{env, path};
#[derive(derive_new::new)]
pub struct CliExecutionContext {
    pub args: MainArgs,
    pub vars: HashMap<String, String>,
    pub is_terminal: bool,
    pub color: bool,
}

pub fn handle(runtime: &mut Runtime, execution_context: &mut CliExecutionContext) -> i32 {
    match handle_internal(runtime, execution_context) {
        Ok(v) => return v,
        Err(msg) => {
            runtime.tunnel.error(&format!("{msg}"));
            return 1;
        }
    };
}
fn handle_entry_payload(
    runtime: &mut Runtime,
    entry: Option<PayloadInput>,
) -> Result<Option<Json>, String> {
    return match entry {
        Some(entry) => Ok(Some(match entry {
            PayloadInput::File(path) => {
                let text =
                    fs::read_to_string(path).str_er_with_context("Failed to read input file")?;
                serde_json::from_str(&text)
                    .str_er_with_context("Failed to treat input file as json")?
            }
            PayloadInput::Template(_) => todo!(),
            PayloadInput::Jq(program) => {
                let filter = JsonFilter::from_str(&program).expect("type assumption");
                filter
                    .filter_json(Json::Object(Map::default()))
                    .str_er_with_context("Failed to apply jq program")?
            }
            PayloadInput::Null => Json::Null,
        })),
        None => Ok(None),
    };
}

fn handle_payload_mod(
    runtime: &mut Runtime,
    payload: Option<Json>,
    mod_jq: Option<String>,
    mod_insert: Option<Vec<(String, String)>>,
) -> Result<Option<Json>, String> {
    let mut payload = payload;
    if payload.is_none() {
        if mod_jq.is_some() || mod_insert.is_some() {
            payload = Some(Json::Object(Map::default()))
        } else {
            return Ok(None);
        }
    }
    let mut payload = payload.unwrap();
    if !payload.is_object() && mod_insert.is_some() {
        return Err(format!("Insert is only allowed if input evulates to json map.",).to_string());
    }
    let swap_map: HashMap<String, String> = mod_insert.unwrap_or_default().into_iter().collect();
    for (key, to) in swap_map {
        payload[key] = helper::to_json_or_nice_error(&to)
            .or(helper::to_json_or_nice_error(&format!("\"{to}\"")))
            .add_context("invalid key value")?
    }

    if let Some(mod_jq_str) = mod_jq {
        let mod_jq =
            JsonFilter::from_str(&mod_jq_str).str_er_with_context("Failed to compile jq mod")?;
        payload = mod_jq
            .filter_json(payload)
            .str_er_with_context("Failed to apply jq mod")?
    }

    let payload_str = serde_json::to_string(&payload)
        .str_er_with_context("Updated payload is unexpectedly invalid")?;

    return Ok(Some(payload));
}

fn require_app_set(app: Option<AppLink>) -> Result<AppLink, String> {
    app.ok_or(format!(
        "app should be either specified or included in hook"
    ))
}

fn require_env_set(env: Option<EnvironmentLink>) -> Result<EnvironmentLink, String> {
    env.ok_or(format!(
        "env for given app should be either specified or included in hook"
    ))
}

fn to_formatted_json_string<T: Serialize>(t: T, execution_context: &CliExecutionContext) -> String {
    const ASSUMUPTION: &'static str = "There's an assumption for block to be json convertable";
    let mut res = serde_json::to_string_pretty(&t).expect(ASSUMUPTION);
    if execution_context.is_terminal {
        res = res
            .to_colored_json(match execution_context.color {
                true => colored_json::ColorMode::On,
                false => colored_json::ColorMode::Off,
            })
            .expect(ASSUMUPTION);
    }
    return res;
}
fn handle_internal(
    r: &mut Runtime,
    execution_context: &mut CliExecutionContext,
) -> Result<i32, String> {
    let args = execution_context.args.clone();
    validate_high_level_valid(&args)?; // fixme: include those additional validation in clap object
    let mut app = None;
    let mut env = None;
    let serialization_context = SerializationContext::new(execution_context.color);
    if let Some(app_name) = args.app {
        app = Some(r.find_app(Some(app_name.as_str()))?)
    } else if can_app_be_passed(&args.command) {
        app = r.get_default_app().ok();
        if let Some(_) = app {
            r.tunnel.log(&format!("Default app will be used"))
        }
    }

    if let Some(env_name) = &args.env {
        let app = require_app_set(app)?;
        env = Some(r.find_env(app, Some(env_name.as_str()))?);
    } else if can_env_be_passed(&args.command) {
        env = r.get_default_env(require_app_set(app)?).ok();
        if can_env_be_passed(&args.command) {
            if let Some(_) = app {
                r.tunnel.log(&format!("Default env will be used"))
            }
        }
    }
    if let Some(app) = app {
        r.tunnel.log(&format!("app: {}", app.get(r).name))
    }
    if let Some(env) = env {
        r.tunnel.log(&format!("env: {}", env.get(r).name))
    }

    match args.command.clone() {
        MasterCommand::Environments(environment_setup) => {
            let app = require_app_set(app)?;
            match environment_setup.command {
                EnvironmentsCommand::Create(env_create) => {
                    r.create_environment(app, env_create.name, env_create.url);
                }
            }
        }
        MasterCommand::Apps(app_setup) => match app_setup.command {
            AppsCommand::Create(AppCreate { name }) => {
                r.create_app(name);
            }
        },
        MasterCommand::Endpoints(endpoints_setup) => {
            let app = require_app_set(app)?;
            match endpoints_setup.command {
                EndpointsCommand::Create(EndpointCreate { name, url, kind }) => {
                    r.create_endpoint(app, url, name, kind, None, false);
                }
                EndpointsCommand::List(endpoints_list) => {
                    let mut conditions: FoldExpression<Endpoint, _, _> = FoldExpression::all_new();
                    if let Some(filter) = endpoints_list.filter {
                        let regex = Regex::new(&filter)
                            .str_er_with_context("Failed to parse filter regex")?;
                        conditions
                            .add((move |t: &Endpoint| regex.is_match(&t.name)).pipe(Box::new));
                    }
                    if endpoints_list.only_pure {
                        conditions.add((move |t: &Endpoint| t.pure).pipe(Box::new))
                    }

                    match endpoints_list.action {
                        ListAction::Display => {
                            let endpoints =
                                app.get(r).endpoints.iter().filter(|t| conditions.calc(t));
                            match args.json {
                                false => println!(
                                    "{}",
                                    endpoints
                                        .map(|t| StringSerialize::serialize_flat(
                                            t,
                                            &serialization_context
                                        ))
                                        .collect_vec()
                                        .serialize_str_deep(&serialization_context)
                                        .into_string(0, true)
                                ),

                                true => println!(
                                    "{}",
                                    to_formatted_json_string(
                                        endpoints.collect_vec(),
                                        execution_context
                                    )
                                ),
                            }
                        }
                        ListAction::Delete => {
                            //replace code could be better
                            let endpoints = take(&mut app.get_mut(r).endpoints);
                            let org_size = endpoints.len();

                            let endpoints = endpoints
                                .into_iter()
                                .filter(|t| !conditions.calc(t))
                                .collect_vec();
                            drop(conditions);
                            app.get_mut(r).endpoints = endpoints;
                            r.tunnel.user_info(&format!(
                                "Deleted {}",
                                org_size - app.get(r).endpoints.len()
                            ));
                        }
                    }
                }
                EndpointsCommand::LoadSchema(load_schema) => {
                    let extension = Path::new(&load_schema.file)
                        .extension()
                        .ok_or("Couldn't determine extension of schema file")?;
                    let content = fs::read_to_string(&load_schema.file)
                        .str_er_with_context("During reading schema file to memory")?;
                    let api = match extension.to_str().unwrap() {
                        "yaml" => serde_yaml::from_str(&content)
                            .str_er_with_context("During parsing yaml"),
                        "json" => serde_json::from_str(&content)
                            .str_er_with_context("During parsing json"),
                        a => return Err(format!("Unrecognized extension type: {a}")),
                    }?;

                    r.load_schema(app, api)?;
                }
            }
        }
        MasterCommand::Endpoint(endpoint_setup) => {
            let app = require_app_set(app)?;
            let endpoint = r.get_endpoint(app, &endpoint_setup.name).ok_or_else(|| {
                format!(
                    "Endpoint not found: {} in app {}",
                    endpoint_setup.name,
                    app.get(r).name
                )
            })?;
            match endpoint_setup.command {
                Some(EndpointCommand::Rename(endpoint_rename)) => {
                    endpoint.get_mut(r).name = endpoint_rename.to
                }
                None => {
                    let selector = endpoint_setup.selector;
                    let endpoint_obj = endpoint.get(r);
                    let empty_vec = vec![];
                    let obj: &dyn FullSerialize = match selector {
                        EndpointGetPart::Response => endpoint_obj
                            .schema
                            .as_ref()
                            .map(|t| &t.responses_schema)
                            .unwrap_or(&empty_vec),
                        EndpointGetPart::Request => {
                            &endpoint_obj.schema.clone().and_then(|t| t.request_schema)
                        }
                        EndpointGetPart::Short => endpoint_obj,
                        EndpointGetPart::Full => endpoint_obj,
                    };
                    match args.json {
                        false => println!(
                            "{}",
                            if selector == EndpointGetPart::Short {
                                obj.serialize_flat(&serialization_context)
                            } else {
                                obj.serialize_str_deep(&serialization_context)
                                    .into_string(0, true)
                            }
                        ),
                        true => println!("{}", to_formatted_json_string(obj, &execution_context)),
                    }
                }
            }
        }
        MasterCommand::Environment(environment_setup) => {
            let app = require_app_set(app)?;
            let environment = r.find_env(app, environment_setup.name.as_deref())?;
            match environment_setup.command {
                EnvironmentCommand::SetAuthToken(SetAuthToken { token }) => {
                    r.set_auth(environment, AuthMethod::SetBearerToken(token));
                }
                EnvironmentCommand::SetAuthSimple(SetAuthSimple { username, password }) => {
                    r.set_auth(environment, AuthMethod::SimpleAuth { username, password });
                }
                EnvironmentCommand::SetAutoAuthToken(SetAutoAuthToken {
                    method,
                    endpoint,
                    payload,
                    output_jq,
                }) => {
                    let v: Json = serde_json::from_str(&payload)
                        .str_er_with_context("Payload was not valid json")?;
                    JsonFilter::from_str(&output_jq)
                        .str_er_with_context("filter was not valid jq")?;
                    r.set_auth(
                        environment,
                        AuthMethod::AutoBearerToken(
                            AutoTokenMethod::new(method, endpoint, v, output_jq),
                            None,
                        ),
                    );
                }
            }
        }
        MasterCommand::Hook(hook_setup) => match hook_setup.command {
            HookCommand::DefaultApp(default_app) => match default_app.value {
                Some(value) => r.hook.default_app = Some(value),
                None => print!("{}", r.hook.default_app.clone().unwrap_or("".to_string())),
            },
            HookCommand::DefaultEnv(default_env) => match default_env.value {
                Some(v) => {
                    let app = require_app_set(app)?;
                    r.hook.default_envs.insert(app.get(r).name.clone(), v);
                }
                None => {
                    if app.is_none() {
                        return Ok(0);
                    }
                    let app = require_app_set(app)?;
                    print!(
                        "{}",
                        &r.hook
                            .default_envs
                            .get(&app.get(r).name)
                            .as_deref()
                            .unwrap_or(&"".to_string())
                    );
                }
            },
        },
        MasterCommand::App(app_setup) => {
            let app = r.find_app(Some(&app_setup.app))?;
            r.tunnel.log(&format!("Found app: {}", app.get(r).name));
        }
        MasterCommand::Send(send) => {
            let app = require_app_set(app)?;
            let env = require_env_set(env)?;
            let endpoint = r
                .get_endpoint(app, &send.endpoint)
                .ok_or("Endpoint doesn't exists".to_string())?;
            let params: HashMap<String, String> =
                send.params.unwrap_or_default().into_iter().collect();
            let mut payload: Option<Json> = handle_entry_payload(r, send.input)?;
            payload = handle_payload_mod(r, payload, send.mod_jq, None)?;

            let payload = payload.map(|t| t.to_string());
            let payload = payload.as_deref();

            let mut counter = 0;
            let result = loop {
                let maybe_result = r.send(env, endpoint, &params, &send.pparams, payload);

                if let Err(failed) = maybe_result {
                    if !match failed {
                        SendError::HTTPSendError(_) => send.retry_on.contains(&RetryType::Net),
                        SendError::AuthenticationMethodMisbehaved(_) => {
                            send.retry_on.contains(&RetryType::AuthenticationMisbehaved)
                        }
                        _ => false,
                    } {
                        break Err(failed).stringify_er()?;
                    }
                    r.tunnel
                        .user_info(&format!("Sending failed: {failed}\n\twill retry in 3 s, already retried {counter} times"));
                    counter += 1;
                    sleep(Duration::from_secs(3));
                    continue;
                }

                break maybe_result.stringify_er()?;
            };

            let status = result.status();

            if let Ok(text) = &result.text() {
                if let Ok(output_as_json) = serde_json::from_str::<serde_json::Value>(text) {
                    let output = to_formatted_json_string(&output_as_json, execution_context);
                    if status.is_success() {
                        if let Some(get) = send.out_get {
                            match output_as_json.get(&get) {
                                Some(value) => println!("{value}"),
                                None => {
                                    return Err(format!("{get} is not valid field\n{output}"));
                                }
                            }
                            return Ok(0);
                        }
                    }
                    eprintln!("{status}");
                    println!("{}", output);
                } else {
                    if send.out_get.is_some() {
                        return Err("Endpoint is not a json, can't grab field".to_string());
                    }
                    eprintln!("{status}");
                    println!("{}", text);
                }
            } else {
                if send.out_get.is_some() {
                    return Err("No text, can't grab field".to_string());
                }
                println!("{status}:(no text)")
            }
            if !status.is_success() {
                return Ok(status.as_u16() as i32);
            }
        }
    }
    Ok(0)
}
