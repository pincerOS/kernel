#!/usr/bin/env bash

set -ex
cd "$(dirname "$0")/../../shell-utils"
./build.sh
cd ../init/
mv ../shell-utils/*.elf fs/
./build.sh