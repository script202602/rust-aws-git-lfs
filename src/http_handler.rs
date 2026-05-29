use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client as S3Client;
use lambda_http::{Body, Error, Request, Response};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const PRESIGNED_URL_EXPIRY_SECS: u64 = 3600;
const LFS_CONTENT_TYPE: &str = "application/vnd.git-lfs+json";

#[derive(Deserialize)]
struct BatchRequest {
    operation: String,
    objects: Vec<LfsObject>,
    #[serde(default = "default_hash_algo")]
    hash_algo: String,
}

#[derive(Deserialize)]
struct LfsObject {
    oid: String,
    size: i64,
}

#[derive(Deserialize)]
struct VerifyRequest {
    oid: String,
    size: i64,
}

fn default_hash_algo() -> String {
    "sha256".to_string()
}

fn lfs_error(status: u16, message: &str) -> Result<Response<Body>, Error> {
    let body = json!({ "message": message });
    Ok(Response::builder()
        .status(status)
        .header("content-type", LFS_CONTENT_TYPE)
        .body(body.to_string().into())
        .map_err(Box::new)?)
}

fn parse_lfs_path(path: &str) -> Option<(String, String, String)> {
    // Expected: /repos/{owner}/{repo}/info/lfs/objects/{endpoint}
    // Allow an optional leading stage segment (e.g. /prod/repos/... or /local/repos/...)
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let start = parts.iter().position(|&s| s == "repos")?;
    let parts = &parts[start..];
    if parts.len() == 7
        && parts[3] == "info"
        && parts[4] == "lfs"
        && parts[5] == "objects"
    {
        Some((
            parts[1].to_string(),
            parts[2].to_string(),
            parts[6].to_string(),
        ))
    } else {
        None
    }
}

fn extract_body(body: &Body) -> String {
    match body {
        Body::Text(s) => s.clone(),
        Body::Binary(b) => String::from_utf8(b.clone()).unwrap_or_default(),
        Body::Empty => String::new(),
        _ => String::new(),
    }
}

fn presigning_config() -> PresigningConfig {
    PresigningConfig::expires_in(Duration::from_secs(PRESIGNED_URL_EXPIRY_SECS))
        .expect("3600s is within the valid presigning duration range")
}

