# DB 設計書

## 使用データストア

| ストア | 用途 |
|---|---|
| AWS S3 | LFS オブジェクト（バイナリ）の格納 |

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
objects/{owner}/{repo}/{oid}
```

| セグメント | 説明 | 例 |
|---|---|---|
| `objects/` | 固定プレフィックス | - |
| `{owner}` | リポジトリオーナー | `myorg` |
| `{repo}` | リポジトリ名 | `myrepo` |
| `{oid}` | SHA-256 ハッシュ (64 文字) | `4d7af9c6e8b12345...` |

**例:**
```
objects/myorg/myrepo/4d7af9c6e8b12345...
```

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
