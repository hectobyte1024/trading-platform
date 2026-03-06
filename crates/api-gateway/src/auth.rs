use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebAuthnRegisterInitRequest {
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebAuthnRegisterInitResponse {
    pub challenge: String,
    pub rp: RelyingParty,
    pub user: WebAuthnUser,
    pub pub_key_cred_params: Vec<PubKeyCredParam>,
    pub timeout: u64,
    pub attestation: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelyingParty {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebAuthnUser {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PubKeyCredParam {
    pub r#type: String,
    pub alg: i32,
}

pub async fn login(
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Mock authentication - accepts any email/password for demo
    // TODO: Integrate with auth-service for real authentication
    
    let user = UserInfo {
        id: uuid::Uuid::new_v4().to_string(),
        email: payload.email.clone(),
        name: payload.email.split('@').next().unwrap_or("User").to_string(),
        role: "trader".to_string(),
    };

    let response = LoginResponse {
        access_token: format!("mock_access_token_{}", uuid::Uuid::new_v4()),
        refresh_token: format!("mock_refresh_token_{}", uuid::Uuid::new_v4()),
        user,
    };

    Ok(Json(response))
}

pub async fn register(
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Mock registration - accepts any user for demo
    // TODO: Integrate with auth-service and database
    
    let user = UserInfo {
        id: uuid::Uuid::new_v4().to_string(),
        email: payload.email,
        name: payload.name,
        role: "trader".to_string(),
    };

    let response = LoginResponse {
        access_token: format!("mock_access_token_{}", uuid::Uuid::new_v4()),
        refresh_token: format!("mock_refresh_token_{}", uuid::Uuid::new_v4()),
        user,
    };

    Ok(Json(response))
}

pub async fn logout() -> Result<StatusCode, StatusCode> {
    // Mock logout - always succeeds
    // TODO: Integrate with auth-service to revoke tokens
    Ok(StatusCode::OK)
}

pub async fn refresh() -> Result<Json<LoginResponse>, StatusCode> {
    // Mock token refresh
    // TODO: Integrate with auth-service for real token refresh
    
    let user = UserInfo {
        id: uuid::Uuid::new_v4().to_string(),
        email: "user@example.com".to_string(),
        name: "User".to_string(),
        role: "trader".to_string(),
    };

    let response = LoginResponse {
        access_token: format!("mock_access_token_{}", uuid::Uuid::new_v4()),
        refresh_token: format!("mock_refresh_token_{}", uuid::Uuid::new_v4()),
        user,
    };

    Ok(Json(response))
}

pub async fn webauthn_register_init(
    Json(payload): Json<WebAuthnRegisterInitRequest>,
) -> Result<Json<WebAuthnRegisterInitResponse>, StatusCode> {
    // Mock WebAuthn registration initialization
    // TODO: Integrate with auth-service WebAuthn components
    
    let response = WebAuthnRegisterInitResponse {
        challenge: general_purpose::URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes()),
        rp: RelyingParty {
            name: "Trading Platform".to_string(),
            id: "localhost".to_string(),
        },
        user: WebAuthnUser {
            id: payload.user_id.clone(),
            name: format!("user_{}", payload.user_id),
            display_name: format!("User {}", payload.user_id),
        },
        pub_key_cred_params: vec![
            PubKeyCredParam {
                r#type: "public-key".to_string(),
                alg: -7, // ES256
            },
            PubKeyCredParam {
                r#type: "public-key".to_string(),
                alg: -257, // RS256
            },
        ],
        timeout: 60000,
        attestation: "none".to_string(),
    };

    Ok(Json(response))
}

pub async fn webauthn_register_complete() -> Result<StatusCode, StatusCode> {
    // Mock WebAuthn registration completion
    // TODO: Integrate with auth-service to store credentials
    Ok(StatusCode::OK)
}

pub async fn webauthn_auth_init() -> Result<Json<serde_json::Value>, StatusCode> {
    // Mock WebAuthn authentication initialization
    // TODO: Integrate with auth-service WebAuthn components
    
    let response = serde_json::json!({
        "challenge": general_purpose::URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes()),
        "timeout": 60000,
        "rpId": "localhost",
        "allowCredentials": [],
    });

    Ok(Json(response))
}

pub async fn webauthn_auth_complete() -> Result<Json<LoginResponse>, StatusCode> {
    // Mock WebAuthn authentication completion
    // TODO: Integrate with auth-service WebAuthn verification
    
    let user = UserInfo {
        id: uuid::Uuid::new_v4().to_string(),
        email: "webauthn@example.com".to_string(),
        name: "WebAuthn User".to_string(),
        role: "trader".to_string(),
    };

    let response = LoginResponse {
        access_token: format!("mock_access_token_{}", uuid::Uuid::new_v4()),
        refresh_token: format!("mock_refresh_token_{}", uuid::Uuid::new_v4()),
        user,
    };

    Ok(Json(response))
}
