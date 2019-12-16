#!/usr/bin/env bash

set -euxo pipefail

./pants fmt2 ::

pex mypy --entry-point=mypy -- **/*.py

pushd terminal
cargo clippy
popd
