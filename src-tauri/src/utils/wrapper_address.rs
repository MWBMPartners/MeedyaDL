// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// The wrapper's decryption address (#1433).
// =========================================
//
// The setting `wrapper_decrypt_ip` is either empty (AUTOMATIC) or an
// address the person typed. Whenever the address is needed, it is worked
// out here, in one place: the typed value if there is one, otherwise the
// host of the wrapper address in use plus the standard decryption port,
// otherwise the local default. Everything that reads the decryption
// address (download options, the config file, the reachability check) goes
// through `effective_decrypt_address`, and everything that needs host and
// port apart goes through `split_decrypt_address`.
//
// The page has a matching helper for the placeholder it shows
// (`src/lib/wrapperDecryptAddress.ts`); both are tested against the same
// cases.

/// The standard decryption port of the wrapper.
pub const DECRYPT_PORT: u16 = 10020;

/// The address used when nothing better is known.
pub const FALLBACK_DECRYPT_ADDRESS: &str = "127.0.0.1:10020";

/// The host of a wrapper address plus the standard decryption port, or
/// `None` if the text is not a usable web address. An address without a
/// scheme is read as `http://`. An IPv6 host keeps its brackets.
pub fn derive_decrypt_address(wrapper_url: &str) -> Option<String> {
    let text = wrapper_url.trim();
    if text.is_empty() {
        return None;
    }
    let with_scheme = if text.contains("://") {
        text.to_string()
    } else {
        format!("http://{text}")
    };
    let parsed = url::Url::parse(&with_scheme).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?;
    if host.is_empty() {
        return None;
    }
    Some(format!("{host}:{DECRYPT_PORT}"))
}

/// The decryption address to use: the typed one if there is one, else
/// the one worked out from the wrapper address in use (`wrapper_url` for
/// the newer wrapper, `wrapper_account_url` for the older one), else the
/// local default.
pub fn effective_decrypt_address(
    stored: &str,
    wrapper_url: &str,
    wrapper_account_url: &str,
    newer_wrapper: bool,
) -> String {
    let typed = stored.trim();
    if !typed.is_empty() {
        return typed.to_string();
    }
    let source = if newer_wrapper {
        wrapper_url
    } else {
        wrapper_account_url
    };
    derive_decrypt_address(source).unwrap_or_else(|| FALLBACK_DECRYPT_ADDRESS.to_string())
}

/// Splits `host:port` at the LAST colon and removes the brackets from an
/// IPv6 host (`[::1]:10020` gives `::1` and `10020`). `None` if either
/// part is missing.
pub fn split_decrypt_address(address: &str) -> Option<(String, String)> {
    let (host, port) = address.trim().rsplit_once(':')?;
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if host.is_empty() || port.is_empty() {
        return None;
    }
    Some((host.to_string(), port.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_from_the_host_and_ignores_the_url_port() {
        let cases = [
            ("http://192.168.1.20:80", "192.168.1.20:10020"),
            ("http://192.168.1.20", "192.168.1.20:10020"),
            ("http://127.0.0.1", "127.0.0.1:10020"),
            ("http://localhost:30020", "localhost:10020"),
            ("https://pi.local/", "pi.local:10020"),
            ("192.168.1.20:80", "192.168.1.20:10020"),
            ("http://[::1]:80", "[::1]:10020"),
            ("http://[fe80::1]", "[fe80::1]:10020"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                derive_decrypt_address(input).as_deref(),
                Some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn invalid_addresses_derive_nothing() {
        for input in ["", "   ", "http://", "ftp://host", "not a url at all"] {
            assert_eq!(derive_decrypt_address(input), None, "{input:?}");
        }
    }

    #[test]
    fn a_typed_value_always_wins() {
        assert_eq!(
            effective_decrypt_address("127.0.0.1:10020", "http://10.0.0.5", "", true),
            "127.0.0.1:10020"
        );
        assert_eq!(
            effective_decrypt_address(" mybox:4444 ", "http://10.0.0.5", "", true),
            "mybox:4444"
        );
    }

    #[test]
    fn empty_follows_the_address_of_the_wrapper_in_use() {
        assert_eq!(
            effective_decrypt_address("", "http://10.0.0.5:80", "http://127.0.0.1:30020", true),
            "10.0.0.5:10020"
        );
        assert_eq!(
            effective_decrypt_address("", "http://127.0.0.1", "http://10.0.0.7:30020", false),
            "10.0.0.7:10020"
        );
    }

    #[test]
    fn empty_with_an_unusable_address_falls_back_to_local() {
        assert_eq!(
            effective_decrypt_address("", "http://", "", true),
            "127.0.0.1:10020"
        );
        assert_eq!(
            effective_decrypt_address("", "", "nonsense ::", false),
            "127.0.0.1:10020"
        );
    }

    #[test]
    fn splitting_strips_ipv6_brackets() {
        assert_eq!(
            split_decrypt_address("[::1]:10020"),
            Some(("::1".to_string(), "10020".to_string()))
        );
        assert_eq!(
            split_decrypt_address("192.168.1.5:10020"),
            Some(("192.168.1.5".to_string(), "10020".to_string()))
        );
        assert_eq!(
            split_decrypt_address("::1:10020"),
            Some(("::1".to_string(), "10020".to_string()))
        );
    }

    #[test]
    fn splitting_refuses_missing_parts() {
        for input in ["noport", ":10020", "host:", ""] {
            assert_eq!(split_decrypt_address(input), None, "{input:?}");
        }
    }
}
