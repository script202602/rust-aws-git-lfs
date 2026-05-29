use lambda_http::{run, service_fn, tracing, Error};
mod http_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let config = aws_config::load_from_env().await;
    // force_path_style is required when AWS_ENDPOINT_URL is set (e.g. LocalStack)
    let use_path_style = std::env::var("AWS_ENDPOINT_URL").is_ok();
    let s3_config = aws_sdk_s3::config::Builder::from(&config)
        .force_path_style(use_path_style)
        .build();
    let s3_client = aws_sdk_s3::Client::from_conf(s3_config);
    let bucket = std::env::var("S3_BUCKET").expect("S3_BUCKET must be set");

    run(service_fn(|event| {
        http_handler::function_handler(event, &s3_client, &bucket)
    }))
    .await
}
