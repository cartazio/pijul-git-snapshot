#!/usr/bin/env bats

load ../test_helper

@test "Challenge" {
    echo "hello, world" | pijul challenge
}
