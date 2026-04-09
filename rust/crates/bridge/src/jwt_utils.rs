//! JWT utilities for bridge authentication

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT claims for session ingress token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIngressClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub session_id: String,
    pub environment_id: String,
}

/// JWT claims for environment secret
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub environment_id: String,
}

/// Create a session ingress token
pub fn create_session_token(
    secret: &str,
    session_id: &str,
    environment_id: &str,
    expires_in_secs: u64,
) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;

    let claims = SessionIngressClaims {
        sub: format!("session:{}", session_id),
        exp: now + expires_in_secs as usize,
        iat: now,
        session_id: session_id.to_string(),
        environment_id: environment_id.to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

/// Verify a session ingress token
pub fn verify_session_token(
    secret: &str,
    token: &str,
) -> Result<SessionIngressClaims, String> {
    let token_data = decode::<SessionIngressClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| e.to_string())?;

    Ok(token_data.claims)
}

/// Create an environment token
pub fn create_environment_token(
    secret: &str,
    environment_id: &str,
    expires_in_secs: u64,
) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;

    let claims = EnvironmentClaims {
        sub: format!("environment:{}", environment_id),
        exp: now + expires_in_secs as usize,
        iat: now,
        environment_id: environment_id.to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

/// Verify an environment token
pub fn verify_environment_token(
    secret: &str,
    token: &str,
) -> Result<EnvironmentClaims, String> {
    let token_data = decode::<EnvironmentClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| e.to_string())?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_token_roundtrip() {
        let secret = "test-secret";
        let session_id = "session-123";
        let environment_id = "env-456";

        let token = create_session_token(secret, session_id, environment_id, 3600).unwrap();
        let claims = verify_session_token(secret, &token).unwrap();

        assert_eq!(claims.session_id, session_id);
        assert_eq!(claims.environment_id, environment_id);
    }

    #[test]
    fn test_environment_token_roundtrip() {
        let secret = "test-secret";
        let environment_id = "env-456";

        let token = create_environment_token(secret, environment_id, 3600).unwrap();
        let claims = verify_environment_token(secret, &token).unwrap();

        assert_eq!(claims.environment_id, environment_id);
    }
}
