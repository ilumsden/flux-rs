#!/usr/bin/env bash

if cargo deny --all-features check; then
    exit 0
fi

if [[ -n "$ALLOW_CARGO_DENY_FAILURE" ]]; then
    echo "cargo deny failed, but ALLOW_CARGO_DENY_FAILURE is set; allowing push."
    exit 0
fi

exit 1
