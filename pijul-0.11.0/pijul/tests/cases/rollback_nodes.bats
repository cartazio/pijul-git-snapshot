#!/usr/bin/env bats

load ../test_helper

@test "Rollback new nodes" {
    pijul init
    echo -e "a\nb\nc\nd" > a
    pijul add a
    pijul record -a -m "+a" -A "Me"
    echo yd | pijul rollback -A "Me" -m "rollback"
}
