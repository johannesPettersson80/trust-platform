### Fixed

- The OPC UA example walks through serving a PLC's variables over OPC UA step by step, with the values of a real
  run: the server's settings, the folder `truST` that holds the exposed variables as `ns=2;s=<name>`, reads with an
  OPC UA client's command-line tools, and a write that the server refuses. It ended with "use your OPC UA client";
  it now names one and shows what it answers. Its Try-its publish less and show `validate` refusing anonymous access
  turned off without a user and a security policy without a message mode. CI reads the example's variables with an
  OPC UA client and checks the Try-its.
