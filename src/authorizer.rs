use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct AuthResponse {
    #[serde(rename = "isAuthorized")]
    is_authorized: bool,
}

async fn get_github_login(github_api_base: &str, auth: &str) -> Option<String> {
    let result = reqwest::Client::new()
        .get(format!("{github_api_base}/user"))
        .header("Authorization", auth)
        .header("User-Agent", "rust-aws-git-lfs")
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            body["login"].as_str().map(|s| s.to_lowercase())
        }
        Ok(resp) => {
            tracing::warn!(github_status = %resp.status(), "GitHub /user failed");
            None
        }
        Err(e) => {
            tracing::error!(error = %e, "GitHub /user request failed");
            None
        }
    }
}

async fn check_github_permission(
    github_api_base: &str,
    auth: &str,
    owner: &str,
    repo: &str,
) -> bool {
    let result = reqwest::Client::new()
        .get(format!("{github_api_base}/repos/{owner}/{repo}"))
        .header("Authorization", auth)
        .header("User-Agent", "rust-aws-git-lfs")
        .send()
        .await;

    match result {
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
    }
}

fn is_user_allowed(login: &str, allowed_users_env: &str) -> bool {
    let login_lower = login.to_lowercase();
    allowed_users_env
        .split(',')
        .map(|u| u.trim().to_lowercase())
        .any(|u| u == login_lower)
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

    let github_api_base = std::env::var("GITHUB_API_BASE_URL")
        .unwrap_or_else(|_| "https://api.github.com".to_string());

    let allowed_users = std::env::var("ALLOWED_GITHUB_USERS").unwrap_or_default();

    // ALLOWED_GITHUB_USERS が設定されている場合はユーザー名を検証する
    if !allowed_users.is_empty() {
        let login = match get_github_login(&github_api_base, auth).await {
            Some(l) => l,
            None => {
                tracing::warn!("could not retrieve GitHub login");
                return Ok(AuthResponse { is_authorized: false });
            }
        };
        if !is_user_allowed(&login, &allowed_users) {
            tracing::warn!(login, "GitHub user not in allowlist");
            return Ok(AuthResponse { is_authorized: false });
        }
        tracing::info!(login, "GitHub user allowed");
    }

    let is_authorized = check_github_permission(&github_api_base, auth, owner, repo).await;

    Ok(AuthResponse { is_authorized })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn start_mock_server() -> MockServer {
        MockServer::start().await
    }

    #[tokio::test]
    async fn authorized_when_pull_is_true() {
        let server = start_mock_server().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo"))
            .and(header("Authorization", "Bearer valid-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "permissions": { "pull": true, "push": false }
            })))
            .mount(&server)
            .await;

        let result =
            check_github_permission(&server.uri(), "Bearer valid-token", "owner", "repo").await;
        assert!(result);
    }

    #[tokio::test]
    async fn not_authorized_when_pull_is_false() {
        let server = start_mock_server().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "permissions": { "pull": false }
            })))
            .mount(&server)
            .await;

        let result =
            check_github_permission(&server.uri(), "Bearer token", "owner", "repo").await;
        assert!(!result);
    }

    #[tokio::test]
    async fn not_authorized_when_github_returns_401() {
        let server = start_mock_server().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result =
            check_github_permission(&server.uri(), "Bearer bad-token", "owner", "repo").await;
        assert!(!result);
    }

    #[tokio::test]
    async fn not_authorized_when_github_returns_404() {
        let server = start_mock_server().await;
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let result =
            check_github_permission(&server.uri(), "Bearer token", "owner", "repo").await;
        assert!(!result);
    }

    #[test]
    fn allowlist_matching_is_case_insensitive() {
        assert!(is_user_allowed("Alice", "alice,bob"));
        assert!(is_user_allowed("BOB", "alice,bob"));
        assert!(!is_user_allowed("carol", "alice,bob"));
    }

    #[test]
    fn allowlist_trims_whitespace() {
        assert!(is_user_allowed("alice", " alice , bob "));
    }

    #[tokio::test]
    async fn get_github_login_returns_login_on_success() {
        let server = start_mock_server().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("Authorization", "Bearer token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "myuser"
            })))
            .mount(&server)
            .await;

        let login = get_github_login(&server.uri(), "Bearer token").await;
        assert_eq!(login, Some("myuser".to_string()));
    }

    #[tokio::test]
    async fn get_github_login_returns_none_on_error() {
        let server = start_mock_server().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let login = get_github_login(&server.uri(), "Bearer bad").await;
        assert_eq!(login, None);
    }
}
