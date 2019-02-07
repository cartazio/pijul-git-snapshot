#!/usr/bin/env bats

load ../test_helper

@test "Key" {
    pijul key gen --signing --ssh
    ! pijul key gen --signing
    ! pijul key gen --ssh
}
