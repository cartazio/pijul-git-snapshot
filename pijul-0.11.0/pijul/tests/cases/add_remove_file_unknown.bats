#!/usr/bin/env bats

load ../test_helper

@test "add/remove file unknown" {
    pijul init
    touch file.txt
    pijul add file.txt
    pijul remove file.txt

    log=`pijul status`
    echo "$log" > log
    cat log
    assert_files_equal log $BATS_TEST_DIRNAME/../expected/add_remove_file_unknown
}
