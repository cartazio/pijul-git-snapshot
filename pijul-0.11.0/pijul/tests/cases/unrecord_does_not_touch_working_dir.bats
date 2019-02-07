#!/usr/bin/env bats

load ../test_helper

@test "unrecord does not touch working dir" {
    make_single_file_repo toto f
    cp -a toto titi
    cd toto
    the_patch=$(pijul log --hash-only | tail -n -1 | cut -d":" -f 1)
    pijul unrecord "$the_patch"
    cd ..
    diff -u -x .pijul titi toto
}
