#!/bin/bash
# Delegate to create_disk.sh for HDD/SSD Image generation
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/create_disk.sh" "$@"
