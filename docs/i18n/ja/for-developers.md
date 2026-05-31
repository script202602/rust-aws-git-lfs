[English](../../for-developers.md) | [日本語](for-developers.md)

# 開発者向けガイド

## AWS デプロイ（Terraform）

S3・Lambda・API Gateway・CloudFront をまとめて構築します。

### 前提条件

- [Terraform](https://developer.hashicorp.com/terraform/install)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)
- [AWS CLI](https://aws.amazon.com/cli/)

### 1. Terraform のインストール

**macOS**

```bash
brew tap hashicorp/tap
brew install hashicorp/tap/terraform
```

**Linux**

```bash
sudo apt-get update && sudo apt-get install -y gnupg software-properties-common
wget -O- https://apt.releases.hashicorp.com/gpg | gpg --dearmor | sudo tee /usr/share/keyrings/hashicorp-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/hashicorp.list
sudo apt-get update && sudo apt-get install terraform
```

**Windows**

```powershell
choco install terraform
```

インストール確認：

```bash
terraform -version
```

### 2. AWS 認証情報の確認

```bash
# 通常の場合（~/.aws/credentials に静的キーがある場合）
aws sts get-caller-identity

# AWS SSO や credential_process を使っている場合は環境変数にエクスポート
eval $(aws configure export-credentials --format env)
```

### 3. RSA キーペアの生成

CloudFront Signed URL の署名に使う RSA キーペアを生成します（初回のみ）：

```bash
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out cloudfront_private.pem
openssl rsa -pubout -in cloudfront_private.pem -out cloudfront_public.pem
```

> **注意:** `*.pem` ファイルは `.gitignore` に含まれています。リポジトリにコミットしないでください。

### 4. 変数ファイルの作成

```bash
cp terraform/terraform.tfvars.example terraform/terraform.tfvars
```

`terraform/terraform.tfvars` を編集します：

```hcl
region        = "ap-northeast-1"
function_name = "rust-aws-lfs"

# 任意：コスト保護（必要に応じてコメントアウトを外して設定）
# lambda_reserved_concurrency          = 50   # 通常は指定不要（下記注意事項参照）
# api_throttling_burst_limit           = 50
# log_retention_days                   = 30
# cloudfront_geo_restriction_locations = ["JP"]  # 国コードでホワイトリスト制限
```

> S3 バケット名は `lfs-<アカウントID>-<リージョン>` の形式で自動生成されます。

> **⚠️ `lambda_reserved_concurrency` の設定制限**
>
> このプロジェクトは Lambda 関数が 2 つ（main + authorizer）あるため、消費される予約済み同時実行数は **`lambda_reserved_concurrency × 2`** になります。
>
> **新規アカウントや無料枠アカウントは同時実行上限が 10 の場合があります。**
> この場合、正の値を設定すると `TooManyRequestsException` エラーが必ず発生します。
> `-1`（デフォルト）のまま使用し、`api_throttling_rate_limit` / `api_throttling_burst_limit` でコストを制御してください。
>
> 設定前に空き容量を確認してください：
> ```bash
> aws lambda get-account-settings --query 'AccountLimit.[ConcurrentExecutions,UnreservedConcurrentExecutions]'
> ```
> `UnreservedConcurrentExecutions` が `lambda_reserved_concurrency × 2 + 10` 以上あれば設定可能です。
> 上限を引き上げたい場合は [AWS Service Quotas](https://console.aws.amazon.com/servicequotas/) から「Lambda の同時実行数」の引き上げをリクエストしてください。

PEM 鍵は `TF_VAR_` 環境変数で渡します。HCL の heredoc は PEM をパースできないため、この方法を使ってください：

```bash
export TF_VAR_cloudfront_private_key_pem="$(cat cloudfront_private.pem)"
export TF_VAR_cloudfront_public_key_pem="$(cat cloudfront_public.pem)"
```

### 5. Lambda バイナリのビルド

```bash
cargo lambda build --release
```

### 6. Terraform でインフラを構築

```bash
cd terraform
terraform init       # プロバイダーをダウンロード（初回のみ）
terraform plan       # 変更内容を確認
terraform apply      # 実際に構築
```

> **注意 — CloudWatch ロググループ：** Lambda 関数が Terraform 実行前に一度でも呼ばれていた場合、Lambda がロググループを自動作成するため `ResourceAlreadyExistsException` で失敗します。先に import してください：
> ```bash
> terraform import aws_cloudwatch_log_group.main /aws/lambda/rust-aws-lfs
> terraform import aws_cloudwatch_log_group.authorizer /aws/lambda/rust-aws-lfs-authorizer
> ```

`apply` 完了後、エンドポイント URL が表示されます：

```
Outputs:

lfs_base_url      = "https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com"
lfs_url_example   = "https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs"
cloudfront_domain = "d111111abcdef8.cloudfront.net"
```

### 7. git-lfs の設定

```bash
git config lfs.url https://<API_ID>.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs
```

`git lfs push` / `git lfs pull` の初回実行時に認証プロンプトが表示されます：

```
Username: <GitHub ユーザー名>
Password: <GitHub Personal Access Token（repo スコープ）>
```

> GitHub の Personal Access Token は [Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) で `repo` スコープを付けて発行してください。

### 8. 動作確認

```bash
curl -s -u <github-username>:<github-pat> \
  -X POST https://<API_ID>.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs/objects/batch \
  -H 'Content-Type: application/vnd.git-lfs+json' \
  -d '{"operation":"upload","objects":[{"oid":"4d7af9c6...","size":1024}]}'
```

### Lambda の更新（コード変更時）

```bash
cargo lambda build --release
cd terraform && terraform apply
```

### インフラの削除

```bash
cd terraform && terraform destroy
```

> **注意:** S3 バケットにオブジェクトが残っている場合は `terraform destroy` が失敗します。先に `aws s3 rm s3://<bucket-name> --recursive` で空にしてください。

---

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
cargo lambda watch
```

### 4. 動作確認

[README の動作確認手順](README.md#動作確認)と同じ `cargo lambda invoke` コマンドをそのまま実行できます。アップロード操作後にオブジェクトが LocalStack に保存されていることを確認:

```bash
AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test \
aws --endpoint-url=http://localhost:4566 s3 ls s3://test-lfs-bucket/objects/
```

---

## GitHub Actions E2E テスト

`.github/workflows/e2e.yml` が pull request ・push 時に自動でE2Eテストを実行します。LocalStack を使うため、以下の設定が必要です。

### LOCALSTACK_AUTH_TOKEN の設定

1. [LocalStack アカウントページ](https://app.localstack.cloud/workspace/auth-token) で無料アカウントを作成し、トークンを取得する
2. GitHub リポジトリの **Settings → Secrets and variables → Actions** を開く
3. **New repository secret** から `LOCALSTACK_AUTH_TOKEN` を追加する
