use derive_new::new;

pub trait Tunel {
    fn log(&self, t: &str);
    fn user_info(&self, t: &str);
    fn error(&self, t: &str);
}
#[derive(new, Default)]
pub struct StdTunnel {
    pub log_to_std: bool,
}

impl Tunel for StdTunnel {
    fn user_info(&self, t: &str) {
        println!("INFO:{t}")
    }
    fn log(&self, t: &str) {
        if self.log_to_std {
            eprintln!("LOG: {t}")
        }
    }

    fn error(&self, t: &str) {
        eprint!("ERROR: {t}")
    }
}
