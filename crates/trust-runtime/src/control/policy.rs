// The operation registry and role policy are shared runtime code (spec 34 ARCH-11); the shared
// request pipeline also checks the debug surface there.
pub(super) use trust_runtime_services::access::required_role_for_control_request;
