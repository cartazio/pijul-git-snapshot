#!/usr/bin/env bats

load ../test_helper

@test "init forbids nesting" {
    pijul init
    mkdir subdir
    cd subdir
    run pijul init
    assert_failure ^Repository.*already\ exists
}
