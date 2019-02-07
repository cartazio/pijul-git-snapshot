#!/usr/bin/env bats

load ../test_helper

@test "rollback a simple patch" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc" > file
    cp file backup
    pijul_uncovered add file
    pijul_uncovered record -a -m "abc" -A "Me"

    echo -e "a\nc" > file
    pijul_uncovered record -a -m "ac" -A "Me"

    echo yd | RUST_LOG="libpijul::apply=debug,libpijul::patch=debug,pijul::commands::rollback=debug" pijul rollback -A "me" -m "rollback" 2> /tmp/log
    pijul_uncovered revert -a
    pijul info --debug
    mv debug_master /tmp
    cp file backup /tmp
    assert_files_equal "file" "backup"
}
