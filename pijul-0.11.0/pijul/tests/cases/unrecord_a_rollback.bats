#!/usr/bin/env bats

load ../test_helper

@test "unrecord a rollback" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "abc" -A "Me"
    cp file /tmp/file0
    echo -e "a\nc" > file
    cp file backup
    pijul_uncovered record -a -m "ac" -A "Me"

    echo yd | pijul rollback -A "Me" -m "ac"
    echo yd | RUST_LOG="libpijul::unrecord=debug" pijul unrecord 2> /tmp/log
    pijul_uncovered revert -a
    pijul_uncovered info --debug
    cp debug_master /tmp
    cp file backup /tmp
    assert_files_equal "file" "backup"
}
