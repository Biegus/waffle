use std::collections::{HashSet, VecDeque};

use derive_new::new;

#[derive(PartialEq, Eq, Clone, Copy, derive_more::Display, Hash)]
pub enum TunnelOutputKind {
    Error,
    UserInfo,
    Warning,
    Log,
    Debug,
}

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

#[derive(Default, Clone)]
pub struct StdTunnel {
    pub logs: HashSet<TunnelOutputKind>,
}

impl Tunnel for StdTunnel {
    fn write(&self, kind: TunnelOutputKind, t: &str) {
        if self.logs.contains(&kind) {
            eprintln!("{kind}: {t}")
        }
    }
}

impl StdTunnel {
    pub fn new(logs: HashSet<TunnelOutputKind>) -> StdTunnel {
        return StdTunnel { logs };
    }
}
