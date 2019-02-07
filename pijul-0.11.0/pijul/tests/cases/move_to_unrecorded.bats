#!/usr/bin/env bats

load ../test_helper


@test "move to new directory" {
    pijul init
    echo test > test
    pijul add test
    pijul record -a -m "add test" -A me
    mkdir dir
    pijul add dir
    pijul mv test dir
    pijul record -a -m "mv" -A me
    pijul record -a -m "mv 2" -A me
    assert_success
}
