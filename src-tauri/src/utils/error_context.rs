// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// `context_err!` macro for command-handler error wrapping.
// =========================================================
//
// Audit v2 finding #7: ~30-40 sites across `commands/` repeat the
// same error-wrapping incantation:
//
//     entry.set_password(&value)
//         .map_err(|e| format!("Failed to store credential '{key}': {e}"))?;
//
// The `.map_err(|e| format!("...: {e}"))` is pure plumbing — the
// caller wants to add a context prefix and propagate. This macro
// collapses it to:
//
//     context_err!(
//         entry.set_password(&value),
//         "Failed to store credential '{key}'"
//     )?;
//
// **Real value isn't LOC saved** (~30 chars per site, modest). It's
// having one place to evolve error formatting — e.g. if we ever
// decide to swap the `:` separator for `—`, hook all errors into
// `tracing` events, or migrate to a `CommandError` enum with
// structured logging, the change touches one macro, not 40 sites.
//
// **Naming:** `context_err!` (not `try_with_context!` or similar)
// to match Rust's `anyhow::Context` ecosystem vocabulary — the
// concept ("attach a context message to an error before propagating")
// is the same, just without the `anyhow` dep weight for now.
//
// **Migration is opt-in.** New commands should use the macro from
// the start; existing sites migrate opportunistically. The macro
// produces byte-for-byte identical output to the inline pattern
// (same `String` shape: `"<message>: <error display>"`), so a partial
// migration is observably equivalent to a full one — error messages
// reaching the frontend stay stable.
//
// @see audit doc finding #7 in
//      [.github/audits/codebase-unification-audit-v2.md]

/// Wrap a `Result<T, E>` with a context-bearing message.
///
/// Expands to `result.map_err(|e| format!("{message}: {e}"))`.
/// The `?` operator stays at the call site so the caller decides
/// the return type (no implicit propagation).
///
/// Format-string args (`{key}`, `{count}`, etc.) are resolved
/// against the surrounding scope — same semantics as `format!()`.
///
/// # Examples
///
/// ```ignore
/// // Simple static message
/// let entry = context_err!(
///     keyring::Entry::new(SERVICE_NAME, &key),
///     "Failed to create keyring entry"
/// )?;
///
/// // With format args from the surrounding scope
/// entry.set_password(&value)
///     .map_err_context("Failed to store credential")?;
/// // ...is equivalent to:
/// context_err!(
///     entry.set_password(&value),
///     "Failed to store credential '{key}'"
/// )?;
/// ```
#[macro_export]
macro_rules! context_err {
    ($result:expr, $($arg:tt)+) => {
        $result.map_err(|e| format!("{}: {}", format_args!($($arg)+), e))
    };
}

#[cfg(test)]
mod tests {
    // Note: the macro is defined with #[macro_export], so it's
    // available as `crate::context_err!` everywhere. The tests just
    // exercise it directly.

    #[test]
    fn passes_ok_through_unchanged() {
        let result: Result<i32, String> =
            context_err!(Ok::<i32, String>(42), "should never appear");
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn prefixes_static_error_message() {
        let result: Result<(), String> = context_err!(
            Err::<(), &str>("disk full"),
            "Failed to write file"
        );
        assert_eq!(result, Err("Failed to write file: disk full".to_string()));
    }

    #[test]
    fn supports_format_args_from_surrounding_scope() {
        let key = "musickit_jwt";
        let result: Result<(), String> = context_err!(
            Err::<(), &str>("locked"),
            "Failed to fetch credential '{key}'"
        );
        assert_eq!(
            result,
            Err("Failed to fetch credential 'musickit_jwt': locked".to_string())
        );
    }

    #[test]
    fn works_with_question_mark_in_a_function() {
        // Mirrors the real call-site shape — the macro returns a
        // Result that the caller propagates with `?`.
        fn doit() -> Result<i32, String> {
            let value = context_err!(
                "abc".parse::<i32>(),
                "Failed to parse number"
            )?;
            Ok(value * 2)
        }
        let err = doit().unwrap_err();
        assert!(err.starts_with("Failed to parse number: "));
    }

    #[test]
    fn preserves_underlying_error_display() {
        // Errors that implement Display via #[derive(Debug)] only
        // would render via `{:?}` — the macro uses `{}` (Display),
        // matching the existing inline pattern's behaviour.
        #[derive(Debug)]
        struct DisplayErr;
        impl std::fmt::Display for DisplayErr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "custom display")
            }
        }
        let result: Result<(), String> = context_err!(
            Err::<(), DisplayErr>(DisplayErr),
            "Operation failed"
        );
        assert_eq!(result, Err("Operation failed: custom display".to_string()));
    }
}
