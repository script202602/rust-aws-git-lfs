[English](README.md) | [日本語](docs/i18n/ja/README.md)

# rust-aws-git-lfs

A Git LFS server implemented with AWS Lambda + S3, written in Rust.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)
- [AWS CLI](https://aws.amazon.com/cli/) (deploy only)

## Environment Variables

**Main Lambda:**

| Variable                    | Description                                                      | Example                                                 |
| --------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------- |
| `S3_BUCKET`                 | S3 bucket name to store LFS objects                              | `my-lfs-bucket`                                         |
| `CLOUDFRONT_DOMAIN`         | CloudFront distribution domain                                   | `d111111abcdef8.cloudfront.net`                         |
| `CLOUDFRONT_KEY_PAIR_ID`    | CloudFront public key ID                                         | `K2JCJMDEHXQW5F`                                        |
| `CLOUDFRONT_PRIVATE_KEY`    | RSA private key for CloudFront Signed URL signing (PEM format)   | `-----BEGIN PRIVATE KEY-----\n...`                      |
| `CLOUDFRONT_URL_TTL_SECS`   | CloudFront Signed URL TTL in seconds. Default: 3600              | `3600`                                                  |

If `CLOUDFRONT_*` variables are not set (e.g. LocalStack), downloads fall back to S3 presigned URLs.

**Authorizer Lambda:**

| Variable                | Description                                                                                                          | Example                   |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------- |
| `ALLOWED_GITHUB_USERS`  | Comma-separated list of GitHub usernames permitted to authenticate. Leave empty to allow any user with pull access.  | `alice,bob`               |

## Deploy & Setup

| Target | Documentation |
|---|---|
| **For users** — Deploy via CloudFormation GUI | [docs/for-users.md](docs/for-users.md) |
| **For developers** — Terraform + LocalStack testing | [docs/for-developers.md](docs/for-developers.md) |
