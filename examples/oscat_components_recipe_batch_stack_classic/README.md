# Recipe Batch Stack Classic

This example uses classic `STACK_16` to store recipe IDs for a batch cell.

The classic version exposes the stack write/read pulses directly.

Compare it with `examples/oscat_components_recipe_batch_stack_components`.

## Pattern Shown

- Classic LIFO memory module.
- Recipe override stack behavior.
- Test verifies latest recipe wins.

## Validate

```bash
trust-runtime test --project examples/oscat_components_recipe_batch_stack_classic
```
