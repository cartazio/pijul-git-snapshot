#!/usr/bin/env bats

load ../test_helper

@test "pull to another branch" {
    mkdir test
    cd test
    pijul init
    echo a > a
    pijul record -A "me" -m "hi" -n -a
    pijul fork test1
    echo b > b
    pijul record -A "me" -m "hi" -n -a
    pijul fork test2
    pijul pull . --from-branch test2 --to-branch master -a
}

@test "push to another branch" {
    mkdir test
    cd test
    pijul init
    echo a > a
    pijul record -A "me" -m "hi" -n -a
    pijul fork test1
    echo b > b
    pijul record -A "me" -m "hi" -n -a
    pijul fork test2
    pijul push . --from-branch test2 --to-branch master -a
}