terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    archive = {
      source  = "hashicorp/archive"
      version = "~> 2.0"
    }
  }
}

provider "aws" {
  region = var.region
}

data "aws_caller_identity" "current" {}

# ── S3 ────────────────────────────────────────────────────────────────────────

resource "aws_s3_bucket" "lfs" {
  bucket = "lfs-${data.aws_caller_identity.current.account_id}-${var.region}"
}

resource "aws_s3_bucket_public_access_block" "lfs" {
  bucket                  = aws_s3_bucket.lfs.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_policy" "lfs" {
  bucket     = aws_s3_bucket.lfs.id
  depends_on = [aws_s3_bucket_public_access_block.lfs]
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Sid       = "AllowCloudFrontOAC"
      Effect    = "Allow"
      Principal = { Service = "cloudfront.amazonaws.com" }
      Action    = "s3:GetObject"
      Resource  = "${aws_s3_bucket.lfs.arn}/*"
      Condition = {
        StringEquals = {
          "AWS:SourceArn" = aws_cloudfront_distribution.lfs.arn
        }
      }
    }]
  })
}

# ── IAM ───────────────────────────────────────────────────────────────────────

resource "aws_iam_role" "main" {
  name = "${var.function_name}-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "main_basic" {
  role       = aws_iam_role.main.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "main_s3" {
  name = "lfs-s3-access"
  role = aws_iam_role.main.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["s3:GetObject", "s3:PutObject"]
      Resource = "${aws_s3_bucket.lfs.arn}/objects/*"
    }]
  })
}

resource "aws_iam_role" "authorizer" {
  name = "${var.function_name}-authorizer-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "lambda.amazonaws.com" }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "authorizer_basic" {
  role       = aws_iam_role.authorizer.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# ── Lambda ────────────────────────────────────────────────────────────────────

# cargo lambda build --release でビルドした bootstrap バイナリを zip 化する
data "archive_file" "main" {
  type        = "zip"
  source_file = "${path.module}/../target/lambda/rust-aws-lfs/bootstrap"
  output_path = "${path.module}/../target/lambda/rust-aws-lfs.zip"
}

data "archive_file" "authorizer" {
  type        = "zip"
  source_file = "${path.module}/../target/lambda/rust-aws-lfs-authorizer/bootstrap"
  output_path = "${path.module}/../target/lambda/rust-aws-lfs-authorizer.zip"
}

resource "aws_lambda_function" "main" {
  function_name                  = var.function_name
  role                           = aws_iam_role.main.arn
  filename                       = data.archive_file.main.output_path
  source_code_hash               = data.archive_file.main.output_base64sha256
  handler                        = "bootstrap"
  runtime                        = "provided.al2023"
  timeout                        = 30
  memory_size                    = 128
  reserved_concurrent_executions = var.lambda_reserved_concurrency

  environment {
    variables = {
      S3_BUCKET               = aws_s3_bucket.lfs.bucket
      CLOUDFRONT_DOMAIN       = aws_cloudfront_distribution.lfs.domain_name
      CLOUDFRONT_KEY_PAIR_ID  = aws_cloudfront_public_key.lfs.id
      CLOUDFRONT_PRIVATE_KEY  = var.cloudfront_private_key_pem
      CLOUDFRONT_URL_TTL_SECS = tostring(var.cloudfront_signed_url_ttl_secs)
    }
  }
}

resource "aws_lambda_function" "authorizer" {
  function_name                  = "${var.function_name}-authorizer"
  role                           = aws_iam_role.authorizer.arn
  filename                       = data.archive_file.authorizer.output_path
  source_code_hash               = data.archive_file.authorizer.output_base64sha256
  handler                        = "bootstrap"
  runtime                        = "provided.al2023"
  timeout                        = 10
  memory_size                    = 128
  reserved_concurrent_executions = var.lambda_reserved_concurrency
}

# ── API Gateway ───────────────────────────────────────────────────────────────

resource "aws_apigatewayv2_api" "main" {
  name          = var.function_name
  protocol_type = "HTTP"
}

resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.main.id
  name        = "$default"
  auto_deploy = true

  default_route_settings {
    throttling_rate_limit  = var.api_throttling_rate_limit
    throttling_burst_limit = var.api_throttling_burst_limit
  }
}

