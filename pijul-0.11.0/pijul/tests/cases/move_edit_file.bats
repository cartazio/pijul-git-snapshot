#!/usr/bin/env bats

load ../test_helper

@test "move and edit file" {
    mkdir a
    cd a
    pijul init
    make_random_file file.txt
    cp file.txt backup.txt
    pijul add file.txt
    pijul record -a -m msg -A me
    pijul mv file.txt new_file.txt
    sed -i '4c new line' new_file.txt
    sed -i '4c new line' backup.txt
    pijul record -a -m msg -A me
    assert_files_equal backup.txt new_file.txt

    cd ..
    pijul clone a b
    assert_files_equal a/new_file.txt b/new_file.txt
}
