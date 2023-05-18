use async_trait::async_trait;
use bytes::Bytes;
use fxhash::FxBuildHasher;
use itertools::Itertools;
use parking_lot::RwLock;
use reqwest::{
    cookie::CookieStore, header::HeaderValue, Client as ReqwestClient,
    ClientBuilder as ReqwestClientBuilder, IntoUrl, Method, RequestBuilder, Url,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::HashMap, sync::Arc};

use crate::{ApiResponse, AuthenticatedClient, BaseClient, RequestResult};

pub use reqwest::ClientBuilder;
pub use reqwest::Proxy;

const CSRF_TOKEN_HEADER: &str = "X-CSRF-TOKEN";
const AUTHENTICATION_COOKIE_NAME: &str = ".ROBLOSECURITY";

#[derive(Default, Debug)]
struct StaticSharedJar(RwLock<HashMap<String, String, FxBuildHasher>>);
impl StaticSharedJar {
    fn new() -> Self {
        Self::default()
    }
    fn insert(&self, name: &str, value: &str) {
        let mut cookie_map = self.0.write();
        cookie_map.insert(String::from(name), String::from(value));
    }
    fn remove(&self, name: &str) {
        let mut cookie_map = self.0.write();
        cookie_map.remove(name);
    }
    fn get(&self, name: &str) -> Option<String> {
        let cookie_map = self.0.read();
        cookie_map.get(name).cloned()
    }
    fn clear(&self) {
        let mut cookie_map = self.0.write();
        cookie_map.clear();
    }
}
impl CookieStore for StaticSharedJar {
    fn cookies(&self, _url: &Url) -> Option<HeaderValue> {
        let cookie_string = self
            .0
            .read()
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .join("; ");

        if cookie_string.is_empty() {
            None
        } else {
            HeaderValue::from_maybe_shared(Bytes::from(cookie_string)).ok()
        }
    }
    fn set_cookies(&self, _cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, _url: &Url) {}
}

#[derive(Debug, Clone, Default)]
pub struct Client {
    inner: Arc<InnerClient>,
}
impl Client {
    #[must_use]
    pub fn new(builder: ReqwestClientBuilder) -> Self {
        Self {
            inner: Arc::new(InnerClient::new(builder)),
        }
    }
}

#[async_trait]
impl BaseClient for Client {
    async fn request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.inner.request(method, url, query, payload).await
    }
}

#[derive(Debug, Default)]
struct InnerClient {
    client: ReqwestClient,
    csrf_token: RwLock<Option<String>>,
}
impl InnerClient {
    fn build_request<'a, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestBuilder {
        let is_get = matches!(&method, &Method::GET);
        let mut builder = self.client.request(method, url);
        if let Some(query) = query.into() {
            builder = builder.query(query);
        };
        builder = match payload.into() {
            Some(payload) => builder.json(&payload),
            None => builder.body(""),
        };
        if !is_get {
            if let Some(csrf_token) = &*self.csrf_token.read() {
                builder = builder.header(CSRF_TOKEN_HEADER, csrf_token);
            }
        }
        builder
    }
    pub async fn request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        let builder = self.build_request(method, url, query, payload);
        let mut response = builder.try_clone().unwrap().send().await?;
        if let Some(csrf_token) = response.headers().get(CSRF_TOKEN_HEADER) {
            *self.csrf_token.write() = Some(csrf_token.to_str().unwrap().to_owned());
            response = builder.header(CSRF_TOKEN_HEADER, csrf_token).send().await?;
        }
        Ok(response.json::<ApiResponse<T>>().await?.0?)
    }
    pub fn new(builder: ReqwestClientBuilder) -> Self {
        Self {
            client: builder.build().unwrap(),
            csrf_token: RwLock::new(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CookieClient {
    inner: Arc<InnerCookieClient>,
}
impl CookieClient {
    #[must_use]
    pub fn new(builder: ReqwestClientBuilder, auth_cookie: &str) -> Self {
        Self {
            inner: Arc::new(InnerCookieClient::new(builder, auth_cookie)),
        }
    }
    pub fn insert_cookie(&self, name: &str, value: &str) {
        self.inner.insert_cookie(name, value);
    }
    pub fn remove_cookie(&self, name: &str) {
        self.inner.remove_cookie(name);
    }
    #[must_use]
    pub fn get_cookie(&self, name: &str) -> Option<String> {
        self.inner.get_cookie(name)
    }
    pub fn clear_cookies(&self) {
        self.inner.clear_cookies();
    }
    pub fn set_auth_cookie(&self, cookie: &str) {
        self.inner.set_auth_cookie(cookie);
    }
}

#[async_trait]
impl AuthenticatedClient for CookieClient {
    async fn authenticated_request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.inner.request(method, url, query, payload).await
    }
}

#[derive(Debug)]
struct InnerCookieClient {
    client: ReqwestClient,
    csrf_token: RwLock<Option<String>>,
    jar: Arc<StaticSharedJar>,
}
impl InnerCookieClient {
    fn build_request<'a, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestBuilder {
        let is_get = matches!(&method, &Method::GET);
        let mut builder = self.client.request(method, url);
        if let Some(query) = query.into() {
            builder = builder.query(query);
        };
        builder = match payload.into() {
            Some(payload) => builder.json(&payload),
            None => builder
                .body("")
                .header("Content-Length", 0)
                .header("Content-Type", "application/json"),
        };
        if !is_get {
            if let Some(csrf_token) = &*self.csrf_token.read() {
                builder = builder.header(CSRF_TOKEN_HEADER, csrf_token);
            }
        }
        builder
    }
    pub async fn request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        let builder = self.build_request(method, url, query, payload);
        let mut response = builder.try_clone().unwrap().send().await?;
        if let Some(csrf_token) = response.headers().get(CSRF_TOKEN_HEADER) {
            *self.csrf_token.write() = Some(csrf_token.to_str().unwrap().to_owned());
            response = builder.header(CSRF_TOKEN_HEADER, csrf_token).send().await?;
        }
        Ok(response.json::<ApiResponse<T>>().await?.0?)
    }
    fn new(builder: ReqwestClientBuilder, auth_cookie: &str) -> Self {
        let jar = Arc::new(StaticSharedJar::new());
        jar.insert(AUTHENTICATION_COOKIE_NAME, auth_cookie);
        Self {
            client: builder.cookie_provider(jar.clone()).build().unwrap(),
            csrf_token: RwLock::new(None),
            jar,
        }
    }
    pub fn insert_cookie(&self, name: &str, value: &str) {
        self.jar.insert(name, value);
    }
    pub fn remove_cookie(&self, name: &str) {
        self.jar.remove(name);
    }
    pub fn get_cookie(&self, name: &str) -> Option<String> {
        self.jar.get(name)
    }
    pub fn clear_cookies(&self) {
        self.jar.clear();
    }
    pub fn set_auth_cookie(&self, cookie: &str) {
        self.jar.insert(".ROBLOSECURITY", cookie);
    }
}
