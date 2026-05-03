pub fn registry_api_profile() -> RegistryApiProfile {
    RegistryApiProfile {
        api_version: "v1".to_string(),
        schema_version: REGISTRY_SCHEMA_VERSION,
        endpoints: vec![
            RegistryEndpoint {
                method: "PUT".to_string(),
                path: "/v1/packages/{name}/{version}".to_string(),
                description: "Publish a package payload and metadata".to_string(),
                auth: "required for private registries".to_string(),
            },
            RegistryEndpoint {
                method: "GET".to_string(),
                path: "/v1/packages/{name}/{version}".to_string(),
                description: "Download package payload".to_string(),
                auth: "required for private registries".to_string(),
            },
            RegistryEndpoint {
                method: "GET".to_string(),
                path: "/v1/packages/{name}/{version}/verify".to_string(),
                description: "Verify package integrity against metadata digests".to_string(),
                auth: "required for private registries".to_string(),
            },
            RegistryEndpoint {
                method: "GET".to_string(),
                path: "/v1/index".to_string(),
                description: "List package summaries".to_string(),
                auth: "required for private registries".to_string(),
            },
        ],
        metadata_model: RegistryMetadataModel {
            package_fields: vec![
                "name".to_string(),
                "version".to_string(),
                "resource_name".to_string(),
                "bundle_version".to_string(),
                "published_at_unix".to_string(),
                "total_bytes".to_string(),
                "package_sha256".to_string(),
                "files".to_string(),
            ],
            file_digest_fields: vec![
                "path".to_string(),
                "bytes".to_string(),
                "sha256".to_string(),
            ],
        },
        private_registry_contract: vec![
            "visibility=private requires configured auth_token".to_string(),
            "all read/write actions require token match".to_string(),
            "token is never returned in API payloads".to_string(),
        ],
    }
}
