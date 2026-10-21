use crate::objects::*;

use serde::{self, Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use serde_json;
use serde_json::ser::PrettyFormatter;

pub trait Namable {
    fn name(&self) -> &str;
}
impl Namable for Environment {
    fn name(&self) -> &str {
        return &self.name;
    }
}

impl Namable for App {
    fn name(&self) -> &str {
        return &self.name;
    }
}

pub fn read_list<T: DeserializeOwned>(path: &str) -> Vec<T> {
    let mut entries = vec![];
    for file in fs::read_dir(path).unwrap() {
        let content = fs::read_to_string(file.unwrap().path()).unwrap();
        let entry: T = serde_json::from_str(&content).unwrap();
        entries.push(entry);
    }
    return entries;
}

pub fn write<T: Serialize + Debug>(file_path: &str, entry: &T) {
    let content = serde_json::to_string_pretty(entry).unwrap();
    fs::write(file_path, content).unwrap()

}
pub fn write_list<T: Serialize + Namable + Debug>(base_path: &str, entries: &Vec<T>) {
    let base_path = Path::new(base_path);
    for entry in entries {
        let name = entry.name();
        let file_path = base_path.join(name);
        write(file_path.to_str().unwrap(), entry);
    }
}

