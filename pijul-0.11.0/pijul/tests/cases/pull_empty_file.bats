#!/usr/bin/env bats

load ../test_helper

@test "pull empty file" {
    make_two_repos a b
    touch a/file.txt
    pijul add --repository a file.txt
    pijul record --repository a -a -m msg -A me
    pijul pull -a --repository b a
    assert_files_equal a/file.txt b/file.txt
}
