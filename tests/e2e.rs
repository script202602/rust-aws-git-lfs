use aws_sdk_s3::primitives::ByteStream;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::OnceCell;

const LAMBDA_URL: &str = concat!(
    "http://localhost:9000/2015-03-31/functions/",
    env!("CARGO_PKG_NAME"),
    "/invocations"
);
const BUCKET: &str = "test-lfs-bucket";
const LOCALSTACK_URL: &str = "http://localhost:4566";
const EXISTING_OID: &str =
    "4d7af9c6e8b123456789abcdef1234567890abcdef1234567890abcdef12345678";
const EXISTING_SIZE: i64 = 1024;
const MISSING_OID: &str =
    "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
const NEW_OID: &str = "1111111111111111111111111111111111111111111111111111111111111111";

// Static content whose byte length matches EXISTING_SIZE, used for ByteStream::from_static.
static OBJECT_CONTENT: [u8; 1024] = [0u8; 1024];

// ---- Helpers ----

fn make_event(method: &str, path: &str, body: &str) -> Value {
    json!({
        "httpMethod": method,
        "path": path,
        "headers": {"content-type": "application/vnd.git-lfs+json"},
        "multiValueHeaders": {},
        "queryStringParameters": null,
        "multiValueQueryStringParameters": null,
        "pathParameters": null,
        "stageVariables": null,
        "isBase64Encoded": false,
        "requestContext": {
            "accountId": "123456789012",
            "resourceId": "test",
            "stage": "local",
            "requestId": "e2e-test",
            "identity": {"sourceIp": "127.0.0.1"},
            "resourcePath": path,
            "httpMethod": method,
            "apiId": "test"
        },
        "body": body
    })
}

async fn invoke(event: Value) -> Value {
    Client::new()
        .post(LAMBDA_URL)
        .json(&event)
        .send()
        .await
        .expect("Lambda invocation failed — is cargo lambda watch running?")
        .json::<Value>()
        .await
        .expect("Failed to parse Lambda response as JSON")
}

fn status(resp: &Value) -> u16 {
    resp["statusCode"]
        .as_i64()
        .unwrap_or_else(|| panic!("statusCode missing in response: {resp}")) as u16
}

fn body_json(resp: &Value) -> Value {
    serde_json::from_str(resp["body"].as_str().unwrap_or("{}")).unwrap_or(Value::Null)
}

// ---- S3 Setup ----

static S3_SEEDED: OnceCell<()> = OnceCell::const_new();

async fn seed_s3() {
    S3_SEEDED
        .get_or_init(|| async {
            // These env vars are only needed for the test-side S3 client (LocalStack).
            // The Lambda server reads its own env vars set when cargo lambda watch was started.
            std::env::set_var("AWS_ACCESS_KEY_ID", "test");
            std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
            std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");
            std::env::set_var("AWS_ENDPOINT_URL", LOCALSTACK_URL);

            let config = aws_config::load_from_env().await;
            let s3 = aws_sdk_s3::Client::from_conf(
                aws_sdk_s3::config::Builder::from(&config)
                    .force_path_style(true)
                    .build(),
            );

            let _ = s3.create_bucket().bucket(BUCKET).send().await;

            s3.put_object()
                .bucket(BUCKET)
                .key(format!("objects/owner/repo/{EXISTING_OID}"))
                .body(ByteStream::from_static(&OBJECT_CONTENT))
                .send()
                .await
                .expect("Failed to seed test object in LocalStack");
        })
        .await;
}

// ---- Tests: Routing ----

#[tokio::test]
async fn routing_unknown_path_returns_404() {
    let resp = invoke(make_event("POST", "/unknown", "{}")).await;
    assert_eq!(status(&resp), 404);
}

#[tokio::test]
async fn routing_wrong_method_returns_404() {
    let body = r#"{"operation":"upload","objects":[{"oid":"abc","size":1}]}"#;
    let resp = invoke(make_event(
        "GET",
        "/repos/owner/repo/info/lfs/objects/batch",
        body,
    ))
    .await;
    assert_eq!(status(&resp), 404);
}

// ---- Tests: Batch ----

#[tokio::test]
async fn batch_invalid_json_returns_422() {
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/batch",
        "not-json",
    ))
    .await;
    assert_eq!(status(&resp), 422);
}

#[tokio::test]
async fn batch_invalid_operation_returns_422() {
    let body = format!(
        r#"{{"operation":"invalid","objects":[{{"oid":"{EXISTING_OID}","size":1}}]}}"#
    );
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/batch",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 422);
}

#[tokio::test]
async fn batch_upload_new_object_has_upload_and_verify_actions() {
    seed_s3().await;
    let body = format!(r#"{{"operation":"upload","objects":[{{"oid":"{NEW_OID}","size":100}}]}}"#);
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/batch",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 200);
    let b = body_json(&resp);
    assert!(!b["objects"][0]["actions"]["upload"].is_null(), "upload action missing");
    assert!(!b["objects"][0]["actions"]["verify"].is_null(), "verify action missing");
}

#[tokio::test]
async fn batch_upload_existing_object_has_no_actions() {
    seed_s3().await;
    let body = format!(
        r#"{{"operation":"upload","objects":[{{"oid":"{EXISTING_OID}","size":{EXISTING_SIZE}}}]}}"#
    );
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/batch",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 200);
    let b = body_json(&resp);
    assert!(
        b["objects"][0]["actions"].is_null(),
        "existing object should not have actions"
    );
}

#[tokio::test]
async fn batch_download_existing_object_has_download_action() {
    seed_s3().await;
    let body = format!(
        r#"{{"operation":"download","objects":[{{"oid":"{EXISTING_OID}","size":{EXISTING_SIZE}}}]}}"#
    );
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/batch",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 200);
    let b = body_json(&resp);
    assert!(
        !b["objects"][0]["actions"]["download"].is_null(),
        "download action missing"
    );
}

#[tokio::test]
async fn batch_download_missing_object_returns_error_code_404() {
    let body =
        format!(r#"{{"operation":"download","objects":[{{"oid":"{MISSING_OID}","size":100}}]}}"#);
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/batch",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 200);
    let b = body_json(&resp);
    assert_eq!(b["objects"][0]["error"]["code"], 404);
}

// ---- Tests: Verify ----

#[tokio::test]
async fn verify_invalid_json_returns_422() {
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/verify",
        "not-json",
    ))
    .await;
    assert_eq!(status(&resp), 422);
}

#[tokio::test]
async fn verify_correct_object_returns_200() {
    seed_s3().await;
    let body = format!(r#"{{"oid":"{EXISTING_OID}","size":{EXISTING_SIZE}}}"#);
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/verify",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 200);
}

#[tokio::test]
async fn verify_size_mismatch_returns_422() {
    seed_s3().await;
    let body = format!(r#"{{"oid":"{EXISTING_OID}","size":9999}}"#);
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/verify",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 422);
}

#[tokio::test]
async fn verify_missing_object_returns_404() {
    let body = format!(r#"{{"oid":"{MISSING_OID}","size":100}}"#);
    let resp = invoke(make_event(
        "POST",
        "/repos/owner/repo/info/lfs/objects/verify",
        &body,
    ))
    .await;
    assert_eq!(status(&resp), 404);
}
