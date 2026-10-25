use itertools::Itertools;
use jq::JsonFilter;

use crate::cli_output_serializers::simple_serialize_endpoint;
use crate::helper::{ResultExtension, StringResultExtension, to_json_or_nice_error};
use crate::objects::*;
use crate::runtime::{AppLink, EnvironmentLink, Runtime};
use crate::{cli_api::*, helper};
use serde_json::{self, Map, json};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::process::exit;
use std::str::FromStr;

pub fn handle(args: MainArgs, runtime: &mut Runtime, vars: &HashMap<String, String>) -> i32 {
    match handle_internal(args, runtime, vars) {
        Ok(_) => return 0,
        Err(msg) => {
            eprintln!("E: {msg}");
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
            PayloadInput::Inline(value) => value,
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

fn handle_internal(
    args: MainArgs,
    r: &mut Runtime,
    vars: &HashMap<String, String>,
) -> Result<(), String> {
    validate_high_level_valid(&args)?; // fixme: include those additional validation in clap object
    let mut app = None;
    let mut env = None;
    if let Some(app_name) = &args.app {
        app = Some(r.find_app(Some(app_name.as_str()))?)
    } else if can_app_be_passed(&args.command) {
        app = r.get_default_app().ok();
        if let Some(app) = app {
            r.tunnel
                .log(&format!("Default app ({}) will be used", &app.get(r).name))
        }
    }

    if let Some(env_name) = args.env {
        let app = require_app_set(app)?;
        env = Some(r.find_env(app, Some(env_name.as_str()))?);
    } else if can_env_be_passed(&args.command) {
        env = r.get_default_env(require_app_set(app)?).ok();
        if can_env_be_passed(&args.command) {
            if let Some(env) = app {
                r.tunnel
                    .log(&format!("Default env ({}) will be used", &env.get(r).name))
            }
        }
    }
    if let Some(app) = app {
        r.tunnel.log(&format!("app: {}", app.get(r).name))
    }
    if let Some(env) = env {
        r.tunnel.log(&format!("env: {}", env.get(r).name))
    }

    match args.command {
        MasterCommand::Environments(environment_setup) => {
            let app = require_app_set(app)?;
            match environment_setup.command {
                EnvironmentsCommand::Create(env_create) => {
                    r.create_environment(app, env_create.name, env_create.url);
                }
            }
        }
        MasterCommand::Apps(app_setup) => match app_setup.command {
            AppCommand::Create(AppCreate { name }) => {
                r.create_app(name);
            }
        },
        MasterCommand::Endpoints(endpoints_setup) => {
            let app = require_app_set(app)?;
            match endpoints_setup.command {
                EndpointsCommand::Create(EndpointCreate { name, url, kind }) => {
                    r.create_endpoint(app, url, name, kind);
                }
                EndpointsCommand::List(endpoints_list) => {
                    print!(
                        "{}",
                        (app.get(r).endpoints.iter().map(simple_serialize_endpoint)).join("\n")
                    )
                }
            }
        }
        MasterCommand::Endpoint(endpoint_setup) => {
            let app = require_app_set(app)?;
            let env = require_env_set(env)?;
            let endpoint = r.get_endpoint(app, &endpoint_setup.name);
            match endpoint_setup.command {
                EndpointCommand::Send(endpoint_send) => {
                    let params: HashMap<String, String> = endpoint_send
                        .params
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                    let mut payload: Option<Json> = handle_entry_payload(r, endpoint_send.input)?;
                    payload = handle_payload_mod(
                        r,
                        payload,
                        endpoint_send.mod_jq,
                        endpoint_send.mod_insert,
                    )?;

                    let payload = payload.map(|t| t.to_string());
                    let payload = payload.as_deref();

                    let result = r.send(
                        env,
                        endpoint,
                        &params,
                        &endpoint_send.pparams,
                        payload
                    )?;

                    let status = result.status();

                    if let Ok(text) = &result.text() {
                        if let Ok(output_as_json) = serde_json::from_str::<serde_json::Value>(text)
                        {
                            let output = serde_json::to_string_pretty(&output_as_json)
                                .expect("deseralization after serialization must work");
                            if status.is_success() {
                                if let Some(get) = endpoint_send.out_get {
                                    match output_as_json.get(&get) {
                                        Some(value) => println!("{value}"),
                                        None => {
                                            return Err(format!(
                                                "{get} is not valid field\n{output}"
                                            ));
                                        }
                                    }
                                    return Ok(());
                                }
                            }
                            eprintln!("{status}");
                            println!("{}", output);
                        } else {
                            if endpoint_send.out_get.is_some() {
                                return Err("Endpoint is not a json, can't grab field".to_string());
                            }
                            eprintln!("{status}");
                            println!("{}", text);
                        }
                    } else {
                        if endpoint_send.out_get.is_some() {
                            return Err("No text, can't grab field".to_string());
                        }
                        println!("{status}:(no text)")
                    }
                }
                EndpointCommand::Templates(templates_setup) => todo!(),
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
                EnvironmentCommand::SetAutoAuthToken(SetAutoAuthToken { method, endpoint, payload, output_jq }) => {
                    let v: Json = serde_json::from_str(&payload).str_er_with_context("Payload was not valid json")?;
                    JsonFilter::from_str(&output_jq).str_er_with_context("filter was not valid jq")?;
                    r.set_auth(environment, AuthMethod::AutoBearerToken(AutoTokenMethod::new(method,endpoint, v, output_jq), None));
                },
            }
        }
        MasterCommand::Hook(hook_setup) => match hook_setup.command {
            HookCommand::SetDefaultApp(set_default_app) => {
                r.hook.default_app = Some(set_default_app.name)
            }
            HookCommand::SetDefaultEnv(set_default_env) => {
                let app = require_app_set(app)?;
                r.hook
                    .default_envs
                    .insert(app.get(r).name.clone(), set_default_env.name);
            }
            HookCommand::GetDefaultApp => {
                print!("{}", r.hook.default_app.clone().unwrap_or("".to_string()))
            }
            HookCommand::GetDefaultEnv => {
                let app = require_app_set(app)?;
                print!(
                    "{}",
                    &r.hook
                        .default_envs
                        .get(&app.get(r).name)
                        .as_deref()
                        .unwrap_or(&"".to_string())
                )
            }
        },
    }
    Ok(())
}
