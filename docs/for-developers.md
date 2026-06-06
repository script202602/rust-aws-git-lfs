[English](for-developers.md) | [日本語](i18n/ja/for-developers.md)

# Developer Guide

## AWS Deployment (Terraform)

Provisions S3, Lambda, API Gateway, and CloudFront together.

### Prerequisites

- [Terraform](https://developer.hashicorp.com/terraform/install)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)
- [AWS CLI](https://aws.amazon.com/cli/)

### 1. Install Terraform

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

Verify installation:

```bash
terraform -version
```

### 2. Verify AWS credentials

```bash
# For static keys in ~/.aws/credentials
aws sts get-caller-identity

# For AWS SSO or credential_process, export to environment variables
eval $(aws configure export-credentials --format env)
```

### 3. Generate an RSA key pair

Generate the RSA key pair used to sign CloudFront Signed URLs (first time only):

```bash
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out cloudfront_private.pem
openssl rsa -pubout -in cloudfront_private.pem -out cloudfront_public.pem
```

> **Note:** `*.pem` files are listed in `.gitignore`. Do not commit them to the repository.

### 4. Create the variables file

```bash
cp terraform/terraform.tfvars.example terraform/terraform.tfvars
```

Edit `terraform/terraform.tfvars`:

```hcl
region        = "ap-northeast-1"
function_name = "rust-aws-lfs"

# Optional: cost protection (uncomment and adjust as needed)
# lambda_reserved_concurrency          = 50   # Normally not needed — see warning below
# api_throttling_burst_limit           = 50
# log_retention_days                   = 30
# cloudfront_geo_restriction_locations = ["JP"]  # Whitelist by country code

# Optional: restrict authentication to specific GitHub accounts
# allowed_github_users = "alice,bob"  # Leave empty to allow any user with pull access
```

> The S3 bucket name is auto-generated as `lfs-<account-id>-<region>`.

> **⚠️ `lambda_reserved_concurrency` limit**
>
> This project deploys 2 Lambda functions (main + authorizer), so total reserved concurrency is **`lambda_reserved_concurrency × 2`**.
>
> **New or free-tier accounts may have a limit as low as 10.** In that case, any positive value always causes `TooManyRequestsException`. Leave this at `-1` and use `api_throttling_rate_limit` / `api_throttling_burst_limit` for cost control instead.
>
> Check available capacity first:
> ```bash
> aws lambda get-account-settings --query 'AccountLimit.[ConcurrentExecutions,UnreservedConcurrentExecutions]'
> ```
> `UnreservedConcurrentExecutions` must be at least `lambda_reserved_concurrency × 2 + 10`.
> To raise the limit, request a quota increase via [AWS Service Quotas](https://console.aws.amazon.com/servicequotas/).

Pass the PEM keys via `TF_VAR_` environment variables. HCL heredocs cannot parse PEM, so use this method:

```bash
export TF_VAR_cloudfront_private_key_pem="$(cat cloudfront_private.pem)"
export TF_VAR_cloudfront_public_key_pem="$(cat cloudfront_public.pem)"
```

### 5. Build the Lambda binary

```bash
cargo lambda build --release
```

### 6. Provision infrastructure with Terraform

```bash
cd terraform
terraform init       # Download providers (first time only)
terraform plan       # Preview changes
terraform apply      # Apply changes
```

> **Note — CloudWatch Log Groups:** If the Lambda functions have already been invoked before running Terraform, the log groups are auto-created by Lambda and Terraform will fail with `ResourceAlreadyExistsException`. Import them first:
> ```bash
> terraform import aws_cloudwatch_log_group.main /aws/lambda/rust-aws-lfs
> terraform import aws_cloudwatch_log_group.authorizer /aws/lambda/rust-aws-lfs-authorizer
> ```

After `apply` completes, the endpoint URL is shown:

```
Outputs:

lfs_base_url      = "https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com"
lfs_url_example   = "https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs"
cloudfront_domain = "d111111abcdef8.cloudfront.net"
```

### 7. Configure git-lfs

```bash
git config lfs.url https://<API_ID>.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs
```

On the first `git lfs push` / `git lfs pull`, you will be prompted for credentials:

```
Username: <GitHub username>
Password: <GitHub Personal Access Token (repo scope)>
```

> Generate a Personal Access Token with the `repo` scope at [Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens).

### 8. Verify operation

```bash
curl -s -u <github-username>:<github-pat> \
  -X POST https://<API_ID>.execute-api.ap-northeast-1.amazonaws.com/<github-owner>/<github-repo>/info/lfs/objects/batch \
  -H 'Content-Type: application/vnd.git-lfs+json' \
  -d '{"operation":"upload","objects":[{"oid":"4d7af9c6...","size":1024}]}'
```

### 9. (Optional) Switch to CloudFront Fixed Pricing Plan

Fixed pricing plans eliminate overage charges even during DDoS attacks or traffic spikes. However, note that the free included allowances (data transfer and requests) are lower than the pay-as-you-go always-free tier. See the official pages for current limits:

- [Pay-as-you-go pricing & free tier](https://aws.amazon.com/cloudfront/pricing/pay-as-you-go/)
- [Fixed pricing plans & limits](https://aws.amazon.com/cloudfront/pricing/)

#### Steps to Switch

> **Note:** This step cannot be automated with Terraform. It must be done manually from the AWS Console.

**Prerequisites:**
- Terraform deployment has completed successfully (`main.tf` already uses `Managed-CachingOptimized`, which is required for fixed pricing plans)
- No AWS credits, EDP discounts, or promotions are active on your account (they cannot be combined with fixed pricing plans)

1. Open the [CloudFront Console](https://console.aws.amazon.com/cloudfront/)
2. In the left menu, select **"Distributions"**
3. Click the link in the **ID column** to open the distribution detail page
4. In the **"Billing"** section, click **"Switch"** on the desired plan
5. Review the details on the confirmation screen and confirm

### Update Lambda (after code changes)

```bash
cargo lambda build --release
cd terraform && terraform apply
```

### Destroy infrastructure

```bash
cd terraform && terraform destroy
```

> **Note:** `terraform destroy` will fail if the S3 bucket still contains objects. Empty it first with `aws s3 rm s3://<bucket-name> --recursive`.

---

## Local S3 Testing with LocalStack

Test the full flow including S3 locally without AWS credentials.

Get your `LOCALSTACK_AUTH_TOKEN` from the [LocalStack account page](https://app.localstack.cloud/workspace/auth-token).

### 1. Start LocalStack

```bash
LOCALSTACK_AUTH_TOKEN=<your-token> \
docker run --rm -p 4566:4566 \
  -e LOCALSTACK_AUTH_TOKEN \
  localstack/localstack:latest
```

### 2. Create a bucket

```bash
AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test \
aws --endpoint-url=http://localhost:4566 \
  --region ap-northeast-1 \
  s3 mb s3://test-lfs-bucket
```

### 3. Start the Lambda server

Setting `AWS_ENDPOINT_URL` directs the AWS SDK to LocalStack.

```bash
AWS_ENDPOINT_URL=http://localhost:4566 \
AWS_ACCESS_KEY_ID=test \
AWS_SECRET_ACCESS_KEY=test \
AWS_DEFAULT_REGION=ap-northeast-1 \
S3_BUCKET=test-lfs-bucket \
cargo lambda watch
```

### 4. Verify operation

Run the same `cargo lambda invoke` commands from [README verification steps](../README.md#verify-operation). After an upload operation, confirm the object is stored in LocalStack:

```bash
AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test \
aws --endpoint-url=http://localhost:4566 s3 ls s3://test-lfs-bucket/objects/
```

---

## GitHub Actions E2E Tests

`.github/workflows/e2e.yml` automatically runs E2E tests on pull requests and pushes. It uses LocalStack, so the following setup is required.

### Set LOCALSTACK_AUTH_TOKEN

1. Create a free account at [LocalStack](https://app.localstack.cloud/workspace/auth-token) and obtain your token
2. Go to your GitHub repository's **Settings → Secrets and variables → Actions**
3. Add `LOCALSTACK_AUTH_TOKEN` as a **New repository secret**
