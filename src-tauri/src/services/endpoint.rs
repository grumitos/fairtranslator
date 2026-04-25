use reqwest::Url;

use crate::error::AppError;

pub fn normalize_https_endpoint_override(
    endpoint: Option<String>,
) -> Result<Option<String>, AppError> {
    let Some(endpoint) = endpoint
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let parsed = Url::parse(&endpoint)
        .map_err(|_| AppError::Api("Custom endpoint must be a valid HTTPS URL".to_string()))?;

    if parsed.scheme() != "https" {
        return Err(AppError::Api(
            "Custom endpoint must use HTTPS before API keys are sent".to_string(),
        ));
    }

    if parsed.host_str().is_none() {
        return Err(AppError::Api(
            "Custom endpoint must include a host".to_string(),
        ));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::Api(
            "Custom endpoint must not include embedded credentials".to_string(),
        ));
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AppError::Api(
            "Custom endpoint must not include query strings or fragments".to_string(),
        ));
    }

    Ok(Some(parsed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_empty_endpoint_override() {
        assert_eq!(
            normalize_https_endpoint_override(Some("   ".into())).unwrap(),
            None
        );
    }

    #[test]
    fn accepts_https_endpoint_override() {
        assert_eq!(
            normalize_https_endpoint_override(Some(
                " https://api.example.com/v1/translate ".into()
            ))
            .unwrap()
            .as_deref(),
            Some("https://api.example.com/v1/translate")
        );
    }

    #[test]
    fn rejects_http_endpoint_override() {
        let error = normalize_https_endpoint_override(Some("http://api.example.com".into()))
            .expect_err("HTTP endpoints must be rejected");
        assert!(error.to_string().contains("HTTPS"));
    }

    #[test]
    fn rejects_endpoint_override_with_credentials() {
        let error =
            normalize_https_endpoint_override(Some("https://user:pass@api.example.com/v1".into()))
                .expect_err("embedded credentials must be rejected");
        assert!(error.to_string().contains("credentials"));
    }

    #[test]
    fn rejects_endpoint_override_with_query() {
        let error =
            normalize_https_endpoint_override(Some("https://api.example.com/v1?key=secret".into()))
                .expect_err("query strings must be rejected");
        assert!(error.to_string().contains("query strings"));
    }
}
