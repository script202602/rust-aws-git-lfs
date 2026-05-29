output "lfs_base_url" {
  description = "LFS_BASE_URL および git config lfs.url のベース URL"
  value       = aws_apigatewayv2_stage.default.invoke_url
}

output "lfs_url_example" {
  description = "git config lfs.url に設定する URL の例（owner/repo を置き換えてください）"
  value       = "${aws_apigatewayv2_stage.default.invoke_url}/<github-owner>/<github-repo>/info/lfs"
}

output "cloudfront_domain" {
  description = "LFS ダウンロードキャッシュ用 CloudFront ディストリビューションのドメイン"
  value       = aws_cloudfront_distribution.lfs.domain_name
}
