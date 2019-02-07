#!/usr/bin/env bats

load ../test_helper

@test "delete lines with pseudo-edges" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "abc" -A "Me"

    echo -e "a\nc" > file
    pijul_uncovered record -a -m "ac" -A "Me"

    echo -e "a" > file
    yes | pijul record -m "a" -A "Me"

    pijul_uncovered info --debug
    mv debug_master /tmp

}
