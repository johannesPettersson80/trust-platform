### Changed

- The MQTT driver's TLS client is the same on Windows, macOS and Linux (specification 32 §3). It
  trusts only the certificates in `tls_ca_path`, every one the file holds, and no longer the
  computer's trusted certificates, and it speaks TLS 1.2 and 1.3. A client's key may be PKCS#8,
  PKCS#1 or SEC1 PEM, and a key that does not belong to the client's certificate is refused with
  the configuration.

### Fixed

- On Windows, two truST runtimes of one user that start MQTT drivers with client certificates at
  the same time no longer share a key store of the system: one of them could fail to start its
  driver, or present the other's key.
