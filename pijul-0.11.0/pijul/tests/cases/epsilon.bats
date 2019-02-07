#!/usr/bin/env bats

load ../test_helper


@test "epsilon" {
    DATE="2017-12-14T14:06:12+01:00"
    mkdir a
    pijul init a
    echo d > a/file
    pijul add --repository a file
    pijul record -a --repository a -m "file" -A Alice --date "$DATE"

    pijul clone a b
    echo a > a/file
    echo d >> a/file
    pijul record -a --repository a -m "a" -A Alice --date "$DATE"

    echo b > b/file
    echo c >> b/file
    echo d >> b/file
    pijul record -a --repository b -m "b, c" -A Bob --date "$DATE"

    pijul pull -a --repository a b

    cd a
    RUST_BACKTRACE=1 pijul info --debug
    cp debug_master /tmp/debug0
    cd ..

    cp a/file /tmp/file0

    echo ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" > a/file
    echo a >> a/file
    echo "================================" >> a/file
    echo b >> a/file
    echo "<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<" >> a/file
    echo c >> a/file
    echo d >> a/file

    RUST_LOG="libpijul=debug" pijul record -a --repository a -m "resolution" -A resolver --date "$DATE" 2> /tmp/log

    cp a/file /tmp/file1
    cd a
    RUST_BACKTRACE=1 pijul info --debug
    cp debug_master /tmp
}
