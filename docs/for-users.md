[English](for-users.md) | [日本語](i18n/ja/for-users.md)

# User Guide — Deploy to AWS (CloudFormation)

Provisions S3, Lambda, API Gateway, and CloudFront together. **An RSA key pair is generated automatically when the stack is created.**

## Prerequisites: Upload the Lambda Binary

CloudFormation cannot compile code, so you must upload a pre-built binary to S3 first.

### 1. Download the files

Download the following 3 files from [GitHub Releases](https://github.com/script202602/rust-aws-git-lfs/releases/latest):

- `rust-aws-lfs.zip`
- `rust-aws-lfs-authorizer.zip`
- `template.yaml`

### 2. Create an S3 bucket for artifacts

> **⚠️ Region must match:** The bucket **must be in the same AWS region** as the CloudFormation stack.
> A cross-region bucket causes a `PermanentRedirect` error and the Lambda functions will fail to create.

1. AWS Console → **S3** → "Create bucket"
2. Enter a bucket name (e.g. `my-lfs-artifacts`) and select the **same region** where you will create the stack
3. Leave all other settings as default and click "Create bucket"

### 3. Upload the ZIP files

1. Open the bucket you created and click "**Upload**"
2. Add `rust-aws-lfs.zip` and `rust-aws-lfs-authorizer.zip`
3. Click "Upload"

## Create the CloudFormation Stack

1. AWS Console → **CloudFormation** → "Create stack" → "With new resources"
2. "Upload a template file" → select the downloaded `template.yaml` → "Next"
3. Fill in the parameters:

   **Required:**

   | Parameter | Value | Description |
   |---|---|---|
   | **ArtifactsBucketName** | `my-lfs-artifacts` | The S3 bucket where you uploaded the ZIPs |

   **Optional (leave as default or adjust as needed):**

   | Parameter | Default | Description |
   |---|---|---|
   | MainFunctionS3Key | `rust-aws-lfs.zip` | S3 object key for the main Lambda ZIP |
   | AuthorizerFunctionS3Key | `rust-aws-lfs-authorizer.zip` | S3 object key for the authorizer Lambda ZIP |
   | CloudFrontSignedUrlTTL | `3600` | Signed URL expiry in seconds |
   | LambdaMaxConcurrency | `-1` | Max concurrent executions per Lambda function. **Leave at `-1` (default) in most cases.** API Gateway throttling already limits the request rate, so a Lambda-side concurrency cap is unnecessary and may cause errors depending on the account's concurrency limit (see warning below). |
   | ApiThrottlingRateLimit | `10` | API Gateway sustained request rate (req/s) |
   | ApiThrottlingBurstLimit | `50` | API Gateway burst request limit |
   | LogRetentionDays | `30` | CloudWatch Logs retention period in days |
   | CloudFrontGeoRestrictionLocations | *(empty)* | Comma-separated ISO 3166-1 alpha-2 country codes for CloudFront whitelist (e.g. `JP,US`). Leave empty for no restriction. |

   > **⚠️ LambdaMaxConcurrency limit**
   >
   > This project deploys **2 Lambda functions** (main + authorizer), and the same value is applied to both.
   > The total reserved concurrency consumed in your account is therefore **`LambdaMaxConcurrency × 2`**.
   >
   > **Error condition:**
   > ```
   > Account concurrency limit
   >   − total reserved concurrency of all Lambda functions in the account
   >   − LambdaMaxConcurrency × 2
   > < 10 (minimum required unreserved concurrency)
   > ```
   >
   > **New or free-tier accounts may have a limit as low as 10.**
   > In that case, any positive value will always trigger `TooManyRequestsException`.
   > Leave this parameter at `-1` (default) and rely on `ApiThrottlingRateLimit` / `ApiThrottlingBurstLimit` for cost protection instead.
   >
   > **Check your current limit before setting a value:**
   >
   > 1. AWS Console → **Lambda**
   > 2. On the **Dashboard** (default landing page), find the **"Resources"** section for your region
   > 3. Confirm **"Unreserved account concurrency"** is at least `LambdaMaxConcurrency × 2 + 10`
   >
   > To raise the limit: AWS Console → **Service Quotas** → "AWS services" → **"AWS Lambda"** → **"Concurrent executions"** → "Request quota increase".
   >
   > **Recommended value (when limit is sufficient):** Set to the same value as `ApiThrottlingBurstLimit` (default 50) to align the Lambda cap with the API Gateway burst limit.

4. "Next" → "Next" → **check "I acknowledge that AWS CloudFormation might create IAM resources"** → "Create stack"
5. Monitor progress in the "Events" tab (takes approximately 5–10 minutes)

## Verify After Completion

The stack's "Outputs" tab will show:

```
LFSBaseUrl       → https://xxxxxxxxxx.execute-api.<region>.amazonaws.com
LFSUrlExample    → https://xxxxxxxxxx.execute-api.<region>.amazonaws.com/<owner>/<repo>/info/lfs
CloudFrontDomain → d111111abcdef8.cloudfront.net
```

Replace `<owner>/<repo>` in `LFSUrlExample` with your actual GitHub repository name and configure git-lfs:

```bash
git config lfs.url https://<API_ID>.execute-api.<region>.amazonaws.com/<owner>/<repo>/info/lfs
```

On the first `git lfs push` / `git lfs pull`, you will be prompted for credentials:

```
Username: <GitHub username>
Password: <GitHub Personal Access Token (repo scope)>
```

> Generate a Personal Access Token with the `repo` scope at [Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens).

## (Optional) Switch to CloudFront Fixed Pricing Plan

Fixed pricing plans eliminate overage charges even during DDoS attacks or traffic spikes. However, note that the free included allowances (data transfer and requests) are lower than the pay-as-you-go always-free tier. See the official pages for current limits:

- [Pay-as-you-go pricing & free tier](https://aws.amazon.com/cloudfront/pricing/pay-as-you-go/)
- [Fixed pricing plans & limits](https://aws.amazon.com/cloudfront/pricing/)

### Steps to Switch

> **Note:** This step cannot be automated with CloudFormation. It must be done manually from the AWS Console.

**Prerequisites:**
- The CloudFormation stack has been successfully deployed (the template already uses `Managed-CachingOptimized`, which is required for fixed pricing plans)
- No AWS credits, EDP discounts, or promotions are active on your account (they cannot be combined with fixed pricing plans)

1. Open the [CloudFront Console](https://console.aws.amazon.com/cloudfront/)
2. In the left menu, select **"Distributions"**
3. Click the link in the **ID column** to open the distribution detail page
4. In the **"Billing"** section, click **"Switch"** on the desired plan
5. Review the details on the confirmation screen and confirm

## Troubleshooting

### `AWS::Logs::LogGroup` — AlreadyExists error

If stack creation fails with:

```
Resource of type 'AWS::Logs::LogGroup' with identifier '...' already exists.
```

A previous stack deletion left the log groups behind. Delete them before retrying:

1. AWS Console → **CloudWatch** → **Log groups** (left sidebar)
2. In the search box, enter `/aws/lambda/<your-stack-name>` (e.g. `/aws/lambda/git-lfs`)
3. Select all matching log groups (`-rsa-key-gen`, `-authorizer`, `-main`)
4. **Actions** → **Delete log group(s)** → confirm

Then retry creating the stack.

---

## Redeploy After Code Updates

1. Download the latest `rust-aws-lfs.zip` and `rust-aws-lfs-authorizer.zip` from [GitHub Releases](https://github.com/script202602/rust-aws-git-lfs/releases/latest)
2. AWS Console → **S3** → artifacts bucket → "Upload" to overwrite both files
3. AWS Console → **CloudFormation** → select the stack → "Update" → "Use existing template" → keep parameters unchanged → "Update stack"

## Delete the Stack

### 1. Empty and delete the S3 buckets

CloudFormation cannot delete buckets that still contain objects, so empty them first from the S3 console.

**LFS bucket (`my-lfs-bucket-yourname`):**

1. AWS Console → **S3** → select `my-lfs-bucket-yourname`
2. Click "**Empty**" → type `permanently delete` in the text box → "Empty"
3. Click "**Delete**" → enter the bucket name → "Delete bucket"

**Repeat for the artifacts bucket (`my-lfs-artifacts`):**

1. Select `my-lfs-artifacts` from the S3 bucket list
2. "**Empty**" then "**Delete**"

### 2. Delete the CloudFormation stack

1. AWS Console → **CloudFormation** → select the stack
2. Click "**Delete**" → confirm with "Delete stack"
3. Monitor progress in the "Events" tab (takes approximately 5–10 minutes)

> Deleting the stack automatically removes the Lambda functions, API Gateway, CloudFront distribution, IAM roles, and SSM parameters (RSA key pair).
