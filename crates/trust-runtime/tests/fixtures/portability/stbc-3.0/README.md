# Current temporal ABI board probe

`gpio-temporal.st` preserves the existing B1 GPIO and depth-result contract.
Its LED and every depth result additionally depend on an admitted relative
`TIME`/`LINT` overlay round-tripping positive and negative five-second values,
and on the four short temporal types occupying eight bytes. A false check forces
the output low and the depth result to -1, which the unchanged B1 verifier rejects.

`expected-gpio-temporal.csv` is an authored native logical oracle. It does not
record an executed test or a hardware observation. The native
`portability_f401_gpio` test uses the firmware's admission limits, checks the
internal alias values, and preserves the old input/output/depth expectations.

In the final validation batch, generate the new artifact into that batch's
external evidence directory:

```sh
cargo run --locked -p trust-runtime --example portability_temporal_fixture -- <artifact-directory>
cargo xtask portability pack <artifact-directory>/gpio-temporal-v3.stbc <artifact-directory>/application.bin
```

The existing pack command keeps the saved 2.0 main and numeric artifacts and
inserts this separately named 3.0 GPIO probe in the third slot. Firmware still
performs full device-side admission. Its B1 schema, firmware runner, verifier and
saved A4 oracle are unchanged. Record the generated artifact and bundle hashes,
and use that exact bundle in the conditional hardware phase. Old 2.0 GPIO and
other saved artifact bytes remain historical compatibility evidence.
