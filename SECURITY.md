# Security Policy

## Supported versions

Security fixes are prioritized for the latest WebCodex release.

| Version | Support |
|---|---|
| 1.2.x | Current development line |
| Older versions | Best effort |

## Reporting vulnerabilities

Please do not disclose exploitable security issues in public GitHub issues.

Use GitHub's private vulnerability reporting feature if it is enabled for the repository. Otherwise, contact the repository maintainers privately through a published project contact channel.

Include reproduction steps, affected versions, and the potential impact. Do not include real credentials, session cookies, or private user data.

## Security model

WebCodex automates a signed-in browser session and can apply file edits and run commands locally.

The command:

`webcodex resume -s danger-full-access -a never`

grants broad local execution privileges and disables WebCodex's approval prompts. It is not an operating-system sandbox.

Use unrestricted execution only with projects and instructions you trust.

WebCodex does not provide unlimited ChatGPT access and does not bypass ChatGPT account restrictions.

## Sensitive data

Never publish the persistent browser profile, session cookies, local conversation database, or files copied from private projects.

Authentication should be performed through the browser's normal login flow. Do not paste passwords, session tokens, or authentication cookies into bug reports.
