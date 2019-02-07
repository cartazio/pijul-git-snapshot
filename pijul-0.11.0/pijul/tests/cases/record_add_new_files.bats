#!/usr/bin/env bats

load ../test_helper

@test "record adding new files" {
    pijul init
    touch file.txt
    touch ignored.txt
    echo ignored* > .pijul/local/ignore
    pijul record -a -n -m "adding file.txt" -A me
    echo "modified" > file.txt
    run pijul status -s
    assert_success "M file.txt"
    pijul record -a -m "modify file.txt" -A me
    rm .pijul/local/ignore
    run pijul status -s
    assert_success "\? ignored.txt"
}
