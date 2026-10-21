

mod objects;
mod helper;
mod runtime;
mod serializer_helper;
mod tests;
mod logger;
mod cli_api;
mod cli_execution;

use std::process::exit;
use std::{collections::HashMap, fs};

use crate::cli_api::MainArgs;
use crate::{objects::Environment, logger::StdTunnel};
use crate::runtime::*;


use clap::Parser;
use itertools::Itertools;
use reqwest::StatusCode;
use crate::objects::App;
use crate::objects::{Globals, HTTPMethod};

fn fail(t:&str){
    eprint!("{t}");
    exit(1)
}

    
pub fn create_runtime(log: bool) -> Runtime {
    let envs: Vec<Environment> = serializer_helper::read_list(".waffle/envs");
    let apps: Vec<App> = serializer_helper::read_list(".waffle/apps");
    let globals: Globals = serde_json::from_str(&fs::read_to_string(".waffle/globals").unwrap()).unwrap();
    let client = reqwest::blocking::Client::new();
    let tunnel =Box::new(StdTunnel::new(log));
    let template_folder = "waffle-templates".to_string();
    return Runtime {
        envs,
        apps,
        globals,
        client,
        tunnel,
        template_folder
    };
}

pub fn save_state_from_runtime(runtime: &Runtime) {
    serializer_helper::write(".waffle/envs", &runtime.envs);
    serializer_helper::write(".waffle/apps", &runtime.apps);
}
fn main() {
    let args = MainArgs::parse();
    let mut runtime = create_runtime(args.logs);

    let code = cli_execution::handle(args, &mut runtime);
    if code == 0{
        save_state_from_runtime(&runtime);
    }
    else{
        exit(code)
    }

}