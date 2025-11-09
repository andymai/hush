#!/bin/bash
# Simple runner for Hush with correct PKG_CONFIG_PATH
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig cargo run -- "$@"
