#!/usr/bin/env bats

load ../test_helper

@test "add/remove nothing to record" {
    pijul init
    touch file.txt
    pijul add file.txt
    pijul remove file.txt
    run pijul record
    assert_success "Nothing to record"
}
