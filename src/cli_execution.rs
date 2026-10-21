use itertools::Itertools;
use jq::JsonFilter;

use crate::helper::{ResultExtension, StringResultExtension, to_json_or_nice_error};
use crate::objects::*;
use crate::runtime::Runtime;
use crate::{cli_api::*, helper};
use serde_json::{self, Map};
use std::collections::HashMap;
use std::fs;
use std::process::exit;
use std::str::FromStr;

pub fn handle(args: MainArgs, runtime: &mut Runtime) -> i32 {
    match handle_internal(args, runtime) {
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

fn handle_internal(args: MainArgs, runtime: &mut Runtime) -> Result<(), String> {
    match args.command {
        MasterCommand::Environments(environment_setup) => match environment_setup.command {
            EnvironmentsCommand::Create(EnvironmentCreate { name, url }) => {
                runtime.create_environment(name, url);
            }
        },
        MasterCommand::Apps(app_setup) => match app_setup.command {
            AppCommand::Create(AppCreate { name }) => {
                runtime.create_app(name);
            }
        },
        MasterCommand::Endpoints(endpoints_setup) => {
            let app = runtime.get_default_app();
            match endpoints_setup.command {
                EndpointsCommand::Create(EndpointCreate { name, url, kind }) => {
                    runtime.create_endpoint(app, url, name, kind);
                }
                EndpointsCommand::List(endpoints_list) => {
                    let output = app
                        .get(runtime)
                        .endpoints
                        .iter()
                        .map(|t| format!("{} - {}", t.name, t.url))
                        .join("\n");
                    println!("{output}");
                }
            }
        }
        MasterCommand::Endpoint(endpoint_setup) => {
            let env = runtime.get_default_env();
            let app = runtime.get_default_app();
            let endpoint = runtime.get_endpoint(app, &endpoint_setup.name);
            match endpoint_setup.command {
                EndpointCommand::Send(endpoint_send) => {
                    let params: HashMap<String, String> = endpoint_send
                        .params
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                    let mut payload: Option<Json> =
                        handle_entry_payload(runtime, endpoint_send.input)?;
                    payload = handle_payload_mod(
                        runtime,
                        payload,
                        endpoint_send.mod_jq,
                        endpoint_send.mod_insert,
                    )?;

                    let result = runtime.send(
                        env,
                        endpoint,
                        params,
                        endpoint_send.pparams,
                        payload.map(|t| t.to_string()),
                    );

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
            let environment = runtime.find_env(&environment_setup.name);
            match environment_setup.command {
                EnvironmentCommand::SetAuthToken(SetAuthToken { token }) => {
                    runtime.set_auth(environment, AuthMethod::SetBearerToken(token));
                }
                EnvironmentCommand::SetAuthSimple(SetAuthSimple { username, password }) => {
                    runtime.set_auth(environment, AuthMethod::SimpleAuth { username, password });
                }
            }
        }
    }
    Ok(())
}
