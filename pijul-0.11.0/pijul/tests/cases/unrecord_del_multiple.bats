#!/usr/bin/env bats

load ../test_helper

@test "unrecord multiple line deletions" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc\ne\nf" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "abcdef" -A "Me"

    echo -e "a\nc\nf" > file
    pijul_uncovered record -a -m "aef" -A "Me"
    cp file backup

    echo -e "a\nf" > file
    pijul_uncovered record -a -m "af" -A "Me"

    pijul_uncovered info --debug
    mv debug_master /tmp/before

    echo yd | RUST_LOG="libpijul::unrecord=debug" pijul unrecord 2> /tmp/log

    pijul_uncovered info --debug
    mv debug_master /tmp

    pijul_uncovered revert -a
    cat file
    cp file backup /tmp
    assert_files_equal "file" "backup"
}
