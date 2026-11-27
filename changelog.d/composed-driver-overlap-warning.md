### Added

- The runtime warns when two composed I/O drivers fill the same bytes of the input image, such as a Modbus/TCP point
  and a raw MQTT topic, where the later driver overwrites what the earlier one read at every cycle. At every start,
  a reload's included, it logs `io_inputs_overlap` with both drivers' links and the first and last shared byte. Such
  a composition still runs.
- The analog, counter, CAN and loopback drivers declare the input bytes they fill, as the other drivers do: an analog
  input its `%IW` word, a count its `%ID` double word, a CAN receive its bytes from its `%IB` address, and the
  loopback driver the image from its start. A value located there is now marked stale while their link exchanges no
  data, and the overlap warning names them.
