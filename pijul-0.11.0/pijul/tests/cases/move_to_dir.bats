#!/usr/bin/env bats

load ../test_helper

@test "move to dir" {
    mkdir a
    cd a
    pijul init
    make_random_file file.txt
    cp file.txt backup.txt
    pijul add file.txt
    pijul record -a -m msg -A me
    mkdir subdir
    pijul mv file.txt subdir
    [[ -f subdir/file.txt ]]
    pijul record -a -m msg -A me
    sed -i '5c something' subdir/file.txt
    sed -i '5c something' backup.txt
    pijul record -a -m msg -A me
    assert_files_equal backup.txt subdir/file.txt

    cd ..
    pijul clone a b
    assert_files_equal a/subdir/file.txt b/subdir/file.txt
}
