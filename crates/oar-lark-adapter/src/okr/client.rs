use async_trait::async_trait;

use crate::config::FeishuOpenApiConfig;
use crate::oauth::{AsyncHttpClient, HttpClient};

use super::error::FeishuOkrReadError;
use super::request_builder::{
    build_batch_get_okr_request, build_list_cycle_objectives_request, build_list_cycles_request,
    build_list_objective_key_results_request, build_progress_list_request,
    DEFAULT_PROGRESS_PAGE_SIZE,
};
use super::response_parser::{
    map_status_or_parse_batch_get, map_status_or_parse_cycle_list,
    map_status_or_parse_cycle_objectives_list, map_status_or_parse_objective_key_results_list,
    map_status_or_parse_progress_list,
};
use super::types::{
    FeishuOkrBatchGetRequest, FeishuOkrBatchGetResponse, FeishuOkrCycleListRequest,
    FeishuOkrCycleListResponse, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrCycleObjectivesListResponse, FeishuOkrObjectiveKeyResultsListRequest,
    FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrProgressListRequest,
    FeishuOkrProgressListResponse,
};
use super::validation::{
    validate_batch_get_request, validate_page_request, validate_path_id,
    validate_progress_list_request,
};

#[derive(Debug, Clone)]
pub struct FeishuOkrReadClient<H> {
    config: FeishuOpenApiConfig,
    http_client: H,
}

impl<H> FeishuOkrReadClient<H> {
    pub fn new(config: FeishuOpenApiConfig, http_client: H) -> Self {
        Self {
            config,
            http_client,
        }
    }

    pub fn http_client(&self) -> &H {
        &self.http_client
    }
}

impl<H> FeishuOkrReadClient<H>
where
    H: HttpClient,
{
    pub fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
        validate_batch_get_request(&request)?;
        let raw = self
            .http_client
            .send_json(build_batch_get_okr_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_batch_get(raw.status, &raw.body)
    }

    pub fn list_cycles(
        &mut self,
        request: FeishuOkrCycleListRequest,
    ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
        validate_path_id(&request.user_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycles_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_list(raw.status, &raw.body)
    }

    pub fn list_cycle_objectives(
        &mut self,
        request: FeishuOkrCycleObjectivesListRequest,
    ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
        validate_path_id(&request.cycle_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycle_objectives_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_objectives_list(raw.status, &raw.body)
    }

    pub fn list_objective_key_results(
        &mut self,
        request: FeishuOkrObjectiveKeyResultsListRequest,
    ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
        validate_path_id(&request.objective_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_objective_key_results_request(
                &self.config,
                request,
            ))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_objective_key_results_list(raw.status, &raw.body)
    }

    pub fn list_progress(
        &mut self,
        request: FeishuOkrProgressListRequest,
    ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError> {
        validate_progress_list_request(&request, DEFAULT_PROGRESS_PAGE_SIZE)?;
        let raw = self
            .http_client
            .send_json(build_progress_list_request(&self.config, request))
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_progress_list(raw.status, &raw.body)
    }
}

#[async_trait]
pub trait AsyncFeishuOkrRead {
    async fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError>;

    async fn list_cycles(
        &mut self,
        request: FeishuOkrCycleListRequest,
    ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError>;

    async fn list_cycle_objectives(
        &mut self,
        request: FeishuOkrCycleObjectivesListRequest,
    ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError>;

    async fn list_objective_key_results(
        &mut self,
        request: FeishuOkrObjectiveKeyResultsListRequest,
    ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError>;

    async fn list_progress(
        &mut self,
        request: FeishuOkrProgressListRequest,
    ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError>;
}

#[async_trait]
impl<H> AsyncFeishuOkrRead for FeishuOkrReadClient<H>
where
    H: AsyncHttpClient + Send,
{
    async fn batch_get_okrs(
        &mut self,
        request: FeishuOkrBatchGetRequest,
    ) -> Result<FeishuOkrBatchGetResponse, FeishuOkrReadError> {
        validate_batch_get_request(&request)?;
        let raw = self
            .http_client
            .send_json(build_batch_get_okr_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_batch_get(raw.status, &raw.body)
    }

    async fn list_cycles(
        &mut self,
        request: FeishuOkrCycleListRequest,
    ) -> Result<FeishuOkrCycleListResponse, FeishuOkrReadError> {
        validate_path_id(&request.user_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycles_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_list(raw.status, &raw.body)
    }

    async fn list_cycle_objectives(
        &mut self,
        request: FeishuOkrCycleObjectivesListRequest,
    ) -> Result<FeishuOkrCycleObjectivesListResponse, FeishuOkrReadError> {
        validate_path_id(&request.cycle_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_cycle_objectives_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_cycle_objectives_list(raw.status, &raw.body)
    }

    async fn list_objective_key_results(
        &mut self,
        request: FeishuOkrObjectiveKeyResultsListRequest,
    ) -> Result<FeishuOkrObjectiveKeyResultsListResponse, FeishuOkrReadError> {
        validate_path_id(&request.objective_id)?;
        validate_page_request(
            request.page_size,
            request.page_token.as_deref(),
            request.lang.as_deref(),
        )?;
        let raw = self
            .http_client
            .send_json(build_list_objective_key_results_request(
                &self.config,
                request,
            ))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_objective_key_results_list(raw.status, &raw.body)
    }

    async fn list_progress(
        &mut self,
        request: FeishuOkrProgressListRequest,
    ) -> Result<FeishuOkrProgressListResponse, FeishuOkrReadError> {
        validate_progress_list_request(&request, DEFAULT_PROGRESS_PAGE_SIZE)?;
        let raw = self
            .http_client
            .send_json(build_progress_list_request(&self.config, request))
            .await
            .map_err(FeishuOkrReadError::from)?;
        map_status_or_parse_progress_list(raw.status, &raw.body)
    }
}
