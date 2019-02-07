#!/usr/bin/env bats

load ../test_helper

@test "add grandchild" {
    pijul_uncovered init
    mkdir subdir
    touch subdir/file.txt
    pijul_uncovered add subdir/file.txt
    run pijul record -a -m msg -A "me <me>"
    assert_success "Recorded patch"
}
