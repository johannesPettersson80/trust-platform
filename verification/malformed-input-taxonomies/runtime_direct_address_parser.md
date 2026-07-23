# Runtime direct-address parser malformed-input taxonomy

This reviewed taxonomy covers only malformed numeric and hierarchical components at the runtime direct-address parser boundary. It is a truST runtime contract, not an IEC decision or deviation.

| Class ID | Title | Disposition | Authority |
| --- | --- | --- | --- |
| `runtime_direct_address_bit_index_out_of_range` | Direct-address bit index out of range | `required` | `SPEC_RUNTIME_SEMANTICS_001#9-2-direct-address-format` |
| `runtime_direct_address_component_overflow` | Direct-address component overflow | `required` | `SPEC_RUNTIME_SEMANTICS_001#9-2-direct-address-format` |
| `runtime_direct_address_missing_or_empty_component` | Missing or empty direct-address component | `required` | `SPEC_RUNTIME_SEMANTICS_001#9-2-direct-address-format` |
| `runtime_direct_address_non_decimal_component` | Non-decimal direct-address component | `required` | `SPEC_RUNTIME_SEMANTICS_001#9-2-direct-address-format` |
| `runtime_direct_address_unknown_area_or_size` | Unknown direct-address area or size | `required` | `SPEC_RUNTIME_SEMANTICS_001#9-2-direct-address-format` |
