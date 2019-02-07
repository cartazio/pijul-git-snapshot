#!/usr/bin/env bats

load ../test_helper

@test "init another directory" {
    mkdir subdir
    pijul init subdir
    [[ -d subdir/.pijul ]]
}
