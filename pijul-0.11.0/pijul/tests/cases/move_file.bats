#!/usr/bin/env bats

load ../test_helper

@test "move file" {
    pijul init
    make_random_file file.txt
    cp file.txt backup.txt
    pijul add file.txt
    pijul record -a -m msg -A me
    pijul mv file.txt new_file.txt
    yes | pijul record -m msg -A me
    assert_files_equal backup.txt new_file.txt
    run pijul ls
    assert_success "new_file.txt"
}
