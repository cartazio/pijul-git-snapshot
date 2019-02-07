#!/usr/bin/env bats

load ../test_helper

@test "Hooks" {
    mkdir a
    cd a
    pijul_uncovered init
    echo a > a
    pijul_uncovered add a
    echo -e "echo Hello > bla; return 1" > .pijul/hooks/pre-record
    chmod 755 .pijul/hooks/pre-record
    ! pijul record -a -A "Me" -m "test"
    grep Hello bla
}
