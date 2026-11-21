use itertools::Itertools;
use jq::JsonFilter;
use serde::de;

use crate::cli_output_serializers::{serialize_endpoint_long, simple_serialize_endpoint};
use crate::helper::{ResultExtension, StringResultExtension, to_json_or_nice_error};
use crate::objects::*;
use crate::runtime::{AppLink, EnvironmentLink, Runtime, SendError};
use crate::{cli_api::*, helper};
use serde_json::{self, Map, json};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;
use std::{env, path};

pub fn handle(args: MainArgs, runtime: &mut Runtime, vars: &HashMap<String, String>) -> i32 {
    match handle_internal(args, runtime, vars) {
        Ok(v) => return v,
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
) -> Result<i32, String> {
    validate_high_level_valid(&args)?; // fixme: include those additional validation in clap object
    let mut app = None;
    let mut env = None;
    if let Some(app_name) = &args.app {
        app = Some(r.find_app(Some(app_name.as_str()))?)
    } else if can_app_be_passed(&args.command) {
        app = r.get_default_app().ok();
        if let Some(_) = app {
            r.tunnel.log(&format!("Default app will be used"))
        }
    }

    if let Some(env_name) = args.env {
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
            AppsCommand::Create(AppCreate { name }) => {
                r.create_app(name);
            }
        },
        MasterCommand::Endpoints(endpoints_setup) => {
            let app = require_app_set(app)?;
            match endpoints_setup.command {
                EndpointsCommand::Create(EndpointCreate { name, url, kind }) => {
                    r.create_endpoint(app, url, name, kind, None);
                }
                EndpointsCommand::List(endpoints_list) => {
                    print!(
                        "{}",
                        (app.get(r).endpoints.iter().map(simple_serialize_endpoint)).join("\n")
                    )
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
                None => println!("{}", serialize_endpoint_long(&endpoint.get(r))),
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
                        .user_info(&format!("Sending failed: {failed}, will retry in 3s..."));
                    sleep(Duration::from_secs(3));
                    continue;
                }

                break maybe_result.stringify_er()?;
            };

            let status = result.status();

            if let Ok(text) = &result.text() {
                if let Ok(output_as_json) = serde_json::from_str::<serde_json::Value>(text) {
                    let output = serde_json::to_string_pretty(&output_as_json)
                        .expect("deseralization after serialization must work");
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
