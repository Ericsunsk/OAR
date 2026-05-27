use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use oar_core::lark::auth::types::FeishuAuthRefreshRequest;

use crate::credentials::StaticFeishuAppCredentialProvider;
use crate::crypto::GrantTimeSource;
use crate::material::{
    AesGcmKeyResolver, FeishuGrantMaterialStore, FeishuStoredRefreshMaterialProvider,
    StoredFeishuGrantMaterial,
};
use crate::oauth::{
    AsyncFeishuRefreshMaterialProvider, AsyncHttpClient, FeishuGrantEncryptionInput,
    FeishuGrantEnvelope, FeishuRefreshMaterial, FeishuRefreshMaterialProvider, HttpClient,
    HttpClientFailure, HttpRequest, HttpResponse,
};
use crate::redaction::SecretString;
use crate::FeishuGrantEncryptor;

use super::common::{
    assert_no_secret, sample_envelope, sample_material, stored_material_from_plaintext,
    ACCESS_TOKEN, CLIENT_SECRET, REFRESH_TOKEN,
};

pub(crate) struct FakeMaterialProvider;

impl FeishuRefreshMaterialProvider for FakeMaterialProvider {
    type Error = ();

    fn refresh_material(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error> {
        Ok(sample_material())
    }
}

pub(crate) struct FakeEncryptor;

impl FeishuGrantEncryptor for FakeEncryptor {
    type Error = ();

    fn encrypt(
        &mut self,
        input: FeishuGrantEncryptionInput,
    ) -> Result<FeishuGrantEnvelope, Self::Error> {
        assert_eq!(input.access_token.expose_secret(), ACCESS_TOKEN);
        assert_eq!(input.refresh_token.expose_secret(), REFRESH_TOKEN);
        assert_eq!(input.expected_fingerprint, "fp-prev");
        Ok(sample_envelope())
    }
}

pub(crate) struct FailingMaterialProvider;

impl FeishuRefreshMaterialProvider for FailingMaterialProvider {
    type Error = &'static str;

    fn refresh_material(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error> {
        Err("material provider failed with secret-sensitive payload")
    }
}

pub(crate) struct AsyncFailingMaterialProvider;

#[async_trait(?Send)]
impl AsyncFeishuRefreshMaterialProvider for AsyncFailingMaterialProvider {
    type Error = &'static str;

    async fn refresh_material(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<FeishuRefreshMaterial, Self::Error> {
        Err("material provider failed with secret-sensitive payload")
    }
}

#[derive(Clone)]
pub(crate) struct FakeHttpClient {
    response: Option<HttpResponse>,
    error: Option<HttpClientFailure>,
    pub(crate) requests: Vec<HttpRequest>,
}

impl FakeHttpClient {
    pub(crate) fn from_response(response: HttpResponse) -> Self {
        Self {
            response: Some(response),
            error: None,
            requests: Vec::new(),
        }
    }

    pub(crate) fn from_error(error: HttpClientFailure) -> Self {
        Self {
            response: None,
            error: Some(error),
            requests: Vec::new(),
        }
    }
}

impl HttpClient for FakeHttpClient {
    fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        self.requests.push(request);
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self
            .response
            .clone()
            .expect("fake http client needs response or error"))
    }
}

#[derive(Clone)]
pub(crate) struct AsyncFakeHttpClient {
    response: HttpResponse,
}

impl AsyncFakeHttpClient {
    pub(crate) fn from_response(response: HttpResponse) -> Self {
        Self { response }
    }
}

#[async_trait(?Send)]
impl AsyncHttpClient for AsyncFakeHttpClient {
    async fn post_json(&mut self, request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        assert_eq!(request.body["client_id"], "cli_test");
        assert_eq!(request.body["client_secret"], CLIENT_SECRET);
        assert_eq!(request.body["refresh_token"], "urt-stored-renewal");
        assert_eq!(request.body["scope"], "offline_access auth:user.id:read");
        let debug = format!("{request:?}");
        assert_no_secret(&debug);
        assert!(!debug.contains("urt-stored-renewal"));
        Ok(self.response.clone())
    }
}

#[derive(Clone)]
pub(crate) struct CountingHttpClient {
    sent_requests: Rc<RefCell<usize>>,
}

impl CountingHttpClient {
    pub(crate) fn new(sent_requests: Rc<RefCell<usize>>) -> Self {
        Self { sent_requests }
    }
}

impl HttpClient for CountingHttpClient {
    fn post_json(&mut self, _request: HttpRequest) -> Result<HttpResponse, HttpClientFailure> {
        *self.sent_requests.borrow_mut() += 1;
        Err(HttpClientFailure::Transport)
    }
}

#[derive(Clone)]
pub(crate) struct CountingAsyncHttpClient {
    sent_requests: Rc<RefCell<usize>>,
}

impl CountingAsyncHttpClient {
    pub(crate) fn new(sent_requests: Rc<RefCell<usize>>) -> Self {
        Self { sent_requests }
    }
}

#[async_trait(?Send)]
impl AsyncHttpClient for CountingAsyncHttpClient {
    async fn post_json(
        &mut self,
        _request: HttpRequest,
    ) -> Result<HttpResponse, HttpClientFailure> {
        *self.sent_requests.borrow_mut() += 1;
        Err(HttpClientFailure::Transport)
    }
}

#[derive(Clone)]
pub(crate) struct OneRowStore(pub(crate) StoredFeishuGrantMaterial);

impl FeishuGrantMaterialStore for OneRowStore {
    type Error = ();

    fn load(
        &mut self,
        _request: &FeishuAuthRefreshRequest,
    ) -> Result<StoredFeishuGrantMaterial, Self::Error> {
        Ok(self.0.clone())
    }
}

#[derive(Clone)]
pub(crate) struct FixedKeyResolver {
    key: [u8; 32],
}

impl FixedKeyResolver {
    pub(crate) fn new(key: [u8; 32]) -> Self {
        Self { key }
    }
}

impl AesGcmKeyResolver for FixedKeyResolver {
    type Error = ();

    fn key_for(&mut self, _key_id: &str) -> Result<[u8; 32], Self::Error> {
        Ok(self.key)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FixedClock {
    pub(crate) now_ms: u64,
}

impl GrantTimeSource for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

pub(crate) fn stored_provider(
    key: [u8; 32],
    stored_refresh_token: &str,
) -> FeishuStoredRefreshMaterialProvider<
    OneRowStore,
    FixedKeyResolver,
    StaticFeishuAppCredentialProvider,
> {
    FeishuStoredRefreshMaterialProvider::new(
        OneRowStore(stored_material_from_plaintext(key, stored_refresh_token)),
        FixedKeyResolver::new(key),
        StaticFeishuAppCredentialProvider::new("cli_test", SecretString::new(CLIENT_SECRET)),
    )
}
