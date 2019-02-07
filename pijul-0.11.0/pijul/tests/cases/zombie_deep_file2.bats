#!/usr/bin/env bats

load ../test_helper

@test "zombie deep files 2" {
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
    grep -v ">>>>" a/x/y/z/file | grep -v "<<<<" > file
    cp file /tmp/file_solved
    mv file a/x/y/z


    cd a
    pijul_uncovered ls | grep file

    # Confirming the presence of the conflicting file
    pijul add x/y/z/file
    pijul record -a -m msg -A me
    pijul_uncovered info --debug
    cp debug_master /tmp

    cd ..

    pijul_uncovered clone a c
    cd c
    cp x/y/z/file /tmp
    ! grep ">>>>" x/y/z/file
    pijul_uncovered ls | grep file
    pijul_uncovered revert -a
    pijul_uncovered ls | grep file


    # Unrecord, the conflict should reappear
    cd ../a
    echo yd | pijul unrecord

    pijul_uncovered revert -a
    pijul ls > /tmp/ls
    grep ">>>>" x/y/z/file
}
