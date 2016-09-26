#!/bin/bash

. mutil.sh

RUST_LOG=hl=debug,highlighter=debug cargo run -- "$@"
