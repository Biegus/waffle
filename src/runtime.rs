use crate::helper;
use crate::logger::Logger;
use crate::objects::*;
use crate::serializer_helper;
use std::collections::HashMap;
use std::fs;
use std::marker::PhantomData;
use std::ops::Deref;
use std::path;
use std::path::Path;
use std::path::PathBuf;

use derive_new::new;
use reqwest;

#[derive(new)]
pub struct Runtime {
    pub envs: Vec<Environment>,
    pub apps: Vec<App>,
    pub globals: Globals,
    pub client: reqwest::blocking::Client,
    pub tunnel: Box<dyn Logger>,
    pub template_folder: String,
}

#[derive(Clone, Copy)]
pub struct AppLink(pub usize);
#[derive(Clone, Copy)]
pub struct EnvironmentLink(pub usize);
#[derive(Clone, Copy)]
pub struct EndpointLink(pub usize, pub usize);
impl AppLink {
    pub fn get(self, runtime: &Runtime) -> &App {
        return &runtime.apps[self.0];
    }
    pub fn get_mut(self, runtime: &mut Runtime) -> &mut App {
        return &mut runtime.apps[self.0];
    }
}
impl EnvironmentLink {
    pub fn get(self, runtime: &Runtime) -> &Environment {
        return &runtime.envs[self.0];
    }
    pub fn get_mut(self, runtime: &mut Runtime) -> &mut Environment {
        return &mut runtime.envs[self.0];
    }
}
impl EndpointLink {
    pub fn get(self, runtime: &Runtime) -> &Endpoint {
        return &runtime.apps[self.0].endpoints[self.1];
    }
    pub fn get_mut(self, runtime: &mut Runtime) -> &mut Endpoint {
        return &mut runtime.apps[self.0].endpoints[self.1];
    }
}

impl Runtime {
    pub fn create_environment(&mut self, name: String, url: String) {
        self.envs.push(Environment {
            name: name,
            url: url,
            auth: None,
        });
    }
    pub fn create_app(&mut self, name: String) {
        self.apps.push(App {
            name,
            endpoints: vec![],
        })
    }
    pub fn get_default_env(&self) -> EnvironmentLink {
        EnvironmentLink(
            self.envs
                .iter()
                .enumerate()
                .filter(|(i, env)| env.name == self.globals.default_env)
                .map(|(i, _)| i)
                .next()
                .unwrap(),
        )
    }
    pub fn get_default_app(&self) -> AppLink {
        return AppLink(
            self.apps
                .iter()
                .enumerate()
                .filter(|(i, app)| app.name == self.globals.default_app)
                .map(|(i, _)| i)
                .next()
                .unwrap(),
        );
    }
    fn build_url(
        &self,
        endpoint: &Endpoint,
        env: &Environment,
        path_params: &Vec<String>,
    ) -> Result<String, String> {
        let pparamas = helper::get_pparams(&endpoint.url);
        if pparamas.len() != path_params.len() {
            return Err("Invalid amounts of pparams".to_string());
        }
        let mut full = endpoint.url.clone();
        for (schemapparam, pparam) in pparamas.iter().zip(path_params.iter()) {
            full = full.replace(format!("<{}>", schemapparam).as_str(), pparam);
        }
        let result = Ok(format!("{}/{}", env.url, full));
        return result;
    }
    pub fn create_endpoint(&mut self, app: AppLink, url: String, name: String, kind: HTTPMethod) {
        app.get_mut(self).endpoints.push(Endpoint {
            url: url,
            kind: kind,
            name,
            schema: EndpointSchema::default(),
        });
    }
    pub fn get_endpoint<'a>(&'a self, app: AppLink, name: &str) -> EndpointLink {
        let index = app
            .get(self)
            .endpoints
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name == name)
            .next()
            .unwrap()
            .0;
        return EndpointLink(app.0, index);
    }
    pub fn find_env(&mut self, name: &str) -> EnvironmentLink {
        if name.eq_ignore_ascii_case("default") {
            return self.get_default_env();
        }
        return EnvironmentLink(
            self.envs
                .iter_mut()
                .enumerate()
                .filter(|(i, e)| e.name == name)
                .next()
                .unwrap()
                .0,
        );
    }
    pub fn read_template(&self, t: &str, app: AppLink, endpoint: EndpointLink) -> Option<String> {
        return fs::read_to_string(
            Path::new(&self.template_folder)
                .join(&app.get(self).name)
                .join(&endpoint.get(self).name)
                .join(t),
        )
        .ok();
    }
    pub fn send(
        &self,
        env: EnvironmentLink,
        endpoint: EndpointLink,
        params: HashMap<String, String>,
        pparams: Option<Vec<String>>,
        payload: Option<String>,
    ) -> reqwest::blocking::Response {
        let env = env.get(self);
        let endpoint = endpoint.get(self);
        let kind = endpoint.kind;

        let url = self
            .build_url(endpoint, env, &pparams.unwrap_or_default())
            .unwrap();
        let payload_str = payload.clone().unwrap_or("NONE".to_string());
        self.tunnel.log(&format!("sending {payload_str} to {url}"));
        let mut builder = self
            .client
            .request(
                reqwest::Method::from_bytes(format!("{kind:?}").as_bytes()).unwrap(),
                url,
            )
            .query(&params)
            .body(payload.unwrap_or_default())
            .header("Content-Type", "application/json");
        if let Some(auth) = &env.auth {
            builder = match &auth {
                &AuthMethod::SetBearerToken(token) => builder.bearer_auth(token),
                AuthMethod::SimpleAuth { username, password } => {
                    builder.basic_auth(username, password.clone())
                }
            }
        }
        if let Some(AuthMethod::SetBearerToken(token)) = &env.auth {
            builder = builder.bearer_auth(token)
        }
        return builder.send().unwrap();
    }

    pub fn set_auth(&mut self, env: EnvironmentLink, auth: AuthMethod) {
        env.get_mut(self).auth = Some(auth);
    }
}
