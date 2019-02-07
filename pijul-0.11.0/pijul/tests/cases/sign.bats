#!/usr/bin/env bats

load ../test_helper

@test "Sign" {
    pijul_uncovered key gen --signing --ssh
    mkdir a b
    cd a
    pijul_uncovered init
    echo a > a
    pijul_uncovered add a
    HASH=$(pijul_uncovered record -a -A "Me" -m "test" | sed -e 's/Recorded patch //')
    echo $HASH
    cd ../b
    pijul_uncovered init
    pijul_uncovered apply < ../a/.pijul/patches/$HASH.gz
    pijul sign < ../a/.pijul/patches/$HASH.sig
    diff a ../b/a
}
