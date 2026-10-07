//! Provider endpoints, and credential hygiene around them.
//!
//! Two rules live here, because both were broken in the same place:
//!
//! * a URL path built from a value the caller chose is assembled through `Url`,
//!   which percent-encodes every segment. The model identifier comes straight
//!   from the request body, and an instance whose model allowlist is empty — the
//!   default — accepts any string; interpolated raw, a `/`, a `?` or a `#` in it
//!   re-pointed the request at another endpoint of the provider, or truncated
//!   the URL and dropped what followed;
//! * a credential never travels in a URL when the provider accepts a header,
//!   and any text we quote back — a provider's error body, an address — goes
//!   through `redact` first. An error raised down here reaches the client as the
//!   body of a 503 (`AssistantError::OllamaUnavailable` renders its message),
//!   so everything it carries must be assumed public.

use anyhow::{anyhow, Context, Result};
use url::Url;

/// Builds `base` + `segments`, percent-encoding each segment.
///
/// Encoding is what makes this safe: `/`, `?`, `#` and `%` inside a segment come
/// out escaped, so a hostile value can only ever name one path segment. A `:`
/// is left alone — the sub-resource syntax some providers use (`…/models/
/// <model>:<method>`) needs it, and a colon cannot leave the path.
pub fn endpoint(base: &str, segments: &[&str]) -> Result<Url> {
    let mut url = Url::parse(base.trim()).context("adresse du fournisseur invalide")?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| anyhow!("adresse du fournisseur sans chemin"))?;
        // `https://host` parses with a single empty segment; dropping it avoids
        // a doubled separator once the real segments are appended.
        path.pop_if_empty();
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url)
}

/// A URL as it may safely be shown — scheme, host, port and path, nothing else.
///
/// The query string is where a provider asks for a key, and a URL may also embed
/// `user:password@`. Neither belongs in a log line or in an error the client
/// reads, so both are dropped rather than trusted to be empty.
pub fn shown(url: &Url) -> String {
    let mut safe = url.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    let _ = safe.set_username("");
    let _ = safe.set_password(None);
    safe.to_string()
}

/// Blanks `secret` out of `text`.
///
/// Belt and braces for text we did not write: a provider's error body is quoted
/// back to the user, and a provider that echoes the request it refused would
/// echo the credential with it. A secret under 8 characters is left alone — it
/// is not a real credential, and masking it would mangle unrelated words.
pub fn redact(text: &str, secret: &str) -> String {
    let secret = secret.trim();
    if secret.len() < 8 {
        return text.to_string();
    }
    text.replace(secret, "***")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AIza-cle-tres-secrete";

    #[test]
    fn a_segment_cannot_escape_its_place_in_the_path() {
        let url = endpoint(
            "https://fournisseur.example",
            &["v1beta", "models", "../../admin:streamGenerateContent"],
        )
        .expect("URL constructible");
        // The traversal is encoded, so the request still targets /v1beta/models/…
        assert!(url.path().starts_with("/v1beta/models/"));
        assert!(!url.path().contains("/../"));
        let depth = url.path().split('/').filter(|s| !s.is_empty()).count();
        assert_eq!(depth, 3, "chemin inattendu : {}", url.path());
    }

    #[test]
    fn a_segment_cannot_open_a_query_string_or_a_fragment() {
        let url = endpoint(
            "https://fournisseur.example",
            &["v1beta", "models", "modele?key=vole&x=:streamGenerateContent"],
        )
        .expect("URL constructible");
        assert_eq!(url.query(), None);
        assert_eq!(url.fragment(), None);

        let url = endpoint("https://fournisseur.example", &["v1beta", "modele#tronque"])
            .expect("URL constructible");
        assert_eq!(url.fragment(), None);
    }

    #[test]
    fn the_sub_resource_colon_survives_encoding() {
        let url = endpoint(
            "https://fournisseur.example",
            &["v1beta", "models", "gemini-2.0-flash:streamGenerateContent"],
        )
        .expect("URL constructible");
        assert_eq!(
            url.path(),
            "/v1beta/models/gemini-2.0-flash:streamGenerateContent"
        );
    }

    #[test]
    fn a_base_with_or_without_a_trailing_slash_gives_the_same_path() {
        let a = endpoint("https://fournisseur.example", &["v1", "chat"]).expect("URL");
        let b = endpoint("https://fournisseur.example/", &["v1", "chat"]).expect("URL");
        assert_eq!(a.as_str(), b.as_str());
        assert_eq!(a.path(), "/v1/chat");
    }

    #[test]
    fn a_base_carrying_a_prefix_keeps_it() {
        let url = endpoint("https://fournisseur.example/api", &["v1", "chat"]).expect("URL");
        assert_eq!(url.path(), "/api/v1/chat");
    }

    #[test]
    fn a_malformed_base_is_refused_rather_than_guessed() {
        assert!(endpoint("pas une adresse", &["v1"]).is_err());
    }

    #[test]
    fn a_shown_url_carries_neither_query_nor_credentials() {
        let url = Url::parse(&format!("https://user:{KEY}@fournisseur.example/v1?key={KEY}"))
            .expect("URL de test");
        let shown = shown(&url);
        assert!(!shown.contains(KEY), "clé présente dans « {shown} »");
        assert!(!shown.contains("user"));
        assert!(!shown.contains('?'));
        assert!(shown.starts_with("https://fournisseur.example/v1"));
    }

    #[test]
    fn a_quoted_provider_body_never_carries_the_key_back() {
        let body = format!("{{\"error\":\"API key not valid: {KEY}\"}}");
        let safe = redact(&body, KEY);
        assert!(!safe.contains(KEY), "clé présente dans « {safe} »");
        assert!(safe.contains("API key not valid"));
    }

    #[test]
    fn redaction_leaves_a_value_too_short_to_be_a_credential_alone() {
        assert_eq!(redact("le modele a echoue", "a"), "le modele a echoue");
        assert_eq!(redact("rien a masquer", ""), "rien a masquer");
    }
}
