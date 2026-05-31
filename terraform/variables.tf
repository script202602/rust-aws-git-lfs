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
  description = "Lambda 関数の最大同時実行数（コスト上限対策）。-1 で無制限"
  type        = number
  default     = -1
}
