#!/usr/bin/env bats

load ../test_helper

@test "unrecord a simple line addition" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nc" > file
    cp file backup
    pijul_uncovered add file
    pijul_uncovered record -a -m "ac" -A "Me"

    echo -e "a\nb\nc" > file
    pijul_uncovered record -a -m "abc" -A "Me"

    echo yd | pijul unrecord
    pijul_uncovered revert -a
    assert_files_equal "file" "backup"
}
