# API 設計書

## 概要

Git LFS プロトコル仕様 ([git-lfs/lfs-spec](https://github.com/git-lfs/git-lfs/blob/main/docs/api)) に準拠したエンドポイントと、リポジトリ削除用のエンドポイントを提供する。

---

## 共通仕様

### ベース URL

```
https://{api-gateway-id}.execute-api.{region}.amazonaws.com/{stage}
```

### 認証

全エンドポイントで **AWS IAM 認証 (SigV4)** を使用する。

```
Authorization: AWS4-HMAC-SHA256 Credential=...
```

> git-lfs クライアントからは認証ヘルパーを経由して自動付与する。

### Content-Type

LFS 系エンドポイントは Git LFS 仕様に従い以下を使用する。

```
Accept: application/vnd.git-lfs+json
Content-Type: application/vnd.git-lfs+json
```

### エラーレスポンス形式

```json
{
  "message": "エラーの説明",
  "documentation_url": "https://..."
}
```

---

## LFS エンドポイント

Lambda: **lfs-handler**

### POST `/repos/{owner}/{repo}/info/lfs/objects/batch`

Git LFS Batch API。アップロードまたはダウンロード用の presigned URL を返す。

**パスパラメータ:**

| 名前 | 型 | 説明 |
|---|---|---|
| `owner` | string | リポジトリオーナー名 |
| `repo` | string | リポジトリ名 |

**リクエストボディ:**

```json
{
  "operation": "upload",
  "transfers": ["basic"],
  "objects": [
    {
      "oid": "4d7af9c6e8b123456789abcdef1234567890abcdef1234567890abcdef12345678",
      "size": 1048576
    }
  ],
  "hash_algo": "sha256"
}
```

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `operation` | string | ✓ | `upload` または `download` |
| `transfers` | string[] | - | 転送アダプタ (常に `["basic"]` を想定) |
| `objects` | object[] | ✓ | 処理対象オブジェクトのリスト |
| `objects[].oid` | string | ✓ | SHA-256 ハッシュ (64 文字) |
| `objects[].size` | number | ✓ | オブジェクトサイズ (バイト) |
| `hash_algo` | string | - | `sha256` (デフォルト) |

**レスポンス (200 OK):**

```json
{
  "transfer": "basic",
  "objects": [
    {
      "oid": "4d7af9c6e8b123456...",
      "size": 1048576,
      "authenticated": true,
      "actions": {
        "upload": {
          "href": "https://s3.amazonaws.com/bucket/objects/owner/repo/4d7af9c6e8b123456...?X-Amz-...",
          "expires_in": 3600
        },
      }
    }
  ],
  "hash_algo": "sha256"
}
```

**エラーレスポンス:**

| コード | 説明 |
|---|---|
| 422 | リクエストボディが不正 |

**処理フロー:**
1. `operation = upload` の場合:
   - S3 で OID の存在確認 (HeadObject)
   - 存在しない場合のみ presigned PUT URL を生成 (有効期限: 3600 秒)
2. `operation = download` の場合:
   - S3 で OID の存在確認 (HeadObject)
   - 存在する場合のみ presigned GET URL を生成 (有効期限: 3600 秒)
   - 存在しない場合は `error.code = 404` をオブジェクトレベルで返す

---

## リポジトリ管理エンドポイント

Lambda: **repo-delete-initiator** (削除)

### DELETE `/repos/{owner}/{repo}`

リポジトリに属するすべての S3 オブジェクトを削除する。

**レスポンス (202 Accepted):**

```json
{
  "message": "Deletion started. This operation may take some time.",
  "owner": "myorg",
  "repo": "myrepo"
}
```

> **202 Accepted**: S3 の削除は非同期で実行されるため、リクエスト受付時点で 202 を返す。

**エラーレスポンス:**

| コード | 説明 |
|---|---|
| 422 | パスパラメータが不正 |

**処理フロー (repo-delete-initiator):**
1. SQS に削除ジョブメッセージを送信

```json
{
  "owner": "myorg",
  "repo": "myrepo",
  "s3_prefix": "objects/myorg/myrepo/",
  "continuation_token": null
}
```

2. 202 Accepted を返す

**処理フロー (repo-delete-worker):**
1. SQS メッセージから削除ジョブを受信
2. S3 `ListObjectsV2` (`Prefix = s3_prefix`, `ContinuationToken` を利用)
3. 取得した最大 1000 件を `DeleteObjects` でバッチ削除
4. `IsTruncated = true` の場合: SQS に継続メッセージを送信
5. `IsTruncated = false` の場合: 削除完了
6. エラー時: SQS の可視性タイムアウトを延長 or DLQ に移動

---

## API エンドポイント一覧

| メソッド | パス | Lambda | 説明 |
|---|---|---|---|
| `POST` | `/repos/{owner}/{repo}/info/lfs/objects/batch` | lfs-handler | LFS Batch API |
| `DELETE` | `/repos/{owner}/{repo}` | repo-delete-initiator | リポジトリ削除（非同期） |

---

## IAM ポリシー設計

### Lambda 実行ロール (lfs-handler)

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:GetObject", "s3:PutObject", "s3:HeadObject"],
      "Resource": "arn:aws:s3:::*-git-lfs-objects/objects/*"
    }
  ]
}
```

### Lambda 実行ロール (repo-delete-worker)

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": ["s3:ListBucket"],
      "Resource": "arn:aws:s3:::*-git-lfs-objects",
      "Condition": {
        "StringLike": { "s3:prefix": "objects/*" }
      }
    },
    {
      "Effect": "Allow",
      "Action": ["s3:DeleteObject"],
      "Resource": "arn:aws:s3:::*-git-lfs-objects/objects/*"
    },
    {
      "Effect": "Allow",
      "Action": ["sqs:SendMessage", "sqs:DeleteMessage", "sqs:GetQueueAttributes"],
      "Resource": "arn:aws:sqs:*:*:lfs-delete-queue"
    }
  ]
}
```

---

## presigned URL の仕様

| 項目 | 値 |
|---|---|
| 有効期限 | 3600 秒 (1 時間) |
| HTTP メソッド (upload) | `PUT` |
| HTTP メソッド (download) | `GET` |
| 署名バージョン | SigV4 |
| Content-Type ヘッダー | アップロード時に `application/octet-stream` を指定 |
| 最大ファイルサイズ | S3 の単一オブジェクト制限: 5 TB (multipart upload 推奨: > 100 MB) |

> presigned URL の発行には Lambda の IAM ロールが使用される。クライアントは Lambda から受け取った presigned URL を用いて直接 S3 に PUT/GET する。
