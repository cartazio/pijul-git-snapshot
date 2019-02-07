#!/usr/bin/env bats

load ../test_helper

@test "add same file twice" {
    pijul init
    touch file.txt
    pijul add file.txt
    run pijul add file.txt
    assert_success "\"file.txt\" is already in the repository"
}
