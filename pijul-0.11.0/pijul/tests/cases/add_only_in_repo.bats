#!/usr/bin/env bats

load ../test_helper

@test "add only in repo" {
    touch file.txt
    run pijul add file.txt
    assert_failure "error: Not in a repository"
}
