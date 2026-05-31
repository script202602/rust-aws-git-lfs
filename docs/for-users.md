[English](for-users.md) | [日本語](i18n/ja/for-users.md)

# User Guide — Deploy to AWS (CloudFormation)

Provisions S3, Lambda, API Gateway, and CloudFront together. **An RSA key pair is generated automatically when the stack is created.**

## Prerequisites: Upload the Lambda Binary

CloudFormation cannot compile code, so you must upload a pre-built binary to S3 first.

### 1. Download the files

Download the following 3 files from [GitHub Releases](https://github.com/rahinaku/rust-aws-git-lfs/releases/latest):

- `rust-aws-lfs.zip`
- `rust-aws-lfs-authorizer.zip`
- `template.yaml`

### 2. Create an S3 bucket for artifacts

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

   | Parameter | Value | Description |
   |---|---|---|
   | **LFSBucketName** | `my-lfs-bucket-yourname` | Bucket to store LFS objects (must be globally unique) |
   | **ArtifactsBucketName** | `my-lfs-artifacts` | The bucket name you uploaded the ZIPs to |
   | MainFunctionS3Key | `rust-aws-lfs.zip` | Leave as default |
   | AuthorizerFunctionS3Key | `rust-aws-lfs-authorizer.zip` | Leave as default |
   | CloudFrontSignedUrlTTL | `3600` | Leave as default |

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

## Redeploy After Code Updates

1. Download the latest `rust-aws-lfs.zip` and `rust-aws-lfs-authorizer.zip` from [GitHub Releases](https://github.com/rahinaku/rust-aws-git-lfs/releases/latest)
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
