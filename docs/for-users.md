# 利用者向けガイド — AWS へのデプロイ（CloudFormation）

S3・Lambda・API Gateway・CloudFront をまとめて構築します。**RSA キーペアはスタック作成時に自動生成**されます。

## 事前準備：Lambda バイナリのアップロード

CloudFormation はコードのコンパイルができないため、先にビルド済みバイナリを S3 に置く必要があります。

**ローカル（CLI あり）の場合:**

```bash
# artifacts 用バケットを作成（初回のみ）
aws s3 mb s3://my-lfs-artifacts --region ap-northeast-1

# ビルド → ZIP → アップロードを一括実行
./scripts/upload-artifacts.sh my-lfs-artifacts
```

**ブラウザだけで完結させる場合（AWS CloudShell）:**

1. AWS コンソール右上の **[>\_]** アイコンをクリックして CloudShell を開く
2. 以下を実行：

```bash
# Rust と Cargo Lambda のインストール（初回のみ・数分かかります）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
pip install cargo-lambda

# リポジトリをクローン
git clone https://github.com/<your-org>/rust-aws-git-lfs.git
cd rust-aws-git-lfs

# artifacts 用バケットを作成
aws s3 mb s3://my-lfs-artifacts --region ap-northeast-1

# ビルド → アップロード
./scripts/upload-artifacts.sh my-lfs-artifacts
```

## CloudFormation でスタックを作成

1. AWS コンソール → **CloudFormation** → 「スタックの作成」→「新しいリソースを使用」
2. 「テンプレートファイルのアップロード」→ `cloudformation/template.yaml` を選択 → 「次へ」
3. パラメータを入力：

   | パラメータ | 入力値 | 説明 |
   |---|---|---|
   | **LFSBucketName** | `my-lfs-bucket-yourname` | LFS オブジェクト格納先（グローバルで一意にしてください） |
   | **ArtifactsBucketName** | `my-lfs-artifacts` | 上でアップロードしたバケット名 |
   | MainFunctionS3Key | `rust-aws-lfs.zip` | デフォルトのまま |
   | AuthorizerFunctionS3Key | `rust-aws-lfs-authorizer.zip` | デフォルトのまま |
   | CloudFrontSignedUrlTTL | `3600` | デフォルトのまま |

4. 「次へ」→「次へ」→ **「AWS CloudFormation によって IAM リソースが作成される場合があることを承認します」にチェック** → 「スタックの作成」
5. 「イベント」タブで進捗を確認（完了まで約 5〜10 分）

## 完了後の確認

スタックの「出力」タブに以下が表示されます：

```
LFSBaseUrl      → https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com
LFSUrlExample   → https://xxxxxxxxxx.execute-api.ap-northeast-1.amazonaws.com/<owner>/<repo>/info/lfs
CloudFrontDomain → d111111abcdef8.cloudfront.net
```

`LFSUrlExample` の `<owner>/<repo>` を実際の GitHub リポジトリ名に置き換えて git-lfs を設定します：

```bash
git config lfs.url https://<API_ID>.execute-api.ap-northeast-1.amazonaws.com/<owner>/<repo>/info/lfs
```

`git lfs push` / `git lfs pull` の初回実行時に認証プロンプトが表示されます：

```
Username: <GitHub ユーザー名>
Password: <GitHub Personal Access Token（repo スコープ）>
```

> GitHub の Personal Access Token は [Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) で `repo` スコープを付けて発行してください。

## コード更新時の再デプロイ

```bash
./scripts/upload-artifacts.sh my-lfs-artifacts
```

アップロード後、CloudFormation コンソール → スタック → 「更新」→「既存のテンプレートを使用」→ パラメータはそのまま → 「スタックの更新」

## スタックの削除

1. S3 バケットにオブジェクトが残っている場合は先に空にする：
   ```bash
   aws s3 rm s3://my-lfs-bucket-yourname --recursive
   ```
2. CloudFormation コンソール → スタック選択 → 「削除」
