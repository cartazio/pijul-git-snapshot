#!/usr/bin/env bats

load ../test_helper

@test "zombies from unrecord" {
    mkdir -p a/x/y/z b
    cd a
    pijul_uncovered init
    echo -e "a\nb" > x/y/z/file
    pijul_uncovered add x/y/z/file
    pijul_uncovered record -a -m msg -A me

    cd ..
    pijul_uncovered clone a b

    cd b
    echo -e "c" >> x/y/z/file
    HASH=$(pijul_uncovered record -a -m msg -A me | sed -e "s/Recorded patch //")

    cd ../a
    pijul_uncovered remove x/y
    pijul_uncovered record -a -m msg -A me

    cd ../b
    pijul_uncovered pull -a ../a
    HASH2=$(pijul_uncovered rollback $HASH  -A "me" -m "rollback" | sed -e "s/Recorded patch //")

    pijul unrecord $HASH2
    pijul_uncovered revert -a

    pijul_uncovered info --debug
    cp debug_master /tmp
    cp x/y/z/file /tmp
}
