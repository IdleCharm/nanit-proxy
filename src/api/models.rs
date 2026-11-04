use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct LoginPayload<'a> {
    pub email: &'a str,
    pub password: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa_code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token: Option<String>,
    pub mfa_token: Option<String>,
    pub channel: Option<String>,
    pub phone_suffix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RefreshPayload<'a> {
    pub refresh_token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Baby {
    pub uid: String,
    pub name: Option<String>,
    #[serde(rename = "camera_uid")]
    pub camera_uid: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BabiesResponse {
    pub babies: Vec<Baby>,
}
