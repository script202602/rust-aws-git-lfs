use lambda_http::{run, service_fn, tracing, Error};
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let config = aws_config::load_from_env().await;
    let s3_client = aws_sdk_s3::Client::new(&config);
    let bucket = std::env::var("S3_BUCKET").expect("S3_BUCKET must be set");

    run(service_fn(|event| {
        http_handler::function_handler(event, &s3_client, &bucket)
    }))
    .await
}
