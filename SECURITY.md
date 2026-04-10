# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.29.x  | :white_check_mark: |
| < 0.29  | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in MeedyaDL, please report it responsibly:

1. **Do NOT open a public GitHub issue** for security vulnerabilities.
2. **Email**: Send details to the repository maintainers via the email listed in the GitHub organisation profile.
3. **Include**: A description of the vulnerability, steps to reproduce, and any potential impact.
4. **Response time**: We aim to acknowledge reports within 48 hours and provide a fix within 7 days for critical issues.

## Security Measures

MeedyaDL implements the following security measures:

- **Path traversal protection** in ZIP and TAR archive extraction
- **URL validation** prevents CLI flag injection in subprocess calls
- **INI injection prevention** via newline sanitization in config values
- **HTML sanitization** in markdown rendering (rehype-sanitize)
- **Credential redaction** in all logging, exports, and crash reports
- **Atomic file writes** for settings and queue persistence
- **Field-level validation** on imported settings/queue files
- **No shell interpolation** — all subprocess calls use parameterised arguments
- **Content Security Policy** configured in Tauri for the webview
- **Secrets stored in OS keychain** (not on disk)
- **SHA-256 checksum verification** for downloaded dependencies
- **GitHub Actions hardening**: all actions pinned to immutable commit SHAs
- **cargo-deny** licence scanning and source allowlisting in CI (org-level `[sources.allow-org]`)
- **CodeQL** static analysis for JavaScript/TypeScript and GitHub Actions
- **Activity log memory bounds** — capped at 10,000 entries to prevent unbounded WebView memory growth
- **Updater artifact signing** — `.app.tar.gz.sig` signature files verified by Tauri updater before installation

## Scope

This security policy covers the MeedyaDL desktop application and its build/release infrastructure. It does not cover third-party dependencies (GAMDL, FFmpeg, etc.) — please report issues with those to their respective projects.
