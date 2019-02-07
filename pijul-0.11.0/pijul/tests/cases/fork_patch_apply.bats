#!/usr/bin/env bats

load ../test_helper

@test "fork patch apply: check symmetry" {
    pijul init
    echo "init" > file.txt
    pijul record -A me -n -a -m "init"
    pijul fork a
    echo "a" >> file.txt
    pijul record -A me -am "a"
    last_patch_a=$(pijul log --hash-only | head -n 2 | tail -n 1 | cut -d: -f 1)
    pijul checkout master
    pijul fork b
    echo "b" >> file.txt
    pijul record -A me -am "b"
    last_patch_b=$(pijul log --hash-only | head -n 2 | tail -n 1 | cut -d: -f 1)

    pijul patch --bin $last_patch_b > patch_file_b
    pijul apply $last_patch_a

    cp file.txt file_txt_b
    pijul checkout a
    pijul apply < patch_file_b
    assert_files_equal file_txt_b file.txt
}
