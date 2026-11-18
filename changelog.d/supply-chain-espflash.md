### Security

- The ruzstd advisory (RUSTSEC-2026-0344, a decompression bomb in zstd-compressed input) no longer
  concerns `trust-runtime firmware install`: it reads the ESP32 firmware it flashes with espflash
  4.7.0, whose release of `object` does not bring ruzstd at all.
