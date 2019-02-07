#!/usr/bin/env bats

load ../test_helper

@test "Edit sides of a conflict (3, simpler case)" {
    mkdir a

    cd a
    pijul_uncovered init
    touch file
    pijul_uncovered add file
    pijul_uncovered record -a -m "file" -A "Me"
    cd ..

    pijul_uncovered clone a b

    cd a
    echo -e "a\nb\nc" > file
    pijul_uncovered record -a -m "abc" -A "Me"

    cd ../b
    echo -e "d\ne\nf" > file
    pijul_uncovered record -a -m "def" -A "Me"

    pijul_uncovered pull -a ../a
    sed -i -e "s/e/x/" file
    sed -i -e "s/a/a1/" file
    sed -i -e "s/f/f2/" file
    RUST_LOG="libpijul::optimal_diff=debug" RUST_BACKTRACE=1 pijul record -a -m "s/e/x, s/b/y" -A "Me" 2> /tmp/log

    pijul_uncovered info --debug --exclude-parents
    cp debug_master /tmp

    cd ../a
    pijul_uncovered pull -a ../b
    cp file /tmp
    (diff file $BATS_TEST_DIRNAME/../expected/edit_conflict3) || (diff file $BATS_TEST_DIRNAME/../expected/edit_conflict3_alt)
}
