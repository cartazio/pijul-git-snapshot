#!/usr/bin/env bats

load ../test_helper

@test "zombie deep files" {
    mkdir -p a/x/y/z
    cd a
    pijul_uncovered init
    echo -e "a\nb\nc\nd\ne\nf" > x/y/z/file
    pijul_uncovered add x/y/z/file
    pijul_uncovered record -a -m msg -A me
    cd ..

    pijul_uncovered clone a b

    pijul remove --repository a x
    pijul record --repository a -a -m msg -A me

    echo "blabla" >> b/x/y/z/file
    pijul record --repository b -a -m msg -A me

    pijul pull -a --repository b a
    pijul pull -a --repository a b

    ls a/x/y/z/file

    # Solving the conflict by deleting the file
    pijul_uncovered remove --repository a x

    RUST_LOG="libpijul=debug" pijul record --repository a -a -m msg -A me 2> /tmp/rec

    cd a
    pijul info --debug
    mv debug_master /tmp/debug_a
    cd ..

    RUST_LOG="libpijul::unrecord=debug,libpijul::apply=debug,libpijul::output=debug,libpijul::record=debug" pijul pull -a --repository b a 2> /tmp/b_log
    cd b
    if [[ "$(pijul_uncovered ls | wc -l)" -ne "0" ]]; then
       return 1
    fi
    cd ..


    RUST_LOG="libpijul=debug,pijul=debug" pijul_uncovered clone a c 2> /tmp/clo
    cd c
    pijul info --debug
    mv debug_master /tmp/debug_c
    pijul ls > /tmp/lsc
    cd ..
    if [[ "$(pijul ls | wc -l)" -ne "0" ]]; then
       return 1
    fi
    if [[ "$(pijul_uncovered ls | wc -l)" -ne "0" ]]; then
       return 1
    fi
}
