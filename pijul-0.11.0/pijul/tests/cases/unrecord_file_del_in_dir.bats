#!/usr/bin/env bats

load ../test_helper

@test "unrecord a file deletion in a directory" {
    mkdir -p a/dir

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc" > dir/file
    pijul_uncovered add dir/file
    pijul_uncovered record -a -m "abc" -A "Me"
    pijul_uncovered remove dir/file
    pijul_uncovered record -a -m "ac" -A "Me"

    ! pijul_uncovered ls | grep file

    echo yd | pijul unrecord
    pijul_uncovered revert -a

    pijul_uncovered ls | grep file
}
