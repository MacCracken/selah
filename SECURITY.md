# Security Policy

## Reporting a Vulnerability

Please report security vulnerabilities by emailing security@agnos.dev.

Do not open a public GitHub issue for security vulnerabilities.

## Scope

Selah processes screenshots which may contain sensitive information. Security-relevant areas:

- **PII detection and redaction** (ai.rs) — false negatives could leak sensitive data
- **MCP server** (mcp.rs) — path traversal, input validation
- **SVG output** (annotate.rs) — XSS prevention via xml_escape
- **File I/O** (history.rs) — concurrent access, path handling
