#!/usr/bin/env bats

load ../test_helper


@test "add from outside repo" {
    mkdir subdir
    touch subdir/file.txt
    pijul init subdir
    pijul add --repository subdir file.txt
    assert_success
}
