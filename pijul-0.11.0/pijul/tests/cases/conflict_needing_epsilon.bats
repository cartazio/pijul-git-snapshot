#!/usr/bin/env bats

load ../test_helper

@test "conflict needing epsilon" {
    mkdir a

    cd a
    pijul init
    echo a > file
    pijul add file
    pijul record -a -m "+file" -A "me"
    cd ..

    pijul clone a b
    cd b
    echo b >> file
    pijul record -a -m "b" -A "me"

    cd ../a
    echo c >> file
    pijul record -a -m "a" -A "me"
    pijul pull ../b -a

    grep -v ">>>" file | grep -v "<<<" | grep -v "===" > file2
    mv file2 file
    pijul record -a -m "conflict resolution" -A "me"
    pijul info --debug

    cp debug_master /tmp
    grep orange debug_master
    if [[ $? -ne 0 ]]; then
        echo "no epsilon lines"
        return 1
    fi
    wc -l debug_master
    if [[ "$(cat debug_master | wc -l)" != "22" ]]; then
        echo "wrong line count"
        return 1
    fi

    cd ..
    pijul clone a c
    cd c
    if [[ "$(pijul_uncovered diff)" != "" ]]; then
       return 1
    fi
}
