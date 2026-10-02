use crate::app_log;
use anyhow::{Result, anyhow};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use tokio::sync::RwLock;

// ── api0 gateway OIDC verification ────────────────────────────────────────────
// The api0 gateway authenticates to us with a Google identity token minted by
// the tenant's service account (downstream auth mode `google_sa`).
// Same scheme cvenom uses, so solanize can live on any server api0 can reach.

const GOOGLE_CERTS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";

pub struct OidcVerifier {
    audience: String,
    /// Allowed signers. Several during a key rotation, comma-separated in
    /// SOLANIZE_OIDC_SERVICE_ACCOUNT: the gateway caches a signed token for up
    /// to an hour, so the old signer must stay accepted that long after a swap.
    service_accounts: Vec<String>,
    jwks: RwLock<Option<JwkSet>>,
}

impl OidcVerifier {
    /// `None` unless both the audience and the service account are configured.
    pub fn from_config(api0: &crate::config::Api0Config) -> Option<Self> {
        Some(Self {
            audience: api0.oidc_audience.clone()?,
            service_accounts: parse_accounts(api0.oidc_service_account.as_deref()?)?,
            jwks: RwLock::new(None),
        })
    }

    /// Cheap pre-check so a plain bearer secret never takes the OIDC path.
    pub fn looks_like_google_token(token: &str) -> bool {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let Some(payload) = token.split('.').nth(1) else { return false };
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload) else { return false };
        let Ok(claims) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return false };
        claims["iss"].as_str().is_some_and(|iss| iss.ends_with("accounts.google.com"))
    }

    /// Verifies signature, audience, issuer, expiry, and that the token was
    /// minted by the pinned service account.
    pub async fn verify(&self, token: &str) -> Result<()> {
        let kid = decode_header(token)?
            .kid
            .ok_or_else(|| anyhow!("OIDC token has no kid"))?;

        let jwk = match self.cached_key(&kid).await {
            Some(jwk) => jwk,
            None => {
                // Google rotates its keys; a miss means we need the new set.
                self.refresh().await?;
                self.cached_key(&kid)
                    .await
                    .ok_or_else(|| anyhow!("unknown OIDC key id '{}'", kid))?
            }
        };

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.audience]);
        validation.set_issuer(&["accounts.google.com", "https://accounts.google.com"]);

        let data = decode::<serde_json::Value>(token, &DecodingKey::from_jwk(&jwk)?, &validation)?;

        let email = data.claims["email"].as_str().unwrap_or_default();
        let verified = data.claims["email_verified"].as_bool().unwrap_or(false);
        let email_lc = email.to_lowercase();
        if !verified || !self.service_accounts.iter().any(|a| *a == email_lc) {
            return Err(anyhow!("OIDC token minted by '{}', not the api0 service account", email));
        }
        Ok(())
    }

    async fn cached_key(&self, kid: &str) -> Option<jsonwebtoken::jwk::Jwk> {
        self.jwks.read().await.as_ref()?.find(kid).cloned()
    }

    async fn refresh(&self) -> Result<()> {
        // Force IPv4 — Google answers OVH IPv6 ranges with 403 (same as cvenom).
        let client = reqwest::Client::builder()
            .local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
            .build()?;
        let jwks: JwkSet = client.get(GOOGLE_CERTS_URL).send().await?.error_for_status()?.json().await?;
        app_log!(info, "Refreshed Google OIDC keys ({} keys)", jwks.keys.len());
        *self.jwks.write().await = Some(jwks);
        Ok(())
    }
}


/// The comma-separated signer list, lowercased. `None` when it names nobody:
/// an empty allowlist must switch the OIDC path off, never accept everyone.
fn parse_accounts(value: &str) -> Option<Vec<String>> {
    let accounts: Vec<String> = value
        .split(',')
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
        .collect();
    (!accounts.is_empty()).then_some(accounts)
}

#[cfg(test)]
mod tests {
    use super::parse_accounts;

    #[test]
    fn signers_parse_as_a_list_and_an_empty_one_is_none() {
        assert_eq!(
            parse_accounts(" Old@p.iam.gserviceaccount.com, new@p.iam.gserviceaccount.com ,"),
            Some(vec!["old@p.iam.gserviceaccount.com".to_string(), "new@p.iam.gserviceaccount.com".to_string()])
        );
        assert_eq!(parse_accounts(" , "), None);
        assert_eq!(parse_accounts(""), None);
    }
}
