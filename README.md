# rust-aws-git-lfs

Git LFS サーバーを AWS Lambda + S3 で実装した Rust プロジェクトです。

## 前提条件

- [Rust](https://www.rust-lang.org/tools/install)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)
- [AWS CLI](https://aws.amazon.com/cli/) (デプロイ時のみ)

## 環境変数

| 変数名         | 説明                                     | 例                                                      |
| -------------- | ---------------------------------------- | ------------------------------------------------------- |
| `S3_BUCKET`    | LFS オブジェクトを格納する S3 バケット名 | `my-lfs-bucket`                                         |
| `LFS_BASE_URL` | API Gateway のベース URL                 | `https://xxxx.execute-api.ap-northeast-1.amazonaws.com` |

## 動作確認

### 1. ローカルサーバーの起動

```bash
S3_BUCKET=my-lfs-bucket LFS_BASE_URL=http://localhost:9000 cargo lambda watch
```

### 2. Batch API の確認

`data/` 配下に API Gateway v1 形式のペイロードを用意しています。

**アップロード用 presigned URL の取得:**

```bash
cargo lambda invoke --data-file data/batch-upload.json | jq .
```

**ダウンロード用 presigned URL の取得:**

```bash
cargo lambda invoke --data-file data/batch-download.json | jq .
```

### 3. Verify API の確認

```bash
cargo lambda invoke --data-file data/verify.json | jq .
```

> **注意:** ローカル実行時も S3 へのアクセスには AWS 認証情報が必要です。`~/.aws/credentials` が設定されているか、`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` を環境変数に設定してください。

### 4. ユニットテスト

```bash
cargo test
```

## LocalStack によるローカル S3 テスト

AWS 認証情報なしで S3 を含む完全なフローをローカルで確認できます。

`LOCALSTACK_AUTH_TOKEN` は [LocalStack のアカウントページ](https://app.localstack.cloud/workspace/auth-token) で取得できます。

### 1. LocalStack の起動

```bash
LOCALSTACK_AUTH_TOKEN=<your-token> \
docker run --rm -p 4566:4566 \
  -e LOCALSTACK_AUTH_TOKEN \
  localstack/localstack:latest
```

### 2. バケットの作成

```bash
AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test \
aws --endpoint-url=http://localhost:4566 \
  --region ap-northeast-1 \
  s3 mb s3://test-lfs-bucket
```

### 3. Lambda サーバーの起動

`AWS_ENDPOINT_URL` を設定すると AWS SDK が LocalStack に向きます。

```bash
AWS_ENDPOINT_URL=http://localhost:4566 \
AWS_ACCESS_KEY_ID=test \
AWS_SECRET_ACCESS_KEY=test \
AWS_DEFAULT_REGION=ap-northeast-1 \
S3_BUCKET=test-lfs-bucket \
LFS_BASE_URL=http://localhost:9000 \
cargo lambda watch
```

### 4. 動作確認

上記「動作確認」の `cargo lambda invoke` コマンドをそのまま実行できます。アップロード操作後にオブジェクトが LocalStack に保存されていることを確認:

```bash
AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test \
aws --endpoint-url=http://localhost:4566 s3 ls s3://test-lfs-bucket/objects/
```

---

## AWS デプロイ

### 最小構成でのデプロイ手順

**1. S3 バケットの作成**

```bash
aws s3api create-bucket \
  --bucket my-lfs-bucket \
  --region ap-northeast-1 \
  --create-bucket-configuration LocationConstraint=ap-northeast-1
```

**2. Lambda 関数のビルドとデプロイ**

```bash
cargo lambda build --release

cargo lambda deploy \
  --env-var S3_BUCKET=my-lfs-bucket \
  --env-var LFS_BASE_URL=https://xxxx.execute-api.ap-northeast-1.amazonaws.com
```

> `LFS_BASE_URL` はデプロイ後に API Gateway のエンドポイント URL に更新してください。

**3. Lambda に S3 アクセス権限を付与**

デプロイで作成された IAM ロール (`rust-aws-lfs-role` など) に以下のポリシーをアタッチします。

```bash
aws iam put-role-policy \
  --role-name rust-aws-lfs-role \
  --policy-name lfs-s3-access \
  --policy-document '{
    "Version": "2012-10-17",
    "Statement": [{
      "Effect": "Allow",
      "Action": ["s3:GetObject", "s3:PutObject", "s3:HeadObject"],
      "Resource": "arn:aws:s3:::my-lfs-bucket/objects/*"
    }]
  }'
```

**4. 動作確認**

デプロイ後、API Gateway の URL に対して上記の curl コマンドの `http://localhost:9000/2015-03-31/functions/rust-aws-lfs/invocations` を実際のエンドポイント URL に置き換えて実行できます。

```bash
curl -s -X POST https://xxxx.execute-api.ap-northeast-1.amazonaws.com/repos/owner/repo/info/lfs/objects/batch \
  -H 'Content-Type: application/vnd.git-lfs+json' \
  -d '{"operation":"upload","objects":[{"oid":"4d7af9c6...","size":1024}]}'
```
