# アーキテクチャ設計書

## 概要

Git LFS (Large File Storage) プロトコルを AWS Lambda 上で実装し、オブジェクトストレージに AWS S3 を使用する。

---

## Git LFS プロトコル概要

Git LFS はオブジェクトの転送に [Batch API](https://github.com/git-lfs/git-lfs/blob/main/docs/api/batch.md) を使用する。

### 転送フロー

```
git push/pull
    │
    ▼
[git-lfs client]
    │  POST /info/lfs/objects/batch
    ▼
[Lambda: LFS Handler]  ───────────────────────────────────┐
    │  S3: presigned URL 生成                               │
    ▼                                                       │
[レスポンス: presigned URL]                                │
    │                                                       │
    ▼                                                       │
[git-lfs client]                                           │
    │  PUT/GET (presigned URL で直接 S3 アクセス)          │
    ▼                                                       │
[AWS S3]  ◄─────────────────────────────────────────────┘
```

実際のオブジェクト転送は **presigned URL 経由で S3 に直接** 行われるため、Lambda は大容量データを扱わない。

---

## システム構成

### Lambda 関数の分割方針

| Lambda 関数 | 役割 | 理由 |
|---|---|---|
| **lfs-handler** | LFS Batch API、presigned URL 生成 | 高頻度・低レイテンシが求められる |
| **repo-delete-initiator** | 削除ジョブの起動 | 同期的に完了できないため非同期化 |
| **repo-delete-worker** | S3 オブジェクトの一括削除 | タイムアウト対策のバッチ処理 |

> **分割の理由：** リポジトリ削除は S3 に数千〜数万オブジェクトが存在する場合、Lambda の最大実行時間 15 分を超える可能性がある。SQS を介した非同期バッチ処理で対応する。

### 全体構成図

```
[git-lfs client]
    │
    │ HTTPS + AWS Signature v4
    ▼
[API Gateway]
    ├── /repos/{owner}/{repo}/info/lfs/*  → lfs-handler Lambda
    └── /repos/{owner}/{repo}  (DELETE)   → repo-delete-initiator Lambda
                                                    │
                                                    │ SQS メッセージ
                                                    ▼
                                           [SQS Queue: lfs-delete]
                                                    │
                                                    ▼
                                           repo-delete-worker Lambda
                                                    │
                                                    ▼
                                                  [S3]
```

---

## 認証方式

### AWS IAM 認証 (推奨)

API Gateway の認証タイプを `AWS_IAM` に設定する。

- **メリット**: 追加の認証 Lambda 不要、SigV4 で署名検証を API Gateway が担う
- **設定**: IAM Policy で `execute-api:Invoke` 権限を付与

#### git-lfs クライアントの設定

git-lfs 自体は IAM 認証をネイティブサポートしないため、認証情報を HTTP Basic Auth にマッピングするクレデンシャルヘルパーを使用する。

```
# ~/.gitconfig
[credential "https://<api-gateway-id>.execute-api.<region>.amazonaws.com"]
    helper = aws-credential-lfs-helper
```

または、AWS SDK を内包した認証プロキシ（サイドカー）経由でアクセスする方式も検討可能。

> **代替案**: API Gateway に Lambda Authorizer を設置し、Bearer Token (AWS STS 発行の一時トークン) を検証する。git-lfs の `lfsconfig` で `access = basic` を設定して Bearer Token を渡す。

---

## Lambda 制約への対応

| 制約 | 値 | 対応策 |
|---|---|---|
| 最大実行時間 | 15 分 | 削除処理を SQS + Worker Lambda で分散 |
| ペイロードサイズ | 6 MB (同期) / 256 MB (非同期) | LFS オブジェクトは S3 presigned URL 経由でバイパス |
| メモリ | 最大 10 GB | 削除 Worker は処理量に応じて設定 |
| 同時実行数 | アカウントデフォルト 1000 | 削除 Worker に予約同時実行数を設定してスロットリング |

### 削除処理の詳細フロー

```
DELETE /repos/{owner}/{repo}
    │
    ▼
repo-delete-initiator Lambda
    │ 1. SQS に削除ジョブメッセージを送信
    │ 2. 202 Accepted を即時返却
    ▼
SQS Queue (lfs-delete)
    │ VisibilityTimeout: 900秒 (15分)
    ▼
repo-delete-worker Lambda
    │ 1. S3 ListObjectsV2 でオブジェクト一覧取得 (最大1000件/回)
    │ 2. S3 DeleteObjects でバッチ削除 (最大1000件/回)
    │ 3. 次ページがある場合: 新たな SQS メッセージを送信して継続
    │ 4. 全削除完了
    └── エラー時: SQS デッドレターキュー (DLQ) に移動
```

---

## インフラ構成 (IaC)

`cargo lambda deploy` による SAM/CloudFormation テンプレートか、Terraform での管理を推奨。

### 必要な AWS リソース

- API Gateway (HTTP API または REST API)
- Lambda × 3 (lfs-handler, repo-delete-initiator, repo-delete-worker)
- S3 Bucket × 1 (LFS オブジェクト格納)
- SQS Queue × 1 + DLQ × 1 (非同期削除)
- IAM Role (Lambda 実行用、最小権限原則)
