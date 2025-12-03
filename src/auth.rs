use anyhow::{Context, Result};
use axum::{Router, extract::Query, http::StatusCode, response::Html, routing::get};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use ticks::{AccessToken, Authorization};
use tokio::sync::Mutex;

const REDIRECT_URI: &str = "http://localhost:8080/callback";

#[derive(Debug, Deserialize, Clone)]
pub struct AuthCallback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub fn get_client_id() -> Result<(String, String)> {
    let client_id = std::env::var("TICKTICK_CLIENT_ID").context("TICKTICK_CLIENT_ID not set")?;
    let client_secret =
        std::env::var("TICKTICK_CLIENT_SECRET").context("TICKTICK_CLIENT_SECRET not set")?;
    Ok((client_id, client_secret))
}

fn get_token_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Could not determine home directory");
    path.push(".subroutine");
    std::fs::create_dir_all(&path).ok();
    path.push("token.json");
    path
}

pub fn load_cached_token() -> Result<AccessToken> {
    let path = get_token_cache_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read token cache from {:?}", path))?;
        let token: AccessToken =
            serde_json::from_str(&content).with_context(|| "Failed to parse token cache JSON")?;
        Ok(token)
    } else {
        Err(anyhow::anyhow!("Token cache does not exist"))
    }
}

pub fn save_token_cache(token: &AccessToken) -> Result<()> {
    let path = get_token_cache_path();
    let json = serde_json::to_string(token)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub async fn perform_authorization(
    client_id: String,
    client_secret: String,
) -> Result<AccessToken> {
    let redirect_uri = REDIRECT_URI.to_string();
    let auth_result = Authorization::begin_auth(client_id.clone(), redirect_uri.clone());
    let awaiting_auth =
        auth_result.map_err(|e| anyhow::anyhow!("Failed to begin authorization: {:?}", e))?;
    let auth_code = Arc::new(Mutex::new(None::<String>));
    let auth_state = Arc::new(Mutex::new(None::<String>));

    let auth_code_clone = auth_code.clone();
    let auth_state_clone = auth_state.clone();

    let callback_handler = move |Query(callback): Query<AuthCallback>| {
        let code_storage = auth_code_clone.clone();
        let state_storage = auth_state_clone.clone();
        async move {
            if let Some(error) = callback.error {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(format!(
                        "<html><body><h1>✗ Authorization Error</h1><p>{}</p></body></html>",
                        error
                    )),
                );
            }
            if let Some(code) = callback.code {
                *code_storage.lock().await = Some(code);
                if let Some(state) = callback.state {
                    *state_storage.lock().await = Some(state);
                }
                (
                    StatusCode::OK,
                    Html("<html><body><h1>✓ Authorization Successful!</h1><p>You can now return to your terminal.</p></body></html>".to_string()),
                )
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    Html("<html><body><h1>✗ No Authorization Code</h1><p>No code received in callback.</p></body></html>".to_string()),
                )
            }
        }
    };
    let app = Router::new().route("/callback", get(callback_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let auth_url = awaiting_auth.get_url().to_string();
    let _ = open::that(&auth_url);
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(300); // 5 minute timeout
    let received_code = loop {
        if let Some(code) = auth_code.lock().await.take() {
            break code;
        }
        if start.elapsed() > timeout {
            return Err(anyhow::anyhow!("Authorization timed out."));
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    };
    let received_state = auth_state.lock().await.take().unwrap_or_default();
    let token = awaiting_auth
        .finish_auth(client_secret, received_code, received_state)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to finish authorization: {:?}", e))?;
    save_token_cache(&token).map_err(|e| anyhow::anyhow!("Failed to save token cache: {:?}", e))?;
    Ok(token)
}

pub async fn get_access_token(client_id: String, client_secret: String) -> Result<AccessToken> {
    match load_cached_token() {
        Ok(token) => Ok(token),
        Err(_) => {
            println!("No valid cached token found, starting authorization flow...");
            perform_authorization(client_id, client_secret).await
        }
    }
}
