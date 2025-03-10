#!/usr/bin/env bash

set -e

# Platform detection
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS version
    SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
    
    # Build the example without using realpath
    cd "$SCRIPT_DIR"
    
    # Try to build with the example package directly
    cargo build --bin example || \
        cargo build -p example --bin example || \
        cargo build --package example
    
    # Look for the example.elf in typical locations
    for dir in target/debug target/*/debug ../../target/debug ../../target/*/debug; do
        if [ -f "$dir/example.elf" ]; then
            cp "$dir/example.elf" ./
            break
        elif [ -f "$dir/example" ]; then
            cp "$dir/example" ./example.elf
            break
        fi
    done
    
    # If we still don't have the file, try find (BSD compatible version)
    if [ ! -f "./example.elf" ]; then
        find ../../target -name "example.elf" -o -name "example" | head -n 1 | xargs -I{} cp {} ./example.elf
    fi
else
    # Linux version - use original commands
    SCRIPT_DIR=$(realpath -s "$(dirname "$0")")
    
    # Original Linux commands
    cargo build -p standalone --bin example
    find "$SCRIPT_DIR" -name "example.elf" -printf "%p" | xargs -I{} cp {} ./
fi
