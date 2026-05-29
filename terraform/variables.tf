variable "region" {
  description = "AWS リージョン"
  type        = string
  default     = "ap-northeast-1"
}

variable "bucket_name" {
  description = "LFS オブジェクトを格納する S3 バケット名（グローバルで一意である必要がある）"
  type        = string
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
