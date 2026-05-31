[English](../../for-users.md) | [日本語](for-users.md)

# 利用者向けガイド — AWS へのデプロイ（CloudFormation）

S3・Lambda・API Gateway・CloudFront をまとめて構築します。**RSA キーペアはスタック作成時に自動生成**されます。

## 事前準備：Lambda バイナリのアップロード

CloudFormation はコードのコンパイルができないため、先にビルド済みバイナリを S3 に置く必要があります。

### 1. ファイルをダウンロード

[GitHub Releases](https://github.com/script202602/rust-aws-git-lfs/releases/latest) から以下の 3 ファイルをダウンロードします：

- `rust-aws-lfs.zip`
- `rust-aws-lfs-authorizer.zip`
- `template.yaml`

### 2. アーティファクト用 S3 バケットを作成

1. AWS コンソール → **S3** → 「バケットを作成」
2. バケット名を入力（例：`my-lfs-artifacts`）、リージョンはスタックを作成するリージョンと**同じ**リージョンを選択
3. その他はデフォルトのまま「バケットを作成」

### 3. ZIP ファイルをアップロード

1. 作成したバケットを開き「**アップロード**」をクリック
2. ダウンロードした `rust-aws-lfs.zip` と `rust-aws-lfs-authorizer.zip` を追加
3. 「アップロード」をクリック

## CloudFormation でスタックを作成

1. AWS コンソール → **CloudFormation** → 「スタックの作成」→「新しいリソースを使用」
2. 「テンプレートファイルのアップロード」→ ダウンロードした `template.yaml` を選択 → 「次へ」
3. パラメータを入力：

   **必須：**

   | パラメータ | 入力値 | 説明 |
   |---|---|---|
   | **ArtifactsBucketName** | `my-lfs-artifacts` | ZIP をアップロードした S3 バケット名 |
   | **BudgetAlertEmail** | `your@example.com` | コストアラートの通知先メールアドレス |

   **任意（デフォルトのままでも動作します）：**

   | パラメータ | デフォルト | 説明 |
   |---|---|---|
   | MainFunctionS3Key | `rust-aws-lfs.zip` | メイン Lambda ZIP の S3 オブジェクトキー |
   | AuthorizerFunctionS3Key | `rust-aws-lfs-authorizer.zip` | Authorizer Lambda ZIP の S3 オブジェクトキー |
   | CloudFrontSignedUrlTTL | `3600` | Signed URL の有効期限（秒） |
   | LambdaMaxConcurrency | `-1` | Lambda 関数ごとの最大同時実行数。**通常は `-1`（デフォルト）のまま変更不要。** API Gateway のスロットリング設定がリクエスト数を制限するため、Lambda 側の同時実行数制限は不要。設定するとアカウントの同時実行上限の制約でエラーになる場合がある（下記参照）。 |
   | ApiThrottlingRateLimit | `10` | API Gateway の持続リクエストレート上限（req/s） |
   | ApiThrottlingBurstLimit | `50` | API Gateway のバーストリクエスト上限 |
   | MonthlyBudgetLimit | `10` | 月次コスト予算の上限（USD）。実績 80%・100% および予測 80% 超過時にメール通知。 |
   | LogRetentionDays | `30` | CloudWatch Logs の保持日数 |
   | CloudFrontGeoRestrictionLocations | *(空)* | CloudFront ホワイトリストの国コード（ISO 3166-1 alpha-2、カンマ区切り）。例：`JP,US`。空で制限なし。 |

   > **⚠️ LambdaMaxConcurrency の設定制限**
   >
   > このプロジェクトは Lambda 関数が **2 つ**（main + authorizer）あり、両方に同じ値が適用されます。
   > そのため、アカウント内で消費される予約済み同時実行数は **`LambdaMaxConcurrency × 2`** になります。
   >
   > **エラーになる条件:**
   > ```
   > アカウントの同時実行上限
   >   − アカウント内の全 Lambda の予約済み同時実行数の合計
   >   − LambdaMaxConcurrency × 2
   > < 10（最低限必要な未予約分）
   > ```
   >
   > **新規アカウントや無料枠アカウントは上限が 10 の場合があります。**
   > この場合、正の値を設定すると `TooManyRequestsException` エラーが必ず発生するため、
   > `-1`（デフォルト）のまま使用してください。
   > コスト保護は `ApiThrottlingRateLimit` / `ApiThrottlingBurstLimit` のスロットリングで行います。
   >
   > **現在の上限を確認する方法:**
   >
   > 1. AWS コンソール → **Lambda**
   > 2. **ダッシュボード**（Lambda を開いた最初の画面）のリージョン別リソース欄を確認
   > 3. **「予約されていないアカウントの同時実行」** が `LambdaMaxConcurrency × 2 + 10` 以上あることを確認
   >
   > 上限を引き上げたい場合：AWS コンソール → **Service Quotas** →「AWS のサービス」→ **「AWS Lambda」** → **「同時実行数（Concurrent executions）」** →「クォータの引き上げをリクエスト」
   >
   > **上限が十分にある場合の推奨値:** `ApiThrottlingBurstLimit`（デフォルト 50）と同じ値にすることで、API Gateway と Lambda の上限が一致し、過剰なスロットリングを防げます。

4. 「次へ」→「次へ」→ **「AWS CloudFormation によって IAM リソースが作成される場合があることを承認します」にチェック** → 「スタックの作成」
5. 「イベント」タブで進捗を確認（完了まで約 5〜10 分）

## 完了後の確認

スタックの「出力」タブに以下が表示されます：

```
LFSBaseUrl      → https://xxxxxxxxxx.execute-api.<region>.amazonaws.com
LFSUrlExample   → https://xxxxxxxxxx.execute-api.<region>.amazonaws.com/<owner>/<repo>/info/lfs
CloudFrontDomain → d111111abcdef8.cloudfront.net
```

`LFSUrlExample` の `<owner>/<repo>` を実際の GitHub リポジトリ名に置き換えて git-lfs を設定します：

```bash
git config lfs.url https://<API_ID>.execute-api.<region>.amazonaws.com/<owner>/<repo>/info/lfs
```

`git lfs push` / `git lfs pull` の初回実行時に認証プロンプトが表示されます：

```
Username: <GitHub ユーザー名>
Password: <GitHub Personal Access Token（repo スコープ）>
```

> GitHub の Personal Access Token は [Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) で `repo` スコープを付けて発行してください。

## コード更新時の再デプロイ

1. [GitHub Releases](https://github.com/script202602/rust-aws-git-lfs/releases/latest) から最新の `rust-aws-lfs.zip` と `rust-aws-lfs-authorizer.zip` をダウンロード
2. AWS コンソール → **S3** → アーティファクトバケット → 「アップロード」で両ファイルを上書き
3. AWS コンソール → **CloudFormation** → スタックを選択 → 「更新」→「既存のテンプレートを使用」→ パラメータはそのまま → 「スタックの更新」

## スタックの削除

### 1. S3 バケットを空にして削除する

CloudFormation はオブジェクトが残っているバケットを削除できないため、先に S3 コンソールから空にします。

**LFS バケット（`my-lfs-bucket-yourname`）:**

1. AWS コンソール → **S3** → バケット一覧から `my-lfs-bucket-yourname` を選択
2. 「**空にする**」ボタンをクリック → テキストボックスに `permanently delete` と入力 → 「空にする」
3. 「**削除**」ボタンをクリック → バケット名を入力 → 「バケットを削除」

**アーティファクトバケット（`my-lfs-artifacts`）も同様に削除:**

1. S3 バケット一覧から `my-lfs-artifacts` を選択
2. 「**空にする**」→「**削除**」の順に実行

### 2. CloudFormation スタックを削除

1. AWS コンソール → **CloudFormation** → スタック一覧からスタックを選択
2. 「**削除**」ボタンをクリック → 確認ダイアログで「スタックの削除」
3. 「イベント」タブで進捗を確認（完了まで約 5〜10 分）

> スタック削除により、Lambda 関数・API Gateway・CloudFront・IAM ロール・SSM パラメータ（RSA キーペア）がすべて自動削除されます。
