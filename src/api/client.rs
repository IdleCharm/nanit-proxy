use crate::api::models::{
    BabiesResponse, Baby, LoginPayload, LoginResponse, RefreshPayload, RefreshResponse,
};
use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use tracing::{debug, info};

const BASE_URL: &str = "https://api.nanit.com";
const API_VERSION: &str = "1";

#[derive(Clone)]
pub struct NanitApiClient {
    client: reqwest::Client,
}

impl NanitApiClient {
    pub fn new() -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("nanit-api-version", HeaderValue::from_static(API_VERSION));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self { client })
    }

    /// Step 1 of login: Send credentials, trigger MFA challenge
    pub async fn initiate_login(&self, email: &str, password: &str) -> Result<LoginResponse> {
        info!(email, "Initiating Nanit login");
        let payload = LoginPayload {
            email,
            password,
            mfa_token: None,
            mfa_code: None,
            channel: None,
        };

        let res = self
            .client
            .post(format!("{BASE_URL}/login"))
            .json(&payload)
            .send()
            .await
            .context("Failed to send login request")?;

        let status = res.status();
        let body: LoginResponse = res
            .json()
            .await
            .context("Failed to parse login response JSON")?;

        if !status.is_success() && status.as_u16() != 201 && status.as_u16() != 200 {
            bail!("Nanit login returned unexpected status {status}");
        }

        Ok(body)
    }

    /// Step 2 of login: Submit 2FA/MFA verification code
    pub async fn complete_mfa(
        &self,
        email: &str,
        password: &str,
        mfa_token: &str,
        mfa_code: &str,
        channel: &str,
    ) -> Result<LoginResponse> {
        info!("Submitting MFA verification code");
        let payload = LoginPayload {
            email,
            password,
            mfa_token: Some(mfa_token),
            mfa_code: Some(mfa_code),
            channel: Some(channel),
        };

        let res = self
            .client
            .post(format!("{BASE_URL}/login"))
            .json(&payload)
            .send()
            .await
            .context("Failed to send MFA verification request")?;

        let status = res.status();
        if !status.is_success() {
            let error_text = res.text().await.unwrap_or_default();
            bail!("MFA verification failed with status {status}: {error_text}");
        }

        let body: LoginResponse = res
            .json()
            .await
            .context("Failed to parse MFA response JSON")?;

        Ok(body)
    }

    /// Refresh access token using long-lived refresh token
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<RefreshResponse> {
        debug!("Refreshing Nanit access token");
        let payload = RefreshPayload { refresh_token };

        let res = self
            .client
            .post(format!("{BASE_URL}/login/refresh"))
            .json(&payload)
            .send()
            .await
            .context("Failed to send refresh token request")?;

        let status = res.status();
        if !status.is_success() {
            let error_text = res.text().await.unwrap_or_default();
            bail!("Token refresh failed with status {status}: {error_text}");
        }

        let body: RefreshResponse = res
            .json()
            .await
            .context("Failed to parse refresh token response JSON")?;

        Ok(body)
    }

    /// Fetch registered babies and cameras
    pub async fn get_babies(&self, access_token: &str) -> Result<Vec<Baby>> {
        debug!("Fetching registered babies and cameras");
        let res = self
            .client
            .get(format!("{BASE_URL}/babies"))
            .header("Authorization", access_token)
            .send()
            .await
            .context("Failed to fetch babies list")?;

        let status = res.status();
        if !status.is_success() {
            let error_text = res.text().await.unwrap_or_default();
            bail!("Fetching babies failed with status {status}: {error_text}");
        }

        let body: BabiesResponse = res
            .json()
            .await
            .context("Failed to parse babies response JSON")?;

        Ok(body.babies)
    }
}
