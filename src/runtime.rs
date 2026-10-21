use crate::helper;
use crate::logger::Tunel;
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
use itertools::Itertools;
use reqwest;

#[derive(new)]
pub struct Runtime {
    pub apps: Vec<App>,
    pub hook: Hook,
    pub client: reqwest::blocking::Client,
    pub tunnel: Box<dyn Tunel>,
}

#[derive(Clone, Copy)]
pub struct AppLink(pub usize);
#[derive(Clone, Copy)]
pub struct EnvironmentLink(pub usize, pub usize);
#[derive(Clone, Copy)]
pub struct EndpointLink(pub usize, pub usize);
impl AppLink {
    pub fn get(self, runtime: &Runtime) -> &App {
        return &runtime.apps[self.0];
    }
    pub fn get_mut(self, runtime: &mut Runtime) -> &mut App {
        return &mut runtime.apps[self.0];
    }
    pub fn env(self, id: usize) -> EnvironmentLink {
        return EnvironmentLink(self.0, id);
    }
}
impl EnvironmentLink {
    pub fn get(self, runtime: &Runtime) -> &Environment {
        return &runtime.apps[self.0].environments[self.1];
    }
    pub fn get_mut(self, runtime: &mut Runtime) -> &mut Environment {
        return &mut runtime.apps[self.0].environments[self.0];
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
    pub fn create_environment(&mut self, app: AppLink, name: String, url: String) {
        app.get_mut(self).environments.push(Environment {
            name: name,
            url: url,
            auth: None,
        });
    }
    pub fn create_app(&mut self, name: String) {
        self.apps.push(App {
            environments: vec![],
            name,
            endpoints: vec![],
        })
    }
    pub fn get_default_env(&self, app: AppLink) -> Result<EnvironmentLink, String> {
        let app_instance = app.get(self);
        if let Some(default_env_name) = self.hook.default_envs.get(&app_instance.name) {
            return Ok(EnvironmentLink(
                app.0,
                app_instance
                    .environments
                    .iter()
                    .enumerate()
                    .filter(|(i, env)| &env.name == default_env_name)
                    .map(|(i, _)| i)
                    .next()
                    .unwrap(),
            ));
        }
        return Err("No default env for this app".to_string());
    }
    pub fn get_default_app(&self) -> Result<AppLink, String> {
        if let Some(default_app) = &self.hook.default_app {
            return Ok(AppLink(
                self.apps
                    .iter()
                    .enumerate()
                    .filter(|(i, app)| &app.name == default_app)
                    .map(|(i, _)| i)
                    .next()
                    .unwrap(),
            ));
        }
        return Err(format!("No default app specified"));
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
    pub fn find_env(
        &mut self,
        app: AppLink,
        name: Option<&str>,
    ) -> Result<EnvironmentLink, String> {
        if name.is_none() {
            return self.get_default_env(app);
        }
        let name = name.unwrap();
        if name.eq_ignore_ascii_case("default") {
            return self.get_default_env(app);
        }
        let maybe_id = app
            .get(self)
            .environments
            .iter()
            .position(|e| e.name == name)
            .ok_or(format!("No environment with name {name}"));

        return maybe_id.map(|val| app.env(val));
    }
    pub fn find_app(&mut self, name: Option<&str>) -> Result<AppLink, String> {
        if name.is_none() {
            return self.get_default_app();
        }
        let name = name.unwrap();
        if name.eq_ignore_ascii_case("default") {
            return self.get_default_app();
        }

        let maybe_id = self
            .apps
            .iter()
            .position(|e| e.name == name)
            .ok_or(format!("App {name} doesn't exists"));

        return maybe_id.map(|val| AppLink(val));
    }
    pub fn send(
        &self,
        env: EnvironmentLink,
        endpoint: EndpointLink,
        params: HashMap<String, String>,
        pparams: Vec<String>,
        payload: Option<String>,
    ) -> Result<reqwest::blocking::Response, String> {
        let env = env.get(self);
        let endpoint = endpoint.get(self);
        let kind = endpoint.kind;

        let url = self.build_url(endpoint, env, &pparams)?;
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
        return Ok(builder.send().unwrap());
    }

    pub fn set_auth(&mut self, env: EnvironmentLink, auth: AuthMethod) {
        env.get_mut(self).auth = Some(auth);
    }
}
