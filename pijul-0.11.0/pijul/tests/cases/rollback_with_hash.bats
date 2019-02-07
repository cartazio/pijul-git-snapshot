#!/usr/bin/env bats

load ../test_helper

@test "Rollback new nodes" {
    pijul_uncovered init
    echo "a" > a
    pijul_uncovered add a
    RUST_LOG="debug" pijul_uncovered record -a -m "+a" -A "Me" 2> /tmp/log
    echo -e "a\nb" > a
    HASH=$(pijul_uncovered record -a -m "+b" -A "Me" | sed -e 's/Recorded patch //')

    ! pijul rollback --branch blabla  -A "me" -m "rollback"
    ! pijul rollback $(echo $HASH | sed -e "s/A/b/")  -A "me" -m "rollback"
    pijul rollback $HASH  -A "me" -m "rollback"
}
