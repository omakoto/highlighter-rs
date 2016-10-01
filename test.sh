#!/bin/bash

. mutil.sh

RUST_BACKTRACE=1 RUST_LOG='hl=debug,highlighter=debug' cargo test -- --nocapture "$@"
