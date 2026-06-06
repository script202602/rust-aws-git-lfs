variable "region" {
  description = "AWS リージョン"
  type        = string
  default     = "ap-northeast-1"
}

variable "function_name" {
  description = "Lambda 関数名のベース"
  type        = string
  default     = "rust-aws-lfs"
}

variable "cloudfront_private_key_pem" {
  description = "CloudFront Signed URL 生成用 RSA 秘密鍵（PEM 形式）。terraform.tfvars に記載しコミットしないこと"
  type        = string
  sensitive   = true
}

variable "cloudfront_public_key_pem" {
  description = "CloudFront にアップロードする RSA 公開鍵（PEM 形式）"
  type        = string
}

variable "cloudfront_signed_url_ttl_secs" {
  description = "CloudFront Signed URL の有効期限（秒）"
  type        = number
  default     = 3600
}

variable "lambda_reserved_concurrency" {
  description = "Lambda 関数ごとの最大同時実行数。通常は -1（無制限）のまま変更不要。API Gateway のスロットリングがリクエスト数を制限するため Lambda 側の制限は不要。設定する場合はアカウントの未予約同時実行数（UnreservedConcurrentExecutions）が 設定値×2+10 以上あることを確認すること"
  type        = number
  default     = -1
}

variable "api_throttling_rate_limit" {
  description = "API Gateway の持続リクエストレート上限（リクエスト/秒）"
  type        = number
  default     = 10
}

variable "api_throttling_burst_limit" {
  description = "API Gateway のバーストリクエスト上限"
  type        = number
  default     = 50
}


variable "log_retention_days" {
  description = "CloudWatch Logs の保持日数"
  type        = number
  default     = 30
}

variable "cloudfront_geo_restriction_locations" {
  description = "CloudFront の地理的制限（ホワイトリスト）の国コード（ISO 3166-1 alpha-2）。空リストで制限なし（例: [\"JP\", \"US\"]）"
  type        = list(string)
  default     = []
}

variable "allowed_github_users" {
  description = "認証を許可する GitHub ユーザー名（カンマ区切り）。空文字でユーザー制限なし（例: \"alice,bob\"）"
  type        = string
  default     = ""
}

variable "cloudfront_waf_web_acl_arn" {
  description = "CloudFront に関連付ける WAF Web ACL の ARN。定額プランサブスクリプション使用時は必須。空文字で WAF なし（例: \"arn:aws:wafv2:us-east-1:123456789012:global/webacl/CreatedByCloudFront-xxxxxxxx/...\"）"
  type        = string
  default     = ""
}
