# Security Policy

## Supported Versions

Security updates are applied to the default branch and the latest released contract version.

| Version | Supported |
| --- | --- |
| `main` | Yes |
| Latest release | Yes |
| Older releases | No |

## Reporting a Vulnerability

Please report suspected vulnerabilities privately through GitHub Security Advisories for this repository. Do not open a public issue or pull request that includes exploit details, private keys, account data, or reproduction steps that could put users at risk.

When possible, include:

- A clear description of the vulnerability and affected code path.
- Minimal reproduction steps or a proof of concept.
- Potential impact, including affected assets or permissions.
- Suggested remediation, if you already have one.

If GitHub Security Advisories are unavailable, open a GitHub Discussion or issue that asks for a private security contact without including sensitive details.

## Disclosure Timeline

The maintainers aim to:

1. Acknowledge valid reports within 3 business days.
2. Triage severity and affected versions within 7 business days.
3. Prepare and test a fix before public disclosure.
4. Credit reporters in release notes when requested and appropriate.

Public disclosure should wait until a fix is available or the maintainers agree on a coordinated disclosure date.

## Scope

Reports are in scope when they affect the StellarTip contract, deployment scripts, CI, or repository configuration in a way that could compromise contract funds, admin controls, user balances, privacy, or release integrity.

Out-of-scope reports include spam, social engineering, denial-of-service against third-party services, and vulnerabilities that require already-compromised maintainer credentials.
