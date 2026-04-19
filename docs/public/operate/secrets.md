# Secrets

Use this page when you need to manage auth tokens, certificates, or other
deployment secrets around truST.

## Secrets You Are Likely To Handle

- control auth tokens
- remote-access tokens
- TLS certificates and keys
- runtime-cloud credentials or allowlists

## Rules

- do not hardcode production secrets in example projects
- keep local-only shortcuts local-only
- rotate tokens and certificates with an explicit procedure
- record the location and owner of each secret in the site runbook

## Related

- [Networking And Remote Access](../connect/networking-and-remote-access.md)
- [Runtime Cloud](runtime-cloud.md)
- [API Lifecycle And Deprecation](../reference/api-lifecycle-and-deprecation.md)
