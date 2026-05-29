use base64::{engine::general_purpose::STANDARD, Engine};
use rsa::{pkcs8::DecodePrivateKey, pkcs1v15::SigningKey, RsaPrivateKey};
use rsa::signature::SignatureEncoding;
use sha1::Sha1;

pub struct CloudFrontSigner {
    signing_key: SigningKey<Sha1>,
    key_pair_id: String,
    domain: String,
}

impl CloudFrontSigner {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let domain = std::env::var("CLOUDFRONT_DOMAIN")?;
        let key_pair_id = std::env::var("CLOUDFRONT_KEY_PAIR_ID")?;
        let private_key_pem = std::env::var("CLOUDFRONT_PRIVATE_KEY")?;
        let private_key = RsaPrivateKey::from_pkcs8_pem(&private_key_pem)?;
        let signing_key = SigningKey::<Sha1>::new(private_key);
        Ok(Self { signing_key, key_pair_id, domain })
    }

    /// CloudFront canned policy の Signed URL を生成する。
    /// `s3_key` は S3 オブジェクトキー（例: `objects/owner/repo/abc123`）。
    /// `expires_unix` は Unix タイムスタンプ（秒）。
    pub fn sign(&self, s3_key: &str, expires_unix: u64) -> String {
        let resource_url = format!("https://{}/{}", self.domain, s3_key);

        // CloudFront canned policy（コンパクトな JSON、空白なし）
        let policy = format!(
            r#"{{"Statement":[{{"Resource":"{resource_url}","Condition":{{"DateLessThan":{{"AWS:EpochTime":{expires_unix}}}}}}}]}}"#
        );

        use rsa::signature::Signer;
        let signature = self.signing_key.sign(policy.as_bytes());
        let sig_bytes = signature.to_bytes();

        // CloudFront は標準 URLセーフ Base64 (RFC 4648) と異なる独自の文字置換を要求する。
        // https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/private-content-creating-signed-url-canned-policy.html
        let sig_encoded = STANDARD
            .encode(sig_bytes.as_ref())
            .replace('+', "-")
            .replace('/', "~")
            .replace('=', "_");

        format!(
            "{resource_url}?Expires={expires_unix}&Signature={sig_encoded}&Key-Pair-Id={}",
            self.key_pair_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_signer() -> CloudFrontSigner {
        // テスト用の固定 RSA キー（2048bit、テスト専用）
        let pem = "-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7o4qne60TB3wo
pHCGJTEGrjq9DMvBXMuMEHKRqCR/MFbKSR8hVcyPFGdKU6FGrVoJBxIbsq4OLnrv
LEdPAzCeRKS3UlNOAKAlBwxN07o6UhEH1pYkCjCxl/DFB2Yjot72FIwGHqFJdU+8
oiqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVF
qXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFq
XJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFqXJVFq
XJVFAgMBAAECggEADummy...
-----END PRIVATE KEY-----";
        // NOTE: 実際のテストでは有効なキーが必要。ここでは構造確認のみのダミーキー。
        // テスト可能な有効キーはCIの秘密変数または test fixture から読み込む。
        let _ = pem;
        panic!("テスト用フィクスチャキーを設定してください")
    }

    #[test]
    #[ignore = "テスト用 RSA フィクスチャキーが必要"]
    fn signed_url_contains_required_query_params() {
        let signer = make_test_signer();
        let url = signer.sign("objects/owner/repo/abc123", 9_999_999_999);
        assert!(url.contains("Expires=9999999999"));
        assert!(url.contains("Key-Pair-Id="));
        assert!(url.contains("Signature="));
        assert!(url.starts_with("https://"));
        assert!(url.contains("/objects/owner/repo/abc123?"));
    }
}
