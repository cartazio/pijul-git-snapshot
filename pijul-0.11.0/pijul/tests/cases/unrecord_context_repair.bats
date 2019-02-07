#!/usr/bin/env bats

load ../test_helper

@test "unrecord context repairs created by zombies" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nc\nd" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "ac" -A "Me"

    cd ..

    pijul_uncovered clone a b
    cd a
    echo -n "" > file
    pijul_uncovered record -a -m "empty" -A "Me"

    cd ../b

    echo -e "a\nb\nc\nd" > file
    pijul_uncovered record -a -m "empty" -A "Me"

    cd ../a
    cp file backup
    pijul_uncovered pull -a ../b

    pijul_uncovered info --debug
    mv debug_master /tmp/deb

    echo yd | RUST_LOG="libpijul::unrecord=debug" pijul unrecord 2> /tmp/log

    pijul_uncovered info --debug
    mv debug_master /tmp

    pijul_uncovered revert -a
    cat file
    assert_files_equal "file" "backup"
}
