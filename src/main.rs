mod cli_api;
mod cli_execution;
mod helper;
mod logger;
mod objects;
mod runtime;
mod serializer_helper;
mod tests;

use std::path::{Path, PathBuf};
use std::process::exit;
use std::{collections::HashMap, fs};

use crate::cli_api::MainArgs;
use crate::logger::Tunel;
use crate::runtime::*;
use crate::{logger::StdTunnel, objects::Environment};

use crate::objects::App;
use crate::objects::{HTTPMethod, Hook};
use clap::Parser;
use itertools::Itertools;
use reqwest::StatusCode;
use sysdirs;

fn fail(t: &str) {
    eprint!("{t}");
    exit(1)
}

const APPS_DIRECTORY: &str = ".waffle";
const HOOK_FILE_PATH: &str = ".waffle-hook";

pub fn get_app_path() -> PathBuf {
    return sysdirs::home_dir()
        .expect("not supported platform")
        .join(Path::new(APPS_DIRECTORY));
}

pub fn create_runtime(tunnel: Box<dyn Tunel>) -> Runtime {
    let final_path = get_app_path();
    fs::create_dir_all(&final_path).unwrap();
    let apps: Vec<App> = serializer_helper::read_list(final_path.to_str().unwrap());
    let hook = fs::read_to_string(HOOK_FILE_PATH)
        .ok()
        .and_then(|e| serde_json::from_str(&e).ok())
        .unwrap_or(Hook::default());

    let client = reqwest::blocking::Client::new();
    return Runtime {
        apps,
        hook,
        client,
        tunnel,
    };
}

pub fn save_state_from_runtime(runtime: &Runtime) {
    serializer_helper::write_list(get_app_path().to_str().unwrap(), &runtime.apps);
    serializer_helper::write(HOOK_FILE_PATH, &runtime.hook);
}
fn main() {
    let args = MainArgs::parse();
    let tunnel = Box::new(StdTunnel::new(args.logs));
    let mut runtime = create_runtime(tunnel);

    let code = cli_execution::handle(args, &mut runtime);
    if code == 0 {
        save_state_from_runtime(&runtime);
    } else {
        exit(code)
    }
}
