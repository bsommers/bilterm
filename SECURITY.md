# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 3.0.x   | :white_check_mark: |
| < 3.0   | :x:                |

## Reporting a Vulnerability

Please **do not** report security vulnerabilities through public GitHub issues, discussions, or pull requests.

Instead, report vulnerabilities privately using **GitHub Private Vulnerability Reporting**:

- **Submit a Private Security Advisory:** [https://github.com/bsommers/bilterm/security/advisories/new](https://github.com/bsommers/bilterm/security/advisories/new)

### What to Include

- A description of the vulnerability and the affected component (e.g., PTY line-discipline routing, JSON-RPC / HTTP / MCP server, `secure_fs` path handling, SSH/tmux transport, or terminal escape-sequence parser).
- Steps to reproduce or a minimal proof-of-concept.
- The version, platform (`Linux`, `macOS`, or `Windows`), and configuration (`config.toml` / CLI flags) used.
