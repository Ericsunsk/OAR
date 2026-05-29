use oar_lark_adapter::{
    AsyncFeishuOkrRead, FeishuOkrCycleListRequest, FeishuOkrCycleObjectivesListRequest,
    FeishuOkrObjectiveKeyResultsListRequest, FeishuOkrReadClient, OkrReadCycle, OkrReadCyclesPage,
    OkrReadKeyResultsPage, OkrReadObjectivesPage, OkrUserIdType, ReqwestAsyncHttpClient,
    SecretString,
};

use super::summary::{compact_text, finalize_summary, truncate_chars};

const MY_OKR_CYCLE_DETAIL_LIMIT: usize = 3;
const MY_OKR_OBJECTIVE_DETAIL_LIMIT: usize = 8;
const MY_OKR_TITLE_LIMIT: usize = 3;

pub(super) async fn read_my_okr_summary(
    okr_client: &mut FeishuOkrReadClient<ReqwestAsyncHttpClient>,
    access_token: SecretString,
    lark_open_id: &str,
) -> Result<String, oar_lark_adapter::FeishuOkrReadError> {
    let response = okr_client
        .list_cycles(FeishuOkrCycleListRequest {
            user_access_token: access_token.clone(),
            user_id_type: OkrUserIdType::OpenId,
            user_id: lark_open_id.to_string(),
            page_size: Some(100),
            page_token: None,
            lang: None,
        })
        .await?;
    let Some(data) = response.data else {
        return Ok("工具 feishu.okr.summarize_my_okr｜实时：Feishu 返回空数据。".to_string());
    };
    let cycles_page = OkrReadCyclesPage::from_cycle_list_data(&data);
    if cycles_page.cycles.is_empty() {
        return Ok("工具 feishu.okr.summarize_my_okr｜实时：未读取到 OKR 周期。".to_string());
    }

    let mut cycle_summaries = Vec::new();
    let mut skipped_missing_id = 0_usize;
    for cycle in cycles_page.cycles.iter().take(MY_OKR_CYCLE_DETAIL_LIMIT) {
        let Some(cycle_id) = cycle.cycle_id.as_deref().filter(|id| !id.trim().is_empty()) else {
            skipped_missing_id += 1;
            continue;
        };
        let objectives_response = okr_client
            .list_cycle_objectives(FeishuOkrCycleObjectivesListRequest {
                user_access_token: access_token.clone(),
                user_id_type: OkrUserIdType::OpenId,
                cycle_id: cycle_id.to_string(),
                page_size: Some(100),
                page_token: None,
                lang: None,
            })
            .await?;
        let Some(objectives_data) = objectives_response.data else {
            cycle_summaries.push(format!("{}：详情为空", cycle_label(cycle)));
            continue;
        };
        let objectives_page =
            OkrReadObjectivesPage::from_cycle_objectives_list_data(cycle_id, &objectives_data);
        let mut kr_count = 0_usize;
        for objective in objectives_page
            .objectives
            .iter()
            .take(MY_OKR_OBJECTIVE_DETAIL_LIMIT)
        {
            let Some(objective_id) = objective
                .objective_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
            else {
                continue;
            };
            let krs_response = okr_client
                .list_objective_key_results(FeishuOkrObjectiveKeyResultsListRequest {
                    user_access_token: access_token.clone(),
                    user_id_type: OkrUserIdType::OpenId,
                    objective_id: objective_id.to_string(),
                    page_size: Some(100),
                    page_token: None,
                    lang: None,
                })
                .await?;
            if let Some(krs_data) = krs_response.data {
                let krs_page = OkrReadKeyResultsPage::from_objective_key_results_list_data(
                    objective_id,
                    &krs_data,
                );
                kr_count += krs_page.krs.len();
            }
        }

        let titles = objectives_page
            .objectives
            .iter()
            .filter_map(|objective| objective.content.as_deref())
            .map(compact_text)
            .filter(|value| !value.is_empty())
            .take(MY_OKR_TITLE_LIMIT)
            .map(|title| truncate_chars(&title, 20))
            .collect::<Vec<_>>();
        let title_suffix = if titles.is_empty() {
            String::new()
        } else {
            format!("，示例：{}", titles.join(" / "))
        };
        cycle_summaries.push(format!(
            "{}：{} 个 Objective、{} 个 KR{}",
            cycle_label(cycle),
            objectives_page.objectives.len(),
            kr_count,
            title_suffix
        ));
    }

    if skipped_missing_id > 0 {
        cycle_summaries.push(format!("{} 个周期缺少稳定 ID，已跳过", skipped_missing_id));
    }
    let detail_suffix = if cycles_page.cycles.len() > MY_OKR_CYCLE_DETAIL_LIMIT {
        format!("；仅展开前 {} 个周期详情", MY_OKR_CYCLE_DETAIL_LIMIT)
    } else {
        String::new()
    };

    Ok(finalize_summary(format!(
        "工具 feishu.okr.summarize_my_okr｜实时：读取到 {} 个 OKR 周期；{}{}。",
        cycles_page.cycles.len(),
        cycle_summaries.join("；"),
        detail_suffix
    )))
}

fn cycle_label(cycle: &OkrReadCycle) -> String {
    if let Some(name) = cycle
        .name
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty())
    {
        return truncate_chars(&name, 24);
    }
    let start = cycle
        .start_time
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty());
    let end = cycle
        .end_time
        .as_deref()
        .map(compact_text)
        .filter(|value| !value.is_empty());
    match (start, end) {
        (Some(start), Some(end)) => format!(
            "{} 至 {}",
            truncate_chars(&start, 10),
            truncate_chars(&end, 10)
        ),
        (Some(start), None) => truncate_chars(&start, 24),
        _ => cycle
            .cycle_id
            .as_deref()
            .map(compact_text)
            .filter(|value| !value.is_empty())
            .map(|id| truncate_chars(&id, 24))
            .unwrap_or_else(|| "未命名周期".to_string()),
    }
}
