use crate::helper;
use crate::helper::ResultExtension;
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
use std::str::FromStr;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use derive_new::new;
use itertools::Itertools;
use jq::JsonFilter;
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
        return &mut runtime.apps[self.0].environments[self.1];
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
    pub fn find_app(&self, name: Option<&str>) -> Result<AppLink, String> {
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
    fn sure_auth_needs_refresh(&self, env: EnvironmentLink) -> bool {
        return matches!(
            env.get(self).auth,
            Some(AuthMethod::AutoBearerToken(_, None))
        );
    }
    fn can_refresh_help_after_401(&self, env: EnvironmentLink) -> bool {
        return matches!(env.get(self).auth, Some(AuthMethod::AutoBearerToken(_, _)));
    }
    fn refresh_auth(&mut self, env: EnvironmentLink) -> Result<(), String> {
        self.tunnel.log("Refreshing auth");
        let host = env.get(self).url.clone();
        let auth = env.get(self).auth.as_ref();
        if auth.is_none() {
            return Ok(());
        }
        let auth = auth.unwrap();
        match auth {
            AuthMethod::SetBearerToken(_) => {}
            AuthMethod::AutoBearerToken(auto_token_method, _auto_token_cached) => {
                let response = self
                    .client
                    .request(
                        auto_token_method.method.into(),
                        format!("{}/{}", host, auto_token_method.endpoint),
                    )
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&auto_token_method.payload).unwrap())
                    .send()
                    .str_er_with_context("Failed to refresh token")?;
                let status = response.status();
                if status != 200 {
                    return Err(format!(
                        "Token endpoint returned result different than 200, it returned {status}"
                    ));
                }
                let text = response.text().unwrap_or("".to_string());
                let json_value: Json =
                    serde_json::from_str(&text)
                        .str_er_with_context("Non json response from token endpoint")?;
                let filter = JsonFilter::from_str(&auto_token_method.output_jq).expect("Type assumption");
                let value = filter.filter_json(json_value)
                    .str_er_with_context("Couldn't apply jq to result")?;
                let str_value: &str = value
                    .as_str()
                    .ok_or("Token endpoint returned non string as token")?;
                env.get_mut(self).auth = Some(AuthMethod::AutoBearerToken(
                    auto_token_method.clone(),
                    Some(str_value.to_string()),
                ));
            }
            AuthMethod::SimpleAuth { username, password } => {}
        }
        Ok(())
    }

    pub fn send(
        &mut self,
        env: EnvironmentLink,
        endpoint: EndpointLink,
        params: &HashMap<String, String>,
        pparams: &Vec<String>,
        payload: Option<&str>,
    ) -> Result<reqwest::blocking::Response, String> {
        let mut did_refresh = false;
        if self.sure_auth_needs_refresh(env) {
            self.tunnel.log("Auth in incomplete state, will refresh");
            self.refresh_auth(env)?;
            did_refresh = true
        }
        let result = self._send(env, endpoint, params, pparams, payload);
        if !did_refresh && self.can_refresh_help_after_401(env) {
            if let Ok(result) = &result {
                if result.status() == 401 {
                    self.tunnel
                        .log("Received 401 but token may be stale, will refresh");
                    self.refresh_auth(env)?;
                    self.tunnel.log("Sending again, after refreshing auth");
                    return self._send(env, endpoint, params, pparams, payload);
                }
            }
        }
        return result;
    }

    fn _send(
        &mut self,
        env: EnvironmentLink,
        endpoint: EndpointLink,
        params: &HashMap<String, String>,
        pparams: &Vec<String>,
        payload: Option<&str>,
    ) -> Result<reqwest::blocking::Response, String> {
        let env = env.get(self);
        let endpoint = endpoint.get(self);
        let kind = endpoint.kind;

        let url = self.build_url(endpoint, env, pparams)?;
        let payload_str = payload.unwrap_or("").to_string();
        self.tunnel.log(&format!("sending {payload_str} to {url}"));
        let mut builder = self
            .client
            .request(kind.into(), url)
            .query(params)
            .body(payload_str)
            .header("Content-Type", "application/json");
        if let Some(auth) = &env.auth {
            builder = match &auth {
                AuthMethod::SetBearerToken(token) => builder.bearer_auth(token),
                AuthMethod::SimpleAuth { username, password } => {
                    builder.basic_auth(username, password.clone())
                }
                AuthMethod::AutoBearerToken(auth_method, Some(auth_cache)) => {
                    builder.bearer_auth(auth_cache)
                }
                AuthMethod::AutoBearerToken(_, None) => {
                    panic!("Not refreshed auto token");
                }
            }
        }
        return builder.send().str_er_with_context("Failed to send request");
    }

    pub fn set_auth(&mut self, env: EnvironmentLink, auth: AuthMethod) {
        env.get_mut(self).auth = Some(auth);
    }
}
