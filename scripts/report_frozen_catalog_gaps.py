#!/usr/bin/env python3
"""Report catalogue gaps from committed frozen metadata without a live scan."""

from verification.frozen_catalog_gaps import main


if __name__ == "__main__":
    raise SystemExit(main())
