#!/usr/bin/env bash
# Lambda バイナリをビルドして S3 にアップロードするスクリプト。
# 使い方: ./scripts/upload-artifacts.sh <artifacts-bucket-name> [region]
set -euo pipefail

BUCKET="${1:?使い方: $0 <artifacts-bucket-name> [region]}"
REGION="${2:-ap-northeast-1}"

echo "▶ Lambda バイナリをビルド中..."
cargo lambda build --release

echo "▶ ZIP ファイルを作成中..."
LAMBDA_DIR="target/lambda"

(cd "$LAMBDA_DIR/rust-aws-lfs"           && zip -j ../rust-aws-lfs.zip bootstrap)
(cd "$LAMBDA_DIR/rust-aws-lfs-authorizer" && zip -j ../rust-aws-lfs-authorizer.zip bootstrap)

echo "▶ S3 にアップロード中 (s3://$BUCKET/)..."
aws s3 cp "$LAMBDA_DIR/rust-aws-lfs.zip"            "s3://$BUCKET/rust-aws-lfs.zip"            --region "$REGION"
aws s3 cp "$LAMBDA_DIR/rust-aws-lfs-authorizer.zip" "s3://$BUCKET/rust-aws-lfs-authorizer.zip" --region "$REGION"

echo ""
echo "✅ アップロード完了！"
echo ""
echo "次のステップ:"
echo "  1. AWS コンソール → CloudFormation → スタックの作成"
echo "  2. cloudformation/template.yaml をアップロード"
echo "  3. パラメータを入力:"
echo "       ArtifactsBucketName = $BUCKET"
echo "       LFSBucketName       = <任意のバケット名>"
