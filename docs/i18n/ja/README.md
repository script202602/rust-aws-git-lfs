[English](../../../README.md) | [日本語](README.md)

# rust-aws-git-lfs

Git LFS サーバーを AWS Lambda + S3 で実装した Rust プロジェクトです。

## 前提条件

- [Rust](https://www.rust-lang.org/tools/install)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)
- [AWS CLI](https://aws.amazon.com/cli/) (デプロイ時のみ)

## 環境変数

| 変数名                      | 説明                                                    | 例                                                      |
| --------------------------- | ------------------------------------------------------- | ------------------------------------------------------- |
| `S3_BUCKET`                 | LFS オブジェクトを格納する S3 バケット名                | `my-lfs-bucket`                                         |
| `CLOUDFRONT_DOMAIN`         | CloudFront ディストリビューションのドメイン             | `d111111abcdef8.cloudfront.net`                         |
| `CLOUDFRONT_KEY_PAIR_ID`    | CloudFront 公開鍵の ID                                  | `K2JCJMDEHXQW5F`                                        |
| `CLOUDFRONT_PRIVATE_KEY`    | CloudFront Signed URL 署名用 RSA 秘密鍵（PEM 形式）     | `-----BEGIN PRIVATE KEY-----\n...`                      |
| `CLOUDFRONT_URL_TTL_SECS`   | CloudFront Signed URL の有効期限（秒）。デフォルト 3600 | `3600`                                                  |

`CLOUDFRONT_*` が未設定の場合（LocalStack など）、ダウンロードは S3 presigned URL にフォールバックします。

## デプロイ・セットアップ

| 対象 | ドキュメント |
|---|---|
| **利用者向け** — CloudFormation で GUI から構築 | [for-users.md](for-users.md) |
| **開発者向け** — Terraform + LocalStack テスト | [for-developers.md](for-developers.md) |
