#!/usr/bin/env bats

load ../test_helper

@test "unrecord file add" {
    mkdir toto
    cd toto
    pijul_uncovered init
    echo a > a
    pijul_uncovered add a
    pijul_uncovered record -a -m "add a" -A "me"
    echo yd | RUST_LOG="libpijul::unrecord=debug" pijul unrecord 2> /tmp/log

    pijul_uncovered ls | grep a
    # a must still be here, ready to be recorded again
    if [[ $? -ne 0 ]]; then
        echo "a is not here"
        return 1
    fi

    pijul_uncovered revert -a
    # a must not be here anymore after revert
    if [[ $(pijul_uncovered ls | wc -l) -ne 0 ]]; then
        echo "a is still here"
        return 1
    fi
}
