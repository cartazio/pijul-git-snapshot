#!/usr/bin/env bats

load ../test_helper

@test "move unadded" {
    pijul init
    touch file.txt
    run pijul mv file.txt other.txt
    assert_failure "error: File \"file.txt\" not tracked"
}
