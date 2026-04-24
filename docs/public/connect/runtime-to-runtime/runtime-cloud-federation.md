# Runtime Cloud Federation

Use this guide when:

- you are still deciding topology
- the system crosses one LAN/site boundary
- transport and control-plane policy need to be designed together

The quickstart gives a first runnable shape; the federation guide explains the
policy model. After reading both, you should know whether runtime-cloud belongs
in the design or whether local mesh/discovery is enough.

Success means you can draw the node roles, say which control plane each node
uses, and identify the policy that lets a remote operation proceed.

Use local discovery or mesh first when the deployment stays inside one trusted
network boundary.

Use this page when the design needs policy, identity, or cross-site dispatch in
addition to peer connectivity.

## Quickstart

--8<-- "docs/guides/RUNTIME_CLOUD_QUICKSTART.md:3"

## Federation Guide

--8<-- "docs/guides/RUNTIME_CLOUD_FEDERATION_GUIDE.md:3"

## Related

- [Operate -> Runtime Cloud](../../operate/runtime-cloud.md)
