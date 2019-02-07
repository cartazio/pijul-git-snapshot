#!/usr/bin/env bats

load ../test_helper

@test "unrecord with patches used in another branch" {
    mkdir a

    cd a
    pijul_uncovered init
    echo a > file
    cp file backup
    pijul_uncovered add file
    pijul_uncovered record -a -m "Add file" -A "Me"

    echo b >> file
    cp file backup2
    HASH=$(pijul_uncovered record -a -m "+c" -A "me" | sed -e 's/Recorded patch //')

    pijul_uncovered fork monster
    pijul unrecord "$HASH"
    pijul_uncovered revert -a
    assert_files_equal "file" "backup"

    pijul checkout master
    assert_files_equal "file" "backup2"
}
