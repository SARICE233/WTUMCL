use serde_json::Value;
use std::collections::HashSet;

/// Normalize the `skinDomains` field of authlib-injector metadata so that
/// entries written as URLs (for example `https://wtumc.top:64053`) match the
/// bare host rule format required by authlib-injector.
///
/// Wildcard rules (`*`, `*.example.com`, `.example.com`) are kept verbatim,
/// order is preserved, duplicates are removed and every other top-level field
/// is left untouched. Invalid JSON or an unexpected `skinDomains` shape makes
/// the function return the input unchanged.
pub fn normalize_authlib_metadata(meta: &str) -> String {
  let Ok(mut value) = serde_json::from_str::<Value>(meta) else {
    return meta.to_string();
  };
  let Some(object) = value.as_object_mut() else {
    return meta.to_string();
  };
  let domains = match object.get_mut("skinDomains") {
    Some(Value::Array(domains)) => std::mem::take(domains),
    _ => return meta.to_string(),
  };

  let mut seen = HashSet::new();
  let mut normalized = Vec::with_capacity(domains.len());
  for domain in domains {
    let Some(raw) = domain.as_str() else {
      continue;
    };
    let Some(host) = normalize_skin_domain(raw) else {
      continue;
    };
    if seen.insert(host.clone()) {
      normalized.push(Value::String(host));
    }
  }

  object.insert("skinDomains".to_string(), Value::Array(normalized));
  serde_json::to_string(&value).unwrap_or_else(|_| meta.to_string())
}

/// Reduce a single `skinDomains` entry to the host rule understood by
/// authlib-injector. Returns `None` for entries that carry no usable host.
fn normalize_skin_domain(raw: &str) -> Option<String> {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return None;
  }

  // Wildcard rules are matching patterns, not URLs, and must stay verbatim.
  if trimmed.starts_with('.') || trimmed.contains('*') {
    return Some(trimmed.to_string());
  }

  let mut rest = trimmed;
  // Drop any scheme (case-insensitive by construction: only the delimiter is
  // searched for, the scheme itself is discarded).
  if let Some(index) = rest.find("://") {
    rest = &rest[index + 3..];
  }

  let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
  let mut authority = &rest[..authority_end];

  // Drop userinfo.
  if let Some(at) = authority.rfind('@') {
    authority = &authority[at + 1..];
  }

  let host = if let Some(stripped) = authority.strip_prefix('[') {
    // Bracketed IPv6 literal, optionally followed by a port.
    match stripped.find(']') {
      Some(end) => &authority[..end + 2],
      None => authority,
    }
  } else if authority.matches(':').count() > 1 {
    // Unbracketed IPv6 literal: there is no port to strip.
    authority
  } else if let Some((host, port)) = authority.rsplit_once(':') {
    if port.chars().all(|c| c.is_ascii_digit()) {
      host
    } else {
      authority
    }
  } else {
    authority
  };

  let host = host.trim_end_matches('.').to_lowercase();
  if host.is_empty() { None } else { Some(host) }
}

#[cfg(test)]
mod tests {
  use super::normalize_authlib_metadata;
  use serde_json::{Value, json};

