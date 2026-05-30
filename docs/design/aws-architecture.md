# AWS 構成図

## 全体構成

```mermaid
%%{init: {"theme": "neutral", "flowchart": {"nodeSpacing": 10, "rankSpacing": 30}} }%%
flowchart LR

git_client["git-lfs client<br/>GitHub Token 認証"]

subgraph aws["AWS"]
  subgraph layer1[" "]
    APIGW["API Gateway<br/>HTTP API"]
  end
  subgraph layer2[" "]
    AUTHORIZER["Lambda Authorizer<br/>rust-aws-lfs-authorizer"]
    LAMBDA["Lambda: LFS Handler<br/>rust-aws-lfs"]
  end
  subgraph layer3[" "]
    S3["S3 Bucket<br/>プライベート"]
    CF["CloudFront<br/>Signed URL 必須<br/>OAC + Key Group"]
  end
end

github_api["GitHub API<br/>api.github.com"]

git_client ~~~ layer1 ~~~ layer2 ~~~ layer3
AUTHORIZER ~~~ LAMBDA
S3 ~~~ CF

git_client ----->|"POST /batch or /verify"| APIGW
APIGW -->|"認可チェック"| AUTHORIZER
AUTHORIZER ---->|"GET /repos/owner/repo"| github_api
github_api ---->|"permissions.pull 確認"| AUTHORIZER
AUTHORIZER -->|"isAuthorized"| APIGW
APIGW -->|"認可済みリクエスト"| LAMBDA
LAMBDA -->|"HeadObject / Presigned URL"| S3
LAMBDA -->|"Signed URL 生成"| CF
git_client -.->|"PUT: Presigned URL"| S3
git_client -.->|"GET: Signed URL"| CF
CF -->|"OAC SigV4"| S3

style aws fill:#fff,color:#345,stroke:#345

classDef lambda     fill:#E8721C,stroke:#E8721C,color:#fff
classDef s3         fill:#3F8624,stroke:#3F8624,color:#fff
classDef cloudfront fill:#8C4FFF,stroke:#8C4FFF,color:#fff
classDef apigw      fill:#E7157B,stroke:#E7157B,color:#fff
classDef external   fill:#232F3E,stroke:#232F3E,color:#fff
classDef group      fill:none,stroke:none

class git_client,github_api external
class APIGW apigw
class AUTHORIZER,LAMBDA lambda
class S3 s3
class CF cloudfront
class layer1,layer2,layer3 group
```

---

## アップロードフロー

```mermaid
sequenceDiagram
    participant GIT as git-lfs client
    participant APIGW as API Gateway
    participant AUTH as Lambda Authorizer
    participant GH as GitHub API
    participant LFS as Lambda LFS Handler
    participant S3 as S3 Bucket

    GIT->>APIGW: POST /batch - operation: upload
    APIGW->>AUTH: 認可チェック (Authorization ヘッダー)
    AUTH->>GH: GET /repos/owner/repo
    GH-->>AUTH: 200 OK - permissions.pull: true
    AUTH-->>APIGW: isAuthorized: true
    APIGW->>LFS: リクエスト転送
    LFS->>S3: HeadObject (存在確認)
    alt オブジェクトが存在しない
        S3-->>LFS: 404 Not Found
        LFS->>S3: PutObject Presigned URL 生成 (有効期限: 3600秒)
        S3-->>LFS: Presigned PUT URL
        LFS-->>APIGW: 200 OK - actions.upload.href: presigned_url
        APIGW-->>GIT: レスポンス
        GIT->>S3: PUT (Presigned URL 経由で直接アップロード)
    else オブジェクトが既に存在する
        S3-->>LFS: 200 OK
        LFS-->>APIGW: 200 OK - authenticated: true (スキップ)
        APIGW-->>GIT: レスポンス
    end
```

---

## ダウンロードフロー

```mermaid
sequenceDiagram
    participant GIT as git-lfs client
    participant APIGW as API Gateway
    participant AUTH as Lambda Authorizer
    participant GH as GitHub API
    participant LFS as Lambda LFS Handler
    participant S3 as S3 Bucket
    participant CF as CloudFront

    GIT->>APIGW: POST /batch - operation: download
    APIGW->>AUTH: 認可チェック (Authorization ヘッダー)
    AUTH->>GH: GET /repos/owner/repo
    GH-->>AUTH: 200 OK - permissions.pull: true
    AUTH-->>APIGW: isAuthorized: true
    APIGW->>LFS: リクエスト転送
    LFS->>S3: HeadObject (存在確認)
    alt オブジェクトが存在する
        S3-->>LFS: 200 OK
        LFS->>LFS: CloudFront Signed URL 生成 (RSA 署名 + 有効期限)
        LFS-->>APIGW: 200 OK - actions.download.href: signed_url
        APIGW-->>GIT: レスポンス
        GIT->>CF: GET (Signed URL 経由)
        CF->>S3: OAC 署名リクエスト (SigV4)
        S3-->>CF: オブジェクトデータ
        CF-->>GIT: オブジェクトデータ
    else オブジェクトが存在しない
        S3-->>LFS: 404 Not Found
        LFS-->>APIGW: 200 OK - error.code: 404
        APIGW-->>GIT: レスポンス
    end
```

---

## AWS リソース一覧

| リソース | 名前/種別 | 役割 |
|---|---|---|
| API Gateway | HTTP API | LFS リクエストの受付・ルーティング |
| Lambda | rust-aws-lfs | LFS Batch/Verify API の処理、Presigned URL / Signed URL 生成 |
| Lambda | rust-aws-lfs-authorizer | GitHub API でトークン検証、リポジトリ read 権限確認 |
| S3 Bucket | (var.bucket_name) | LFS オブジェクト格納（プライベート、パブリックアクセス完全ブロック） |
| CloudFront | Distribution | S3 オブジェクトの CDN 配信（Signed URL 必須） |
| CloudFront OAC | Origin Access Control | S3 への署名付きアクセス制御（SigV4） |
| CloudFront Key Group | Public Key + Key Group | Signed URL の RSA 署名検証 |
| IAM Role | rust-aws-lfs-role | Lambda (LFS Handler) の実行ロール（S3 アクセス権限付き） |
| IAM Role | rust-aws-lfs-authorizer-role | Lambda Authorizer の実行ロール（基本実行権限のみ） |

---

## セキュリティ設計

```mermaid
%%{init: {"theme": "neutral", "flowchart": {"nodeSpacing": 10, "rankSpacing": 30}} }%%
flowchart TB

subgraph AUTHZ["認証・認可"]
  A["GitHub Token<br/>HTTP Basic Auth"] -->|"Lambda Authorizer で検証"| B["GitHub API<br/>/repos/owner/repo"]
  B -->|"permissions.pull == true"| C["アクセス許可"]
end

subgraph S3SEC["S3 保護"]
  D["パブリックアクセス<br/>完全ブロック"]
  E["バケットポリシー<br/>CloudFront OAC のみ許可"]
  F["Lambda IAM<br/>オブジェクト単位の最小権限"]
  D ~~~ E ~~~ F
end

subgraph DLSEC["ダウンロード保護"]
  G["CloudFront Signed URL<br/>RSA-SHA1 署名"]
  H["有効期限付き<br/>CLOUDFRONT_URL_TTL_SECS"]
  G --- H
end

AUTHZ ~~~ S3SEC ~~~ DLSEC
```
