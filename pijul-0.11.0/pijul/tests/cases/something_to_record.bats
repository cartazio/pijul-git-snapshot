#!/usr/bin/env bats

load ../test_helper

@test "something to record" {
    pijul init
    touch file.txt
    pijul add file.txt
    pijul remove file.txt

    out=`pijul status`
    echo "$out" > out
    assert_files_equal out $BATS_TEST_DIRNAME/../expected/add_remove_file_unknown
}
