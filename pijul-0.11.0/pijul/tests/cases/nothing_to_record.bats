#!/usr/bin/env bats

load ../test_helper

@test "nothing to record" {
    pijul init
    run pijul record
    assert_success "Nothing to record"
}
