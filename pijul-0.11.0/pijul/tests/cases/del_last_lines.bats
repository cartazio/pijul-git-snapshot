#!/usr/bin/env bats

load ../test_helper

@test "Delete the last lines of a file" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc\nd" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "file" -A "Me"
    echo -e "e\nd" > file
    pijul record -a -m "delete" -A "Me"
}
