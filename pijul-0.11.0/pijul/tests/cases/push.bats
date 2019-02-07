#!/usr/bin/env bats

load ../test_helper

@test "Push" {
    mkdir a b
    cd b
    pijul_uncovered init
    cd ../a
    pijul_uncovered init
    echo a > a
    pijul_uncovered add a
    pijul_uncovered record -a -m "+b" -A "Me"
    echo yd | pijul push ../b
}
