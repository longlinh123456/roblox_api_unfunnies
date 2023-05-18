#![deny(
    clippy::suspicious,
    clippy::complexity,
    clippy::perf,
    clippy::style,
    clippy::pedantic,
    clippy::correctness,
    clippy::nursery
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions
)]

use async_trait::async_trait;
use bounded_integer::BoundedU64;
use reqwest::{IntoUrl, Method};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::mem;
use thiserror::Error;

pub mod apis;
pub mod clients;

pub type Id = BoundedU64<1, { i64::MAX as u64 }>;

#[derive(Deserialize)]
#[serde(untagged)]
enum ResultDef<T, E> {
    Ok(T),
    Err(E),
}

#[derive(Deserialize)]
#[serde(from = "ResultDef<T, E>")]
struct UntaggedResult<T, E>(Result<T, E>);
impl<T, E> From<ResultDef<T, E>> for UntaggedResult<T, E> {
    fn from(result: ResultDef<T, E>) -> Self {
        match result {
            ResultDef::Ok(value) => Self(Ok(value)),
            ResultDef::Err(value) => Self(Err(value)),
        }
    }
}

pub type RequestResult<T> = Result<T, Error>;
type ApiResponse<T> = UntaggedResult<T, ApiError>;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("roblox api error: {0}")]
    Api(#[from] ApiError),

    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
}

#[derive(Debug, Deserialize, Clone, Default)]
struct ApiErrors {
    errors: [InnerApiError; 1],
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct InnerApiError {
    pub code: i8,
    pub message: String,
    pub user_facing_message: Option<String>,
}

#[derive(Debug, Deserialize, Error, Clone, Default)]
#[serde(rename_all = "camelCase", from = "ApiErrors")]
#[error("{code}: {}", Self::display_error_message(self))]
pub struct ApiError {
    pub code: i8,
    pub message: String,
    pub user_facing_message: Option<String>,
}
impl ApiError {
    #[must_use]
    pub fn display_error_message(&self) -> &String {
        self.user_facing_message.as_ref().unwrap_or(&self.message)
    }
}
impl From<ApiErrors> for ApiError {
    fn from(mut value: ApiErrors) -> Self {
        Self {
            code: value.errors[0].code,
            message: mem::take(&mut value.errors[0].message),
            user_facing_message: mem::take(&mut value.errors[0].user_facing_message),
        }
    }
}

#[async_trait]
pub trait BaseClient: Sync {
    async fn request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T>;
    async fn get<'a, T: DeserializeOwned>(
        &self,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
    ) -> RequestResult<T> {
        self.request::<T, ()>(Method::GET, url, query, None).await
    }
    async fn post<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.request::<T, U>(Method::POST, url, None, payload).await
    }
    async fn put<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.request::<T, U>(Method::PUT, url, None, payload).await
    }
    async fn patch<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.request::<T, U>(Method::PATCH, url, None, payload)
            .await
    }
    async fn delete<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.request::<T, U>(Method::DELETE, url, None, payload)
            .await
    }
}
#[async_trait]
impl<C: AuthenticatedClient> BaseClient for C {
    async fn request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.authenticated_request(method, url, query, payload)
            .await
    }
}

#[async_trait]
pub trait AuthenticatedClient: Sync {
    async fn authenticated_request<'a, T: DeserializeOwned, U: Serialize>(
        &self,
        method: Method,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T>;
    async fn authenticated_get<'a, T: DeserializeOwned>(
        &self,
        url: impl IntoUrl + Send,
        query: impl Into<Option<&'a [(&'a str, &'a str)]>> + Send,
    ) -> RequestResult<T> {
        self.authenticated_request::<T, ()>(Method::GET, url, query, None)
            .await
    }
    async fn authenticated_post<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.authenticated_request::<T, U>(Method::POST, url, None, payload)
            .await
    }
    async fn authenticated_put<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.authenticated_request::<T, U>(Method::PUT, url, None, payload)
            .await
    }
    async fn authenticated_patch<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.authenticated_request::<T, U>(Method::PATCH, url, None, payload)
            .await
    }
    async fn authenticated_delete<T: DeserializeOwned, U: Serialize>(
        &self,
        url: impl IntoUrl + Send,
        payload: impl Into<Option<U>> + Send,
    ) -> RequestResult<T> {
        self.authenticated_request::<T, U>(Method::DELETE, url, None, payload)
            .await
    }
}
