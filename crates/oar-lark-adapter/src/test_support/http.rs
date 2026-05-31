use async_trait::async_trait;
use std::collections::VecDeque;

use crate::oauth::{AsyncHttpClient, HttpClient, HttpClientFailure, HttpRequest, HttpResponse};

#[derive(Clone)]
pub(crate) struct FakeHttpClient {
    responses: VecDeque<HttpResponse>,
    error: Option<HttpClientFailure>,
    pub(crate) request: Option<HttpRequest>,
    pub(crate) requests: Vec<HttpRequest>,
}

impl FakeHttpClient {
    pub(crate) fn from_response(response: HttpResponse) -> Self {
        Self {
            responses: VecDeque::from([response]),
            error: None,
            request: None,
            requests: Vec::new(),
        }
    }

    pub(crate) fn from_responses(responses: Vec<HttpResponse>) -> Self {
        Self {
            responses: VecDeque::from(responses),
            error: None,
            request: None,
            requests: Vec::new(),
        }
    }

    pub(crate) fn from_error(error: HttpClientFailure) -> Self {
        Self {
            responses: VecDeque::new(),
            error: Some(error),
            request: None,
            requests: Vec::new(),
        }
    }
}

impl HttpClient for FakeHttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.request = Some(request.clone());
        self.requests.push(request);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.responses.pop_front().expect("response exists"))
    }
}

#[async_trait]
impl AsyncHttpClient for FakeHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.request = Some(request.clone());
        self.requests.push(request);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.responses.pop_front().expect("response exists"))
    }
}

#[derive(Clone)]
pub(crate) struct AsyncFakeHttpClient {
    pub(crate) response: HttpResponse,
}

#[async_trait]
impl AsyncHttpClient for AsyncFakeHttpClient {
    async fn post_json(
        &mut self,
        _request: HttpRequest,
    ) -> Result<HttpResponse, HttpClientFailure> {
        Ok(self.response.clone())
    }
}
