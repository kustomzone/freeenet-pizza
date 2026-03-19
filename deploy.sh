#!/usr/bin/env bash

set -euxo pipefail

nix develop -c deploy-tool deploy
