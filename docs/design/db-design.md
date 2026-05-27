# DB 設計書

## 使用データストア

| ストア | 用途 |
|---|---|
| AWS S3 | LFS オブジェクト（バイナリ）の格納 |
| AWS DynamoDB | リポジトリ管理テーブル（メタデータ） |

---

## S3 設計

### バケット構成

LFS オブジェクトはすべて **単一の S3 バケット** に格納する。

```
バケット名: <prefix>-git-lfs-objects  (例: myorg-git-lfs-objects)
```

### オブジェクトキー構造

Git LFS の仕様に準拠したコンテンツアドレス可能なパス構造を採用する。

```
objects/{owner}/{repo}/{oid[0:2]}/{oid[2:4]}/{oid}
```

| セグメント | 説明 | 例 |
|---|---|---|
| `objects/` | 固定プレフィックス | - |
| `{owner}` | リポジトリオーナー | `myorg` |
| `{repo}` | リポジトリ名 | `myrepo` |
| `{oid[0:2]}` | OID の先頭 2 文字（分散配置） | `4d` |
| `{oid[2:4]}` | OID の次の 2 文字（分散配置） | `7a` |
| `{oid}` | SHA-256 ハッシュ (64 文字) | `4d7a...` |

**例:**
```
objects/myorg/myrepo/4d/7a/4d7af9c6e8b12345...
```

> **理由**: OID の先頭 4 文字でディレクトリを分割することで、S3 のプレフィックス分散効果を得る（大規模運用時の LIST パフォーマンス向上）。

### S3 バケットポリシー

- Lambda の IAM Role のみアクセス可能
- presigned URL を用いた一時的なクライアントアクセスを許可
- パブリックアクセスはすべてブロック

### バケット設定

```json
{
  "VersioningConfiguration": { "Status": "Suspended" },
  "LifecycleConfiguration": {
    "Rules": [
      {
        "Id": "abort-incomplete-multipart",
        "Status": "Enabled",
        "AbortIncompleteMultipartUpload": { "DaysAfterInitiation": 7 }
      }
    ]
  },
  "CORSConfiguration": {
    "CORSRules": [
      {
        "AllowedOrigins": ["*"],
        "AllowedMethods": ["GET", "PUT"],
        "AllowedHeaders": ["*"],
        "MaxAgeSeconds": 3600
      }
    ]
  }
}
```

> CORS 設定は git-lfs クライアントが presigned URL に直接アクセスする際に必要。

---

## DynamoDB 設計

### テーブル: `git-lfs-repositories`

リポジトリのメタデータと S3 プレフィックスの対応を管理する。

#### キー設計

| 属性名 | 型 | 役割 | 例 |
|---|---|---|---|
| `pk` (Partition Key) | String | `REPO#{owner}#{repo}` | `REPO#myorg#myrepo` |
| `sk` (Sort Key) | String | `METADATA` (固定) | `METADATA` |

> シングルテーブル設計を採用。将来的にリポジトリ内のオブジェクトメタデータ管理など用途を拡張できる。

#### 属性一覧

| 属性名 | 型 | 必須 | 説明 |
|---|---|---|---|
| `pk` | S | ✓ | パーティションキー: `REPO#{owner}#{repo}` |
| `sk` | S | ✓ | ソートキー: `METADATA` |
| `owner` | S | ✓ | リポジトリオーナー名 |
| `repo` | S | ✓ | リポジトリ名 |
| `s3_prefix` | S | ✓ | S3 オブジェクトのプレフィックス: `objects/{owner}/{repo}/` |
| `status` | S | ✓ | リポジトリ状態: `active` / `deleting` |
| `created_at` | S | ✓ | 作成日時 (ISO 8601 形式: `2024-01-01T00:00:00Z`) |
| `updated_at` | S | ✓ | 更新日時 (ISO 8601 形式) |
| `description` | S | - | リポジトリの説明（オプション） |

#### レコード例

```json
{
  "pk": "REPO#myorg#myrepo",
  "sk": "METADATA",
  "owner": "myorg",
  "repo": "myrepo",
  "s3_prefix": "objects/myorg/myrepo/",
  "status": "active",
  "created_at": "2024-01-15T10:00:00Z",
  "updated_at": "2024-01-15T10:00:00Z",
  "description": "My large file repository"
}
```

#### `status` の状態遷移

```
                  登録
  (存在しない) ──────────► active
                              │
                              │ DELETE リクエスト
                              ▼
                           deleting
                              │
                              │ 削除Worker完了
                              ▼
                         (レコード削除)
```

### GSI (Global Secondary Index)

#### GSI-1: `status-index`

リポジトリ一覧取得・削除中ステータスの監視用。

| 属性 | 役割 |
|---|---|
| Partition Key | `status` |
| Sort Key | `created_at` |
| Projection | ALL |

**ユースケース**: `status = "active"` で全リポジトリ一覧を取得する。

### テーブル設定

```json
{
  "TableName": "git-lfs-repositories",
  "BillingMode": "PAY_PER_REQUEST",
  "PointInTimeRecoverySpecification": {
    "PointInTimeRecoveryEnabled": true
  }
}
```

> **PAY_PER_REQUEST** (オンデマンド): LFS サーバーのアクセスパターンは予測が難しいため、プロビジョンドキャパシティよりオンデマンドを推奨。

---

## データ整合性

### S3 とDynamoDB の整合性

S3 はリポジトリごとにプレフィックスで分離されているが、DynamoDB はそのマッピングを管理するのみ。

- DynamoDB に `status = "active"` のレコードがある → S3 オブジェクトへのアクセスを許可
- DynamoDB に `status = "deleting"` のレコードがある → 新規 LFS アクセスを 409 Conflict で拒否
- DynamoDB にレコードがない → 404 Not Found を返す

### 削除時の整合性

削除処理は S3 → DynamoDB の順で行う（S3 削除が完了してから DynamoDB レコードを削除）。

```
1. DynamoDB: status を "deleting" に更新 (楽観的ロック: condition expression)
2. S3: ListObjectsV2 + DeleteObjects のループ
3. S3: 全オブジェクト削除確認
4. DynamoDB: レコードを削除
```

---

## インデックス利用パターン

| 操作 | アクセスパターン | 使用インデックス |
|---|---|---|
| リポジトリ存在確認 | `pk = REPO#{owner}#{repo}` | Primary Key |
| リポジトリ詳細取得 | `pk = REPO#{owner}#{repo}, sk = METADATA` | Primary Key |
| 全リポジトリ一覧 | `status = active` | GSI-1 (status-index) |
| リポジトリ登録 | PutItem (条件: pk が存在しない) | Primary Key |
| ステータス更新 | UpdateItem (条件: 現状態チェック) | Primary Key |
| リポジトリ削除 | DeleteItem | Primary Key |
