#!/usr/bin/env bats

load ../test_helper


@test "Unable to switch branch" {
    pijul init hello
    cd hello
    echo "Hello, World!" > hello.txt
    pijul add hello.txt
    pijul record -a -m "Initial state" -A "me"

    pijul fork other
    pijul checkout other
    pijul mv hello.txt hello_world.txt
    echo "Hello!" > hello.txt
    pijul add hello.txt
    pijul record -am 'Moved "Hello, World!", created new hello.txt'

    pijul checkout master
    pijul branches > branches
    grep "^\* master" branches
    grep "^  other" branches

    pijul checkout other
    pijul branches > branches
    grep "^  master" branches
    grep "^\* other" branches
    touch foo
}
