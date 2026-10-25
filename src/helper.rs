use std::{
    collections::{HashMap, LinkedList},
    fmt::Display,
    hash::Hash,
};

use regex::Regex;

pub fn insert_error_in_human_way(
    t: &str,
    error: &str,
    pos: (usize, usize),
) -> Result<String, String> {
    // TODO: make this function better, its pretty stinky
    let lines: Vec<_> = t.lines().collect();
    // make it by index
    if pos.0 == 0 || pos.1 == 0 {
        return Ok(error.to_string());
    }
    let pos = (pos.0 - 1, pos.1 - 1);
    if pos.0 >= lines.len() {
        return Err("Error out of lines".to_string());
    }
    let line = lines[pos.0];
    if pos.1 >= line.len() {
        return Err("Error out of column".to_string());
    }
    let mut builder = String::new();
    builder += &lines[0..=pos.0]
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{:03} {t}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    builder += "\n";
    builder += "   ";
    builder += &whitespaceify(&lines[pos.0][0..pos.1]);
    builder += "^";
    builder += error;
    builder += "\n";
    builder += &lines[(pos.0) + 1..]
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{:03} {t}", i + pos.0 + 2))
        .collect::<Vec<_>>()
        .join("\n");
    return Ok(builder);
}
pub fn whitespaceify(t: &str) -> String {
    return t
        .chars()
        .map(|t| {
            if t.is_whitespace() {
                t
            } else {
                " ".chars().next().unwrap()
            }
        })
        .collect();
}

pub fn to_json_or_nice_error(text: &str) -> Result<serde_json::Value, String> {
    match serde_json::from_str(text) {
        Ok(result) => Ok(result),
        Err(err) => {
            let pos: (usize, usize) = (err.line(), err.column());
            match insert_error_in_human_way(text, &format!("{err}"), pos) {
                Ok(content) => Err(content),
                Err(_) => Err(format!("{err}")),
            }
        }
    }
}

pub fn get_pparams(url: &str) -> Vec<String> {
    let mut results = vec![];
    let re = Regex::new(r"<(\w+)>").unwrap();
    for entry in re.captures_iter(url) {
        results.push(entry.get(1).unwrap().as_str().to_string());
    }
    return results;
}

pub trait StringResultExtension<T> {
    fn add_context(self, context: &str) -> Result<T, String>;
}
pub trait ResultExtension<T> {
    fn str_er_with_context(self, context: &str) -> Result<T, String>;
    fn stringify_er(self) -> Result<T, String>;
}
impl<T, E: Display> ResultExtension<T> for Result<T, E> {
    fn str_er_with_context(self, context: &str) -> Result<T, String> {
        self.map_err(|e| format!("{context}:\n{e}"))
    }
    fn stringify_er(self) -> Result<T, String>{
        self.map_err(|e| e.to_string())
    }
}

impl<T> StringResultExtension<T> for Result<T, String> {
    fn add_context(self, context: &str) -> Result<T, String> {
        self.str_er_with_context(context)
    }
}
