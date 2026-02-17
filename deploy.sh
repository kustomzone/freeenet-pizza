#!/usr/bin/env bash

set -euxo pipefail

nix develop -c cargo run --bin deploy-tool deploy
