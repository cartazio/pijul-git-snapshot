#!/usr/bin/env bats

load ../test_helper

@test "init allows nesting" {
    # The --allow-nested option is not yet implemented
    skip
    pijul init
    mkdir subdir
    cd subdir
    pijul --allow-nested init
}