resource "aws_apigatewayv2_integration" "main" {
  api_id                 = aws_apigatewayv2_api.main.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.main.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_authorizer" "github" {
  api_id                            = aws_apigatewayv2_api.main.id
  authorizer_type                   = "REQUEST"
  identity_sources                  = ["$request.header.authorization"]
  name                              = "github-auth"
  authorizer_uri                    = aws_lambda_function.authorizer.invoke_arn
  authorizer_payload_format_version = "2.0"
  enable_simple_responses           = true
  authorizer_result_ttl_in_seconds  = 300
}

resource "aws_apigatewayv2_route" "main" {
  api_id             = aws_apigatewayv2_api.main.id
  route_key          = "ANY /{proxy+}"
  target             = "integrations/${aws_apigatewayv2_integration.main.id}"
  authorization_type = "CUSTOM"
  authorizer_id      = aws_apigatewayv2_authorizer.github.id
}

# ── Lambda 実行権限 ────────────────────────────────────────────────────────────

resource "aws_lambda_permission" "main_apigw" {
  statement_id  = "apigateway-invoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.main.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.main.execution_arn}/*/*/{proxy+}"
}

resource "aws_lambda_permission" "authorizer_apigw" {
  statement_id  = "apigateway-authorizer"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.authorizer.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.main.execution_arn}/authorizers/*"
}

# ── CloudFront ────────────────────────────────────────────────────────────────

resource "aws_cloudfront_origin_access_control" "lfs" {
  name                              = "${var.function_name}-oac"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

resource "aws_cloudfront_public_key" "lfs" {
  name        = "${var.function_name}-cf-pubkey"
  encoded_key = var.cloudfront_public_key_pem

  lifecycle {
    ignore_changes = [encoded_key]
  }
}

resource "aws_cloudfront_key_group" "lfs" {
  name  = "${var.function_name}-key-group"
  items = [aws_cloudfront_public_key.lfs.id]
}

resource "aws_cloudfront_distribution" "lfs" {
  enabled         = true
  is_ipv6_enabled = true
  comment         = "Git LFS object cache"

  origin {
    domain_name              = aws_s3_bucket.lfs.bucket_regional_domain_name
    origin_id                = "s3-lfs-origin"
    origin_access_control_id = aws_cloudfront_origin_access_control.lfs.id
  }

  default_cache_behavior {
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "s3-lfs-origin"
    viewer_protocol_policy = "https-only"
    trusted_key_groups     = [aws_cloudfront_key_group.lfs.id]

    forwarded_values {
      query_string = false
      cookies { forward = "none" }
    }

    min_ttl     = 0
    default_ttl = var.cloudfront_signed_url_ttl_secs
    max_ttl     = 86400
  }

  restrictions {
    geo_restriction { restriction_type = "none" }
  }

  viewer_certificate {
    cloudfront_default_certificate = true
  }
}

# ── Budgets ───────────────────────────────────────────────────────────────────

resource "aws_budgets_budget" "monthly" {
  name         = "${var.function_name}-monthly-budget"
  budget_type  = "COST"
  limit_amount = tostring(var.monthly_budget_limit)
  limit_unit   = "USD"
  time_unit    = "MONTHLY"

  notification {
    comparison_operator        = "GREATER_THAN"
    threshold                  = 80
    threshold_type             = "PERCENTAGE"
    notification_type          = "ACTUAL"
    subscriber_email_addresses = [var.budget_alert_email]
  }

  notification {
    comparison_operator        = "GREATER_THAN"
    threshold                  = 100
    threshold_type             = "PERCENTAGE"
    notification_type          = "ACTUAL"
    subscriber_email_addresses = [var.budget_alert_email]
  }
}
