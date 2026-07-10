use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub owned_by: Option<String>,
    pub created: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ModelListResponse {
    data: Vec<ModelResponseItem>,
}

#[derive(Debug, Deserialize)]
struct ModelResponseItem {
    id: String,
    #[serde(default)]
    owned_by: Option<String>,
    #[serde(default)]
    created: Option<i64>,
}

pub fn normalize_model_endpoint(endpoint: Option<String>) -> Result<Option<String>, AppError> {
    let Some(endpoint) = endpoint
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let parsed = Url::parse(&endpoint)
        .map_err(|_| AppError::Api("Model endpoint must be a valid HTTPS URL".to_string()))?;

    if parsed.scheme() != "https" {
        return Err(AppError::Api(
            "Model endpoint must use HTTPS before API keys are sent".to_string(),
        ));
    }

    if parsed.host_str().is_none() {
        return Err(AppError::Api(
            "Model endpoint must include a host".to_string(),
        ));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::Api(
            "Model endpoint must not include embedded credentials".to_string(),
        ));
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AppError::Api(
            "Model endpoint must not include query strings or fragments".to_string(),
        ));
    }

    Ok(Some(parsed.to_string()))
}

pub fn derive_models_endpoint(endpoint_or_base: &str) -> Result<String, AppError> {
    let mut url = Url::parse(endpoint_or_base)
        .map_err(|_| AppError::Api("Model endpoint must be a valid HTTPS URL".to_string()))?;
    let path = url.path().trim_end_matches('/');
    let base_path = if let Some(base) = path.strip_suffix("/chat/completions") {
        base
    } else if let Some(base) = path.strip_suffix("/responses") {
        base
    } else if let Some(base) = path.strip_suffix("/messages") {
        base
    } else if let Some(base) = path.strip_suffix("/models") {
        base
    } else {
        path
    };
    let models_path = if base_path.is_empty() {
        "/models".to_string()
    } else {
        format!("{base_path}/models")
    };
    url.set_path(&models_path);
    Ok(url.to_string())
}

pub async fn list_openai_compatible_models(
    client: &Client,
    endpoint_or_base: &str,
    api_key: Option<&str>,
    extra_headers: &[(&str, &str)],
) -> Result<Vec<ProviderModel>, AppError> {
    let models_endpoint = derive_models_endpoint(endpoint_or_base)?;
    let mut request = client.get(models_endpoint);
    if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
        request = request.header("Authorization", format!("Bearer {}", api_key.trim()));
    }
    for (name, value) in extra_headers {
        if !value.trim().is_empty() {
            request = request.header(*name, value.trim());
        }
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(AppError::Api(format!(
            "Model list request failed ({status})"
        )));
    }

    parse_model_list_response(&response.text().await?)
}

pub fn parse_model_list_response(raw: &str) -> Result<Vec<ProviderModel>, AppError> {
    let parsed: ModelListResponse = serde_json::from_str(raw)
        .map_err(|error| AppError::Api(format!("Could not parse provider model list: {error}")))?;
    let mut models = parsed
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| ProviderModel {
            id: model.id,
            owned_by: model.owned_by,
            created: model.created,
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_models_endpoint_from_known_generation_endpoints() {
        assert_eq!(
            derive_models_endpoint("https://api.example.com/v1/chat/completions").unwrap(),
            "https://api.example.com/v1/models"
        );
        assert_eq!(
            derive_models_endpoint("https://api.example.com/v1/responses").unwrap(),
            "https://api.example.com/v1/models"
        );
        assert_eq!(
            derive_models_endpoint("https://api.example.com/v1/messages").unwrap(),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn derives_models_endpoint_from_base_url() {
        assert_eq!(
            derive_models_endpoint("https://opencode.ai/zen/go/v1").unwrap(),
            "https://opencode.ai/zen/go/v1/models"
        );
    }

    #[test]
    fn parses_openai_compatible_model_lists() {
        let models = parse_model_list_response(
            r#"{"object":"list","data":[{"id":"b","created":2,"owned_by":"x"},{"id":"a"}]}"#,
        )
        .unwrap();

        assert_eq!(models[0].id, "a");
        assert_eq!(models[1].id, "b");
        assert_eq!(models[1].owned_by.as_deref(), Some("x"));
        assert_eq!(models[1].created, Some(2));
    }
}
