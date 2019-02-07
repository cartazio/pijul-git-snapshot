#!/usr/bin/env bats

load ../test_helper

@test "Edit sides of a conflict (not at the end of the file)" {
    mkdir a

    cd a
    pijul_uncovered init
    echo END > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "file" -A "Me"
    cd ..

    pijul_uncovered clone a b

    cd a
    echo -e "a\nb\nc\nEND" > file
    pijul_uncovered record -a -m "abc" -A "Me"

    cd ../b
    echo -e "d\ne\nf\nEND" > file
    pijul_uncovered record -a -m "def" -A "Me"

    pijul_uncovered pull -a ../a
    echo w > file2
    cat file >> file2
    mv file2 file
    sed -i -e "s/f/f2/" file
    sed -i -e "s/c/c2/" file

    cp file /tmp/file
    RUST_LOG="libpijul::optimal_diff=debug" RUST_BACKTRACE=1 pijul record -a -m "s/e/x, s/b/y" -A "Me" 2> /tmp/log
    pijul_uncovered revert -a
    cp file /tmp/file_b
    pijul_uncovered info --debug --exclude-parents
    cp debug_master /tmp

    cd ../a
    pijul_uncovered pull -a ../b
    cp file /tmp/file_a
    (diff file $BATS_TEST_DIRNAME/../expected/edit_conflict2) || (diff file $BATS_TEST_DIRNAME/../expected/edit_conflict2_alt)
}
