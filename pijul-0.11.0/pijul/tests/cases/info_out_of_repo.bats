#!/usr/bin/env bats

load ../test_helper

@test "info out of repo" {
    run pijul info
    assert_failure "error: Not in a repository"
}
