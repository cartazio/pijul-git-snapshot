#!/usr/bin/env bats

load ../test_helper

@test "zombie files" {
    mkdir a
    cd a
    pijul init
    echo -e "a\nb" > file
    pijul add file
    pijul record -a -m "add a" -A me
    cd ..
    pijul clone a b

    pijul remove --repository a file
    pijul record --repository a -a -m remove -A me

    echo -e "a\nx\ny\nb" > b/file
    pijul record --repository b -a -m blabla -A me

    pijul pull -a --repository b a

    pijul revert --repository b -a

    pijul pull -a --repository a b

    if [[ "$(cat a/file | wc -l)" -ne "8" ]]; then
       return 1
    fi

    pijul remove --repository a file
    cd a
    pijul info --debug
    mv debug_master /tmp/before_a
    cd ..
    pijul record --repository a -a -m solve -A me
    cd b
    pijul info --debug
    mv debug_master /tmp/before_b
    cd ..
    RUST_LOG="libpijul::output=debug,libpijul::unrecord=debug,libpijul::apply=debug" pijul pull -a --repository b a 2> /tmp/log
    pijul clone a c
    cd b
    pijul info --debug
    mv debug_master dump_contents /tmp
    cd ..

    if [[ "$(ls -1 c | wc -l)" -ne "0" ]]; then
       return 1
    fi
}
