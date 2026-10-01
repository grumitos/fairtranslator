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

    /// Overrides are trimmed and must be plain HTTPS URLs: no HTTP, embedded
    /// credentials or query strings (they would leak API keys or secrets).
    #[test]
    fn endpoint_overrides_must_be_plain_https() {
        assert_eq!(
            normalize_https_endpoint_override(Some("   ".into())).unwrap(),
            None
        );
        assert_eq!(
            normalize_https_endpoint_override(Some(
                " https://api.example.com/v1/translate ".into()
            ))
            .unwrap()
            .as_deref(),
            Some("https://api.example.com/v1/translate")
        );
        for (endpoint, reason) in [
            ("http://api.example.com", "HTTPS"),
            ("https://user:pass@api.example.com/v1", "credentials"),
            ("https://api.example.com/v1?key=secret", "query strings"),
        ] {
            let error = normalize_https_endpoint_override(Some(endpoint.into()))
                .expect_err(endpoint)
                .to_string();
            assert!(error.contains(reason), "{endpoint}: {error}");
        }
    }
}
