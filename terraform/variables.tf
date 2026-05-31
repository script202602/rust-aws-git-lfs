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
  description = "Lambda 関数ごとの最大同時実行数。デフォルトは -1（無制限）。コスト上限を設ける場合に指定する。アカウント内の全 Lambda の予約済み同時実行数の合計を差し引いた未予約分が最低 10 を下回らないよう注意"
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

variable "budget_alert_email" {
  description = "月次コスト予算アラートの通知先メールアドレス"
  type        = string
}

variable "monthly_budget_limit" {
  description = "月次コスト予算の上限（USD）。80% と 100% 超過時にメール通知"
  type        = number
  default     = 10
}
