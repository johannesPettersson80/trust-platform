# Secrets

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

## Verification

For each deployment, a second person should be able to answer where each secret
lives, who owns rotation, and which service will fail if the secret expires. If
that audit trail is missing, treat the deployment as incomplete.

## Related

- [Networking And Remote Access](../connect/networking-and-remote-access.md)
- [Runtime Cloud](runtime-cloud.md)
- [API Lifecycle And Deprecation](../reference/api-lifecycle-and-deprecation.md)
