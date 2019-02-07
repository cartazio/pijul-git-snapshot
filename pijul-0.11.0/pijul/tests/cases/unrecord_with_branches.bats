#!/usr/bin/env bats

load ../test_helper

@test "unrecord with branches" {
    mkdir a

    cd a
    pijul_uncovered init
    echo a > file
    cp file backup
    pijul_uncovered add file
    pijul_uncovered record -a -m "Add file" -A "Me"

    pijul_uncovered fork monster

    # echo b >> file
    # pijul_uncovered record -a -m "+b" -A "Me"

    echo c >> file
    HASH=$(RUST_BACKTRACE=1 pijul_uncovered record -a -m "+c" -A "me" | sed -e 's/Recorded patch //')

    pijul unrecord "$HASH"
    pijul_uncovered revert -a
    assert_files_equal "file" "backup"


    pijul_uncovered checkout master
    ! pijul unrecord "$HASH"
}
