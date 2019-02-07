#!/usr/bin/env bats

load ../test_helper

@test "zombie files" {
    mkdir a
    cd a
    pijul init
    echo -e "a\nb\nc\nd\ne" > file
    pijul add file
    pijul record -a -m "add a" -A me
    cd ..
    pijul clone a b


    echo -e "a\ne" > a/file
    pijul record --repository a -a -m remove -A me

    echo -e "a\nb\nx\nc\nd\ne" > b/file
    pijul record --repository b -a -m blabla -A me

    RUST_LOG="libpijul::apply=debug" pijul pull -a --repository b a 2> /tmp/log
    cd b
    pijul info --debug
    cp debug_master /tmp
    cd ..

    pijul pull -a --repository a b

    find b >> /tmp/log
    cp b/file /tmp/b_file
    if [[ "$(cat a/file | wc -l)" -ne "9" ]]; then
       return 1
    fi

    pijul remove --repository a file
    pijul record --repository a -a -m solve -A me
    pijul clone a c
    if [[ "$(ls -1 c | wc -l)" -ne "0" ]]; then
       return 1
    fi
}
