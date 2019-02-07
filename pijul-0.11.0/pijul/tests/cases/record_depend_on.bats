#!/usr/bin/env bats

load ../test_helper

@test "record depend on" {
    pijul init a
    cd a
    touch file.txt
    pijul record -a -n -m "adding file.txt" -A me
    echo "another file" > file2.txt
    last_patch=$(pijul log --hash-only | tail -1 | cut -d: -f 1)
    pijul record -a -n -m "add file2.txt" -A me --depends-on $last_patch
    cd ..
    pijul init b
    cd b
    echo n | pijul pull ../a
    run ls
    assert_output "^$"
}
