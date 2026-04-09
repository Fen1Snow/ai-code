//! Work secret encoding/decoding utilities

use crate::types::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json;

/// Encode work secret to base64url
pub fn encode_work_secret(secret: &WorkSecret) -> Result<String, BridgeError> {
    let json = serde_json::to_string(secret)?;
    Ok(URL_SAFE_NO_PAD.encode(json.as_bytes()))
}

/// Decode work secret from base64url
pub fn decode_work_secret(encoded: &str) -> Result<WorkSecret, BridgeError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| BridgeError::InternalError(format!("Base64 decode error: {}", e)))?;
    
    let secret: WorkSecret = serde_json::from_slice(&decoded)?;
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_work_secret_roundtrip() {
        let secret = WorkSecret {
            version: 1,
            session_ingress_token: "test-token".to_string(),
            api_base_url: "https://api.example.com".to_string(),
            sources: vec![WorkSource {
                source_type: "git".to_string(),
                git_info: Some(GitInfo {
                    git_type: "github".to_string(),
                    repo: "owner/repo".to_string(),
                    ref_name: Some("main".to_string()),
                    token: None,
                }),
            }],
            auth: vec![WorkAuth {
                auth_type: "bearer".to_string(),
                token: "auth-token".to_string(),
            }],
            claude_code_args: None,
            mcp_config: None,
            environment_variables: Some(HashMap::from([
                ("NODE_ENV".to_string(), "production".to_string()),
            ])),
            use_code_sessions: None,
        };

        let encoded = encode_work_secret(&secret).unwrap();
        let decoded = decode_work_secret(&encoded).unwrap();

        assert_eq!(decoded.version, secret.version);
        assert_eq!(decoded.session_ingress_token, secret.session_ingress_token);
    }
}
