# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Prerequisites

- Rust (via rustup)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html) — required for building, local testing, and deploying
- [cfn-lint](https://github.com/aws-cloudformation/cfn-lint) (`pipx install cfn-lint`) — required for validating CloudFormation templates

## Commands

```bash
# Build for production
cargo lambda build --release

# Build for development
cargo lambda build

# Run unit tests
cargo test

# Run a single test
cargo test test_generic_http_handler

# Start local Lambda server (hot-reload on file changes)
cargo lambda watch

# Invoke the local server with a pre-defined AWS event payload
cargo lambda invoke --data-example apigw-request

# Invoke with a custom JSON payload
cargo lambda invoke --data-file ./data.json

# Deploy to AWS (creates IAM role + Lambda function)
cargo lambda deploy
```

## CloudFormation

```bash
# Validate the CloudFormation template before deploying
cfn-lint cloudformation/template.yaml

# Deploy (first time)
aws cloudformation deploy \
  --template-file cloudformation/template.yaml \
  --stack-name rust-aws-lfs \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameter-overrides \
    ArtifactsBucketName=<bucket> \
    BudgetAlertEmail=<email>

# Deploy with optional parameters
aws cloudformation deploy \
  --parameter-overrides \
    LogRetentionDays=30 \
    CloudFrontGeoRestrictionLocations="JP,US" \
    LambdaMaxConcurrency=50 \
    MonthlyBudgetLimit=10
```

### CloudFormation パラメータ

| パラメータ | デフォルト | 説明 |
|---|---|---|
| `LogRetentionDays` | 30 | CloudWatch Logs 保持日数 |
| `CloudFrontGeoRestrictionLocations` | `""` | CloudFront ホワイトリスト国コード（例: `JP,US`）。空で制限なし |
| `LambdaMaxConcurrency` | -1 | Lambda 同時実行数上限。-1 で無制限。`ApiThrottlingBurstLimit` 以上を推奨 |
| `ApiThrottlingRateLimit` | 10 | API Gateway 持続リクエスト上限（req/s） |
| `ApiThrottlingBurstLimit` | 50 | API Gateway バーストリクエスト上限 |
| `MonthlyBudgetLimit` | 10 | 月次コスト予算上限（USD） |
| `BudgetAlertEmail` | 必須 | コスト通知先メールアドレス |

### IDE の YAML 警告について

VS Code の YAML 言語サーバーが `!If` / `!Sub` / `!Ref` 等の CloudFormation 組み込み関数タグを解釈できず「Unresolved tag」エラーを表示するが、いずれも偽陽性。`cfn-lint` が正式なバリデーターとして使用すること。

## Terraform

```bash
cd terraform

# 初期化
terraform init

# プレビュー
terraform plan -var-file=terraform.tfvars

# デプロイ
terraform apply -var-file=terraform.tfvars

# 既存リソースのインポート（Lambda が先に実行済みの場合）
terraform import aws_cloudwatch_log_group.main /aws/lambda/rust-aws-lfs
terraform import aws_cloudwatch_log_group.authorizer /aws/lambda/rust-aws-lfs-authorizer
```

### Terraform 変数

`terraform/terraform.tfvars.example` をコピーして `terraform.tfvars` を作成する。

| 変数 | デフォルト | 説明 |
|---|---|---|
| `log_retention_days` | 30 | CloudWatch Logs 保持日数 |
| `cloudfront_geo_restriction_locations` | `[]` | CloudFront ホワイトリスト国コード（例: `["JP", "US"]`）。空で制限なし |
| `lambda_reserved_concurrency` | -1 | Lambda 同時実行数上限。`api_throttling_burst_limit` 以上を推奨 |
| `api_throttling_rate_limit` | 10 | API Gateway 持続リクエスト上限（req/s） |
| `api_throttling_burst_limit` | 50 | API Gateway バーストリクエスト上限 |
| `monthly_budget_limit` | 10 | 月次コスト予算上限（USD） |
| `budget_alert_email` | 必須 | コスト通知先メールアドレス |
| `cloudfront_private_key_pem` | 必須 | CloudFront Signed URL 用 RSA 秘密鍵（PEM）。`terraform.tfvars` にのみ記載しコミット禁止 |
| `cloudfront_public_key_pem` | 必須 | CloudFront にアップロードする RSA 公開鍵（PEM） |

## コスト保護

以下の対策が実装済み：

| 対策 | 実装箇所 |
|---|---|
| S3 Block Public Access（全項目 ON） | `aws_s3_bucket_public_access_block` / `PublicAccessBlockConfiguration` |
| CloudFront + OAC（S3 直接公開なし） | `aws_cloudfront_origin_access_control` / `CloudFrontOAC` |
| CloudFront Signed URL（コンテンツ保護） | `TrustedKeyGroups` 設定済み |
| API Gateway スロットリング | `default_route_settings` |
| Lambda 同時実行数制限（オプション） | `reserved_concurrent_executions` |
| CloudWatch Logs 保持期間設定 | `aws_cloudwatch_log_group` / `AWS::Logs::LogGroup` |
| AWS Budgets（実績 80%・100%、予測 80%） | `aws_budgets_budget` / `AWS::Budgets::Budget` |
| CloudFront 地理的制限（オプション） | `geo_restriction` / `GeoRestriction` |

**Cost Anomaly Detection（異常検知）について：** AWS アカウントにはデフォルトで `Default-Services-Monitor`（DIMENSIONAL/SERVICE）が作成済み。このプロジェクトのテンプレートでは管理しない。通知サブスクリプションが必要な場合は AWS コンソールの Cost Anomaly Detection から手動で設定する。

## Architecture

This is a minimal AWS Lambda HTTP handler using the `lambda_http` crate from the [aws-lambda-rust-runtime](https://github.com/awslabs/aws-lambda-rust-runtime) project.

- `src/main.rs` — entry point; initializes tracing and registers the handler with the Lambda runtime via `service_fn`
- `src/http_handler.rs` — contains `function_handler`, the core request/response logic, and its unit tests

The handler receives a `lambda_http::Request`, extracts query parameters, and returns a `Response<Body>`. All new business logic belongs in `http_handler.rs` (or additional modules imported from there). The `main.rs` wiring should not need to change.

HTTP integration tests use `cargo lambda watch` + `cargo lambda invoke` or direct `curl` against `localhost:9000`.