async fn handle_batch(
    event: Request,
    s3_client: &S3Client,
    bucket: &str,
    owner: &str,
    repo: &str,
) -> Result<Response<Body>, Error> {
    let body_str = extract_body(event.body());
    let req: BatchRequest = match serde_json::from_str(&body_str) {
        Ok(r) => r,
        Err(_) => return lfs_error(422, "Invalid request body"),
    };

    if req.operation != "upload" && req.operation != "download" {
        return lfs_error(422, "Invalid operation: must be 'upload' or 'download'");
    }

    let base_url = std::env::var("LFS_BASE_URL").unwrap_or_default();
    let verify_href = format!("{base_url}/repos/{owner}/{repo}/info/lfs/objects/verify");
    let mut result_objects = Vec::new();

    for obj in &req.objects {
        let s3_key = format!("objects/{owner}/{repo}/{}", obj.oid);

        match req.operation.as_str() {
            "upload" => {
                let exists = s3_client
                    .head_object()
                    .bucket(bucket)
                    .key(&s3_key)
                    .send()
                    .await
                    .is_ok();

                if exists {
                    result_objects.push(json!({
                        "oid": obj.oid,
                        "size": obj.size,
                        "authenticated": true
                    }));
                } else {
                    let presigned = s3_client
                        .put_object()
                        .bucket(bucket)
                        .key(&s3_key)
                        .presigned(presigning_config())
                        .await?;

                    result_objects.push(json!({
                        "oid": obj.oid,
                        "size": obj.size,
                        "authenticated": true,
                        "actions": {
                            "upload": {
                                "href": presigned.uri().to_string(),
                                "expires_in": PRESIGNED_URL_EXPIRY_SECS
                            },
                            "verify": {
                                "href": verify_href,
                                "expires_in": PRESIGNED_URL_EXPIRY_SECS
                            }
                        }
                    }));
                }
            }
            "download" => {
                match s3_client
                    .head_object()
                    .bucket(bucket)
                    .key(&s3_key)
                    .send()
                    .await
                {
                    Ok(_) => {
                        let presigned = s3_client
                            .get_object()
                            .bucket(bucket)
                            .key(&s3_key)
                            .presigned(presigning_config())
                            .await?;

                        result_objects.push(json!({
                            "oid": obj.oid,
                            "size": obj.size,
                            "authenticated": true,
                            "actions": {
                                "download": {
                                    "href": presigned.uri().to_string(),
                                    "expires_in": PRESIGNED_URL_EXPIRY_SECS
                                }
                            }
                        }));
                    }
                    Err(_) => {
                        result_objects.push(json!({
                            "oid": obj.oid,
                            "size": obj.size,
                            "error": {
                                "code": 404,
                                "message": "Object not found"
                            }
                        }));
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    let response_body = json!({
        "transfer": "basic",
        "objects": result_objects,
        "hash_algo": req.hash_algo
    });

    Ok(Response::builder()
        .status(200)
        .header("content-type", LFS_CONTENT_TYPE)
        .body(response_body.to_string().into())
        .map_err(Box::new)?)
}

async fn handle_verify(
    event: Request,
    s3_client: &S3Client,
    bucket: &str,
    owner: &str,
    repo: &str,
) -> Result<Response<Body>, Error> {
    let body_str = extract_body(event.body());
    let req: VerifyRequest = match serde_json::from_str(&body_str) {
        Ok(r) => r,
        Err(_) => return lfs_error(422, "Invalid request body"),
    };

    let s3_key = format!("objects/{owner}/{repo}/{}", req.oid);

    match s3_client
        .head_object()
        .bucket(bucket)
        .key(&s3_key)
        .send()
        .await
    {
        Ok(head) => {
            let actual_size = head.content_length().unwrap_or(0);
            if actual_size != req.size {
                return lfs_error(422, "Object size mismatch");
            }
            Ok(Response::builder()
                .status(200)
                .header("content-type", LFS_CONTENT_TYPE)
                .body("{}".into())
                .map_err(Box::new)?)
        }
        Err(_) => lfs_error(404, "Object not found"),
    }
}

pub(crate) async fn function_handler(
    event: Request,
    s3_client: &S3Client,
    bucket: &str,
) -> Result<Response<Body>, Error> {
    let path = event.uri().path().to_string();

    if let Some((owner, repo, endpoint)) = parse_lfs_path(&path) {
        if event.method().as_str() == "POST" {
            match endpoint.as_str() {
                "batch" => return handle_batch(event, s3_client, bucket, &owner, &repo).await,
                "verify" => return handle_verify(event, s3_client, bucket, &owner, &repo).await,
                _ => {}
            }
        }
    }

    lfs_error(404, "Not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_http::http::{Method, Uri};

    fn test_s3_client() -> S3Client {
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .build();
        aws_sdk_s3::Client::from_conf(config)
    }

    fn post_request(uri: &'static str, body: Body) -> Request {
        lambda_http::http::Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static(uri))
            .body(body)
            .unwrap()
    }

    #[test]
    fn test_parse_lfs_path_batch() {
        let result = parse_lfs_path("/repos/myorg/myrepo/info/lfs/objects/batch");
        assert_eq!(
            result,
            Some(("myorg".to_string(), "myrepo".to_string(), "batch".to_string()))
        );
    }

    #[test]
    fn test_parse_lfs_path_verify() {
        let result = parse_lfs_path("/repos/myorg/myrepo/info/lfs/objects/verify");
        assert_eq!(
            result,
            Some(("myorg".to_string(), "myrepo".to_string(), "verify".to_string()))
        );
    }

    #[test]
    fn test_parse_lfs_path_invalid() {
        assert_eq!(parse_lfs_path("/"), None);
        assert_eq!(parse_lfs_path(""), None);
        assert_eq!(parse_lfs_path("/repos/owner/repo"), None);
        assert_eq!(parse_lfs_path("/repos/owner/repo/info/lfs/objects"), None);
        assert_eq!(parse_lfs_path("/other/path"), None);
    }

    #[tokio::test]
    async fn test_unknown_path_returns_404() {
        let s3 = test_s3_client();
        let request = Request::default();
        let response = function_handler(request, &s3, "test-bucket").await.unwrap();
        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn test_wrong_method_returns_404() {
        let s3 = test_s3_client();
        let request = lambda_http::http::Request::builder()
            .method(Method::GET)
            .uri(Uri::from_static("/repos/owner/repo/info/lfs/objects/batch"))
            .body(Body::Empty)
            .unwrap();
        let response = function_handler(request, &s3, "test-bucket").await.unwrap();
        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn test_batch_invalid_json_returns_422() {
        let s3 = test_s3_client();
        let request = post_request(
            "/repos/owner/repo/info/lfs/objects/batch",
            Body::Text("not json".to_string()),
        );
        let response = function_handler(request, &s3, "test-bucket").await.unwrap();
        assert_eq!(response.status(), 422);
    }

    #[tokio::test]
    async fn test_batch_invalid_operation_returns_422() {
        let s3 = test_s3_client();
        let body = r#"{"operation":"delete","objects":[{"oid":"abc","size":100}]}"#;
        let request = post_request(
            "/repos/owner/repo/info/lfs/objects/batch",
            Body::Text(body.to_string()),
        );
        let response = function_handler(request, &s3, "test-bucket").await.unwrap();
        assert_eq!(response.status(), 422);
    }

    #[tokio::test]
    async fn test_verify_invalid_json_returns_422() {
        let s3 = test_s3_client();
        let request = post_request(
            "/repos/owner/repo/info/lfs/objects/verify",
            Body::Text("not json".to_string()),
        );
        let response = function_handler(request, &s3, "test-bucket").await.unwrap();
        assert_eq!(response.status(), 422);
    }
}
