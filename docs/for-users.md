# 利用者向けガイド — AWS へのデプロイ（CloudFormation）

S3・Lambda・API Gateway・CloudFront をまとめて構築します。**RSA キーペアはスタック作成時に自動生成**されます。

## 事前準備：Lambda バイナリのアップロード

CloudFormation はコードのコンパイルができないため、先にビルド済みバイナリを S3 に置く必要があります。

### 1. ファイルをダウンロード

[GitHub Releases](https://github.com/rahinaku/rust-aws-git-lfs/releases/latest) から以下の 3 ファイルをダウンロードします：

- `rust-aws-lfs.zip`
- `rust-aws-lfs-authorizer.zip`
- `template.yaml`

### 2. アーティファクト用 S3 バケットを作成

1. AWS コンソール → **S3** → 「バケットを作成」
2. バケット名を入力（例：`my-lfs-artifacts`）、リージョンは `ap-northeast-1` を選択
3. その他はデフォルトのまま「バケットを作成」

### 3. ZIP ファイルをアップロード

1. 作成したバケットを開き「**アップロード**」をクリック
2. ダウンロードした `rust-aws-lfs.zip` と `rust-aws-lfs-authorizer.zip` を追加
3. 「アップロード」をクリック

## CloudFormation でスタックを作成

1. AWS コンソール → **CloudFormation** → 「スタックの作成」→「新しいリソースを使用」
2. 「テンプレートファイルのアップロード」→ ダウンロードした `template.yaml` を選択 → 「次へ」
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

1. [GitHub Releases](https://github.com/rahinaku/rust-aws-git-lfs/releases/latest) から最新の `rust-aws-lfs.zip` と `rust-aws-lfs-authorizer.zip` をダウンロード
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
