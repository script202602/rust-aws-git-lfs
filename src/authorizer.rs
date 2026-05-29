use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct AuthResponse {
    #[serde(rename = "isAuthorized")]
    is_authorized: bool,
}

async fn handler(event: LambdaEvent<Value>) -> Result<AuthResponse, Error> {
    let headers = &event.payload["headers"];
    let auth = headers["authorization"]
        .as_str()
        .or_else(|| headers["Authorization"].as_str())
        .unwrap_or("");

    if auth.is_empty() {
        tracing::warn!("no Authorization header");
        return Ok(AuthResponse { is_authorized: false });
    }

    let path = event.payload["rawPath"].as_str().unwrap_or("");
    let parts: Vec<&str> = path.trim_start_matches('/').splitn(3, '/').collect();
    if parts.len() < 2 {
        tracing::warn!(path, "cannot parse owner/repo");
        return Ok(AuthResponse { is_authorized: false });
    }
    let (owner, repo) = (parts[0], parts[1]);

    let result = reqwest::Client::new()
        .get(format!("https://api.github.com/repos/{owner}/{repo}"))
        .header("Authorization", auth)
        .header("User-Agent", "rust-aws-git-lfs")
        .send()
        .await;

    let is_authorized = match result {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                tracing::warn!(github_status = %status, "GitHub auth failed");
                false
            } else {
                let body: Value = resp.json().await.unwrap_or(Value::Null);
                let pull = body["permissions"]["pull"].as_bool().unwrap_or(false);
                tracing::info!(github_status = %status, pull, "GitHub authorizer");
                pull
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "GitHub request failed");
            false
        }
    };

    Ok(AuthResponse { is_authorized })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
