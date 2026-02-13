#!/usr/bin/env bash

set -euxo pipefail

if [ ! -f "webapp.parameters" ]; then
    echo "bootstrap" > /tmp/webapp-bootstrap.tar.xz
    cargo run --bin web-container-tool -- sign \
        --input /tmp/webapp-bootstrap.tar.xz \
        --output /tmp/webapp-bootstrap.metadata \
        --parameters "webapp.parameters" \
        --version 1 2>/dev/null
    # rm -f /tmp/webapp-bootstrap.tar.xz /tmp/webapp-bootstrap.metadata
fi
