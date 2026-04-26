# Recipe Batch Stack Components

This example uses `DwordStack16` to make recipe override behavior explicit.

The OOP wrapper changes pulse-style stack use into command methods. The calling
code reads as a small recipe command queue.

Compare it with `examples/oscat_components_recipe_batch_stack_classic`.

## Pattern Shown

- Command-method wrapper for LIFO memory.
- Interface type `IDwordStack`.
- Latest recipe selection with a parity-style ST test.

## Validate

```bash
trust-runtime test --project examples/oscat_components_recipe_batch_stack_components
```
