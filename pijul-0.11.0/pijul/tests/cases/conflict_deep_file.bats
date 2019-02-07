#!/usr/bin/env bats

load ../test_helper

@test "conflict deep files" {
    mkdir -p a/x/y/z
    cd a
    pijul_uncovered init
    echo -e "a\nb" > x/y/z/file
    pijul_uncovered add x/y/z/file
    pijul_uncovered record -a -m msg -A me

    echo -e "b" > x/y/z/file
    HASH=$(pijul_uncovered record -a -m msg -A me | sed -e "s/Recorded patch //")

    # pijul_uncovered remove x/y/z/file
    # HASH2=$(pijul_uncovered record -a -m msg -A me | sed -e "s/Recorded patch //")

    pijul_uncovered remove x
    pijul_uncovered record -a -m msg -A me

    pijul_uncovered info --debug
    cp debug_master /tmp/debug_before

    RUST_LOG="libpijul::unrecord=debug" pijul unrecord $HASH 2> /tmp/unrec

    pijul_uncovered info --debug
    cp debug_master /tmp/debug_after

    RUST_LOG="libpijul::output=debug" pijul_uncovered revert -a 2> /tmp/log
    pijul_uncovered ls | grep file

}
