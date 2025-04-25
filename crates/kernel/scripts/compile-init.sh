#!/usr/bin/env bash

set -ex
cd "$(dirname "$0")/../../shell"
./build.sh
cd ../init/
mv ../shell/bin/* fs/
./build.sh