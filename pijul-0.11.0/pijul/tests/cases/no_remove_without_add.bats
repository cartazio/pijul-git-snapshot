#!/usr/bin/env bats

load ../test_helper

@test "no remove without add" {
    pijul init
    touch file.txt
    run pijul remove file.txt
    assert_failure "error: File \"file.txt\" not tracked"
}
