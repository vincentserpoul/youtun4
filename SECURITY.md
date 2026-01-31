# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Youtun4, please report it responsibly:

1. **Do NOT** open a public GitHub issue for security vulnerabilities
2. Email the maintainers directly (add your security contact email here)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

We will acknowledge receipt within 48 hours and provide a detailed response within 7 days.

## Security Practices

This project follows these security practices:

### Dependency Management

- **cargo-audit**: Regular checks against RustSec advisory database
- **cargo-deny**: License compliance and banned crate checks
- **Dependabot/Renovate**: Automated dependency updates

### Code Quality

- **Clippy**: Strict linting with security-focused rules
- **cargo-geiger**: Unsafe code detection
- **prek**: Fast Rust-based pre-commit hooks (compatible with pre-commit configs)

### Secret Management

- **Gitleaks**: Automated secret detection in commits
- No secrets stored in code repository
- Environment variables for sensitive configuration

### CI/CD

- Security scanning on every PR
- Weekly scheduled security audits
- Automated vulnerability alerts

## Security Checklist for Contributors

- [ ] No hardcoded secrets, API keys, or credentials
- [ ] No `unsafe` blocks without thorough documentation and review
- [ ] Dependencies are from trusted sources (crates.io)
- [ ] New dependencies have acceptable licenses (MIT, Apache-2.0, BSD)
- [ ] Input validation for all user-provided data
- [ ] Proper error handling (no `unwrap()` on untrusted input)