  const WTUMC_METADATA: &str = r#"{
    "meta": {
      "serverName": "WTUMC",
      "implementationName": "authlib-injector",
      "implementationVersion": "1.2.5"
    },
    "skinDomains": ["https://wtumc.top:64053"],
    "signaturePublickey": "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuNoPaddingForTestOnly"
  }"#;

  fn normalized_domains(meta: &str) -> Vec<String> {
    let parsed: Value = serde_json::from_str(&normalize_authlib_metadata(meta))
      .expect("normalized metadata should stay valid JSON");
    parsed["skinDomains"]
      .as_array()
      .expect("skinDomains should stay an array")
      .iter()
      .map(|domain| {
        domain
          .as_str()
          .expect("normalized skinDomains entries should be strings")
          .to_string()
      })
      .collect()
  }

  #[test]
  fn normalizes_real_wtumc_metadata() {
    let output = normalize_authlib_metadata(WTUMC_METADATA);
    let parsed: Value = serde_json::from_str(&output).expect("output should be valid JSON");
    let original: Value =
      serde_json::from_str(WTUMC_METADATA).expect("fixture should be valid JSON");

    assert_eq!(parsed["skinDomains"], json!(["wtumc.top"]));
    assert_eq!(parsed["signaturePublickey"], original["signaturePublickey"]);
    assert_eq!(
      parsed["signaturePublickey"],
      json!("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAuNoPaddingForTestOnly")
    );
    assert_eq!(parsed["meta"], original["meta"]);
  }

  #[test]
  fn leaves_already_bare_domains_unchanged() {
    let meta = r#"{"skinDomains":["wtumc.top","example.com","mc.example.org"]}"#;
    assert_eq!(
      normalized_domains(meta),
      vec!["wtumc.top", "example.com", "mc.example.org"]
    );
  }

  #[test]
  fn strips_path_query_and_fragment() {
    let meta = r#"{"skinDomains":["https://wtumc.top:64053/api/yggdrasil?x=1#frag","http://example.com/some/path"]}"#;
    assert_eq!(normalized_domains(meta), vec!["wtumc.top", "example.com"]);
  }

  #[test]
  fn strips_scheme_variants_and_lowercases() {
    let meta =
      r#"{"skinDomains":["HTTP://Example.COM","https://EXAMPLE.com:443","ftp://Example.COM:21"]}"#;
    assert_eq!(normalized_domains(meta), vec!["example.com"]);
  }

  #[test]
  fn strips_userinfo_and_port() {
    let meta = r#"{"skinDomains":["https://user:secret@wtumc.top:64053/path"]}"#;
    assert_eq!(normalized_domains(meta), vec!["wtumc.top"]);
  }

  #[test]
  fn keeps_wildcard_rules_verbatim() {
    let meta = r#"{"skinDomains":[".example.com","*.example.com","*","https://wtumc.top:64053"]}"#;
    assert_eq!(
      normalized_domains(meta),
      vec![".example.com", "*.example.com", "*", "wtumc.top"]
    );
  }

  #[test]
  fn deduplicates_preserving_first_occurrence_order() {
    let meta = r#"{"skinDomains":["https://wtumc.top:64053","WTUMC.TOP","another.example.com","wtumc.top"]}"#;
    assert_eq!(
      normalized_domains(meta),
      vec!["wtumc.top", "another.example.com"]
    );
  }

  #[test]
  fn is_idempotent() {
    let once = normalize_authlib_metadata(WTUMC_METADATA);
    let twice = normalize_authlib_metadata(&once);
    assert_eq!(once, twice);
    assert_eq!(normalized_domains(&once), vec!["wtumc.top"]);
  }

  #[test]
  fn returns_input_for_invalid_or_missing_skin_domains() {
    let invalid_json = "{not json";
    assert_eq!(normalize_authlib_metadata(invalid_json), invalid_json);

    let empty = "";
    assert_eq!(normalize_authlib_metadata(empty), empty);

    let missing = r#"{"meta":{"serverName":"WTUMC"}}"#;
    assert_eq!(normalize_authlib_metadata(missing), missing);

    let non_array = r#"{"skinDomains":"wtumc.top"}"#;
    assert_eq!(normalize_authlib_metadata(non_array), non_array);

    let non_object = r#"["https://wtumc.top:64053"]"#;
    assert_eq!(normalize_authlib_metadata(non_object), non_object);
  }

  #[test]
  fn skips_non_string_and_empty_entries() {
    let meta = r#"{"skinDomains":[1,true,null,"","   ","https://wtumc.top:64053",".example.com"]}"#;
    assert_eq!(normalized_domains(meta), vec!["wtumc.top", ".example.com"]);
  }

  #[test]
  fn handles_empty_skin_domains_array() {
    let meta = r#"{"meta":{"serverName":"WTUMC"},"skinDomains":[]}"#;
    assert_eq!(normalized_domains(meta), Vec::<String>::new());
  }
}
