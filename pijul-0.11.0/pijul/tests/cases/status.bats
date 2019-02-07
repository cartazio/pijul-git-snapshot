#!/usr/bin/env bats

load ../test_helper

@test "status" {
    make_repo toto
    cd toto
    mkdir -p dir/sub/subsub
    touch dir/sub/subsub/file
    echo a > mod
    touch del
    touch to_move
    pijul record -n -am "a" -A myself
    echo c > touch_no_add
    echo d > touch_add
    pijul add touch_add
    echo aa > mod
    rm del
    rm -rf dir
    pijul mv to_move moved

    out=`pijul status -s`
    echo "$out" > out
    diff -u out $BATS_TEST_DIRNAME/../expected/short_status

    rm out
    out=`pijul status`
    echo "$out" > out
    diff -u out $BATS_TEST_DIRNAME/../expected/long_status
}
