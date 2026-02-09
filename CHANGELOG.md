
# Changelog

All notable changes to **GAMDL GUI** are documented in this file.

This changelog is automatically generated from [conventional commits](https://www.conventionalcommits.org/).

## [Unreleased]

### ✨ Features

- Initial project scaffold with Tauri 2.0 + React + TypeScript
- Platform-adaptive CSS themes (macOS Liquid Glass, Windows Fluent, Linux Adwaita)
- Complete GAMDL CLI options model with all 11 audio codecs and 8 video resolutions
- Application settings model with fallback quality chain defaults
- Secure credential storage via OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- Cookie file validation (Netscape format parsing with expiry detection)
- IPC command framework bridging React frontend to Rust backend

### 📦 Build System

- GitHub Actions CI workflow (lint, type-check, test on macOS/Windows/Linux)
- GitHub Actions Release workflow (build .dmg/.msi/.deb/.AppImage on tag push)
- Automated CHANGELOG generation via git-cliff
- Conventional commit linting via commitlint
- Copyright year automation script

### 📚 Documentation

- Comprehensive README with feature list, architecture overview, and build instructions
- Project Plan (Project_Plan.md) with 6-phase implementation roadmap
- Project Status tracker (PROJECT_STATUS.md) with phase checkboxes
- Help documentation stubs (10 topics: getting started, downloads, settings, troubleshooting, FAQ)

---
*Generated with [git-cliff](https://git-cliff.org/)*
