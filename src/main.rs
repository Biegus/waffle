mod cli_api;
mod cli_execution;
mod cli_output_serializers;
mod openapi_helper;
mod helper;
mod logger;
mod objects;
mod runtime;
mod serializer_helper;
mod tests;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;
use std::{collections::HashMap, fs};

use crate::cli_api::{MainArgs, add_context_from_given_env_vars};
use crate::helper::ResultExtension;
use crate::logger::{Tunnel, TunnelOutputKind};
use crate::runtime::*;
use crate::{logger::StdTunnel, objects::Environment};

use crate::objects::{App, Snapshot};
use crate::objects::{HTTPMethod, Hook};
use clap::Parser;
use itertools::Itertools;
use reqwest::StatusCode;
use sysdirs;

const APPS_DIRECTORY: &str = ".waffle";
const HOOK_FILE_PATH: &str = ".waffle-hook";

pub fn get_app_path() -> PathBuf {
    return sysdirs::home_dir()
        .expect("not supported platform")
        .join(Path::new(APPS_DIRECTORY));
}

pub fn read_snapshot() -> Snapshot {
    let final_path = get_app_path();
    fs::create_dir_all(&final_path).unwrap();
    let apps: Vec<App> = serializer_helper::read_list(final_path.to_str().unwrap());
    let hook = fs::read_to_string(HOOK_FILE_PATH)
        .ok()
        .and_then(|e| serde_json::from_str(&e).ok())
        .unwrap_or(Hook::default());
    return Snapshot::new(apps, hook);
}
pub fn create_runtime(tunnel: Box<dyn Tunnel>, snapshot: Snapshot) -> Runtime {
    let client = reqwest::blocking::Client::new();
    return Runtime {
        apps: snapshot.apps,
        hook: snapshot.hook,
        client,
        tunnel: tunnel,
    };
}

pub fn handle_state_change(
    snapshot: &Snapshot,
    org_snapshot: &Snapshot,
    logger: &impl Tunnel,
) -> Result<(), String> {
    if read_snapshot() != *org_snapshot {
        return Err("There was write in between".to_string());
    }
    if snapshot == org_snapshot {
        logger.log("Saving state: no changes detected, will skip save");
        return Ok(());
    }

    if snapshot.apps != org_snapshot.apps {
        logger.log("Saving state: Apps changed, will save");
        serializer_helper::write_list(get_app_path().to_str().unwrap(), snapshot.apps.iter());
    }
    if snapshot.hook != org_snapshot.hook {
        logger.log("Saving state: hook changed, will save");
        serializer_helper::write(HOOK_FILE_PATH, &snapshot.hook);
    }
    return Ok(());
}

fn construct_tunnel_output_kinds(args: &MainArgs) -> HashSet<TunnelOutputKind> {
    use TunnelOutputKind as T;
    HashSet::from_iter(match (args.logs, args.quiet, args.mute) {
        (_, _, true) => vec![],
        (_, true, false) => vec![T::Warning, T::Error],
        (true, false, false) => vec![T::Warning, T::Error, T::Log],
        (false, false, false) => vec![T::Warning, T::Error],
    })
}

fn main() -> Result<(), String> {
    let mut args = MainArgs::parse();

    let vars: HashMap<String, String> = std::env::vars().collect();
    add_context_from_given_env_vars(&mut args, &vars);

    let tunnel = Box::new(StdTunnel::new(construct_tunnel_output_kinds(&args)));

    let org_snapshot = read_snapshot();
    let mut runtime = create_runtime(tunnel.clone(), org_snapshot.clone());

    let code = cli_execution::handle(args, &mut runtime, &vars);
    if code == 0 {
        handle_state_change(&runtime.into_snapshot(), &org_snapshot, tunnel.as_ref())?;
    } else {
        exit(code)
    }
    return Ok(());
}
