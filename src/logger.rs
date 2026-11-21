use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{Write, stderr},
};

use colored::Colorize;
use derive_new::new;
use lazy_static::lazy_static;

#[derive(PartialEq, Eq, Clone, Copy, derive_more::Display, Hash)]
pub enum TunnelOutputKind {
    Error,
    UserInfo,
    Warning,
    Log,
    Debug,
}
#[derive(Debug, Clone, Copy)]
pub struct Pin(u16);

pub trait Tunnel {
    fn write(&self, kind: TunnelOutputKind, t: &str);
    fn error(&self, t: &str) {
        self.write(TunnelOutputKind::Error, t)
    }
    fn log(&self, t: &str) {
        self.write(TunnelOutputKind::Log, t);
    }
    fn user_info(&self, t: &str) {
        self.write(TunnelOutputKind::UserInfo, t);
    }
    fn warning(&self, t: &str) {
        self.write(TunnelOutputKind::Warning, t);
    }
}

lazy_static! {
    static ref COLOR_MAP: HashMap<TunnelOutputKind, colored::Color> = HashMap::from_iter([
        (TunnelOutputKind::Debug, colored::Color::Green),
        (TunnelOutputKind::Error, colored::Color::Red),
        (TunnelOutputKind::UserInfo, colored::Color::White),
        (TunnelOutputKind::Warning, colored::Color::Yellow),
        (TunnelOutputKind::Log, colored::Color::BrightBlue)
    ],);
    static ref NAME_MAP: HashMap<TunnelOutputKind, &'static str> = HashMap::from_iter([
        (TunnelOutputKind::Debug, "DEBUG: "),
        (TunnelOutputKind::Error, "ERROR: "),
        (TunnelOutputKind::UserInfo, ""),
        (TunnelOutputKind::Warning, "WARNING: "),
        (TunnelOutputKind::Log, "LOG: ")
    ],);
}

#[derive(Default, Clone, derive_new::new)]
pub struct StdTunnel {
    pub logs: HashSet<TunnelOutputKind>,
    pub color: bool,
}

impl Tunnel for StdTunnel {
    fn write(&self, kind: TunnelOutputKind, t: &str) {
        if self.logs.contains(&kind) {
            if self.color {
                eprintln!("{}{t}", NAME_MAP[&kind].color(COLOR_MAP[&kind]))
            } else {
                eprintln!("{}{t}", NAME_MAP[&kind])
            }
        }
    }
}
