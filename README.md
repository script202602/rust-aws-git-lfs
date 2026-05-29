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

## AWS デプロイ（Terraform）

S3・Lambda・API Gateway・オーソライザーをまとめて構築します。

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

### 3. 変数ファイルの作成

```bash
cp terraform/terraform.tfvars.example terraform/terraform.tfvars
```

`terraform/terraform.tfvars` を編集してバケット名を設定します（グローバルで一意な名前にしてください）：

```hcl
bucket_name   = "my-lfs-bucket-yourname"
region        = "ap-northeast-1"
function_name = "rust-aws-lfs"
```

### 4. Lambda バイナリのビルド

Terraform の前に Lambda バイナリをビルドしておく必要があります：

```bash
cargo lambda build --release
```

### 5. Terraform でインフラを構築

```bash
cd terraform
terraform init       # プロバイダーをダウンロード（初回のみ）
terraform plan       # 変更内容を確認
terraform apply      # 実際に構築
```

`apply` 完了後、エンドポイント URL が表示されます：

```
Outputs:

lfs_base_url      = "https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com"
lfs_url_example   = "https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs"
```

### 6. git-lfs の設定

出力された URL を使って設定します：

```bash
git config lfs.url https://<API_ID>.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs
```

`git lfs push` / `git lfs pull` の初回実行時に認証プロンプトが表示されます：

```
Username: <GitHub ユーザー名>
Password: <GitHub Personal Access Token（repo スコープ）>
```

> GitHub の Personal Access Token は [Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) で `repo` スコープを付けて発行してください。

### 7. 動作確認

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
