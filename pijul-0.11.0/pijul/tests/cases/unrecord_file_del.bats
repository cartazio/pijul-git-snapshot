#!/usr/bin/env bats

load ../test_helper

@test "unrecord file del" {
    mkdir toto
    cd toto
    pijul_uncovered init
    echo a > a
    pijul_uncovered add a
    pijul_uncovered record -a -m "add a" -A "me"
    pijul_uncovered remove a
    pijul_uncovered record -a -m "rm f" -A "me"
    echo yd | RUST_BACKTRACE=1 RUST_LOG="libpijul::unrecord=debug" pijul unrecord 2> /tmp/log
    RUST_LOG="libpijul::backend::dump=debug" pijul info --debug 2> /tmp/dump
    mv debug_master /tmp
    pijul_uncovered revert -a
    pijul_uncovered ls > files
    if [[ "$(cat files)" != "a" ]]; then
      echo "files = $(cat files)"
      return 1
    fi
}
