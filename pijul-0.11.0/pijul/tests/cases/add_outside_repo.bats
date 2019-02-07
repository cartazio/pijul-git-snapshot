#!/usr/bin/env bats

load ../test_helper

@test "add outside repo" {
    mkdir subdir
    cd subdir
    pijul init
    touch ../file.txt
    run pijul add ../file.txt
    assert_failure "error: Invalid path"
}
