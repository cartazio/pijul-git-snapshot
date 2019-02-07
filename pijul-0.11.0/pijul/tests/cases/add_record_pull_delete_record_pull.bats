#!/usr/bin/env bats

load ../test_helper

@test "add/record/pull/delete/record/pull" {
    make_single_file_repo a toto
    pijul clone a b
    pijul remove --repository b toto
    yes | pijul record --repository b -m msg -A me
    yes | RUST_LOG="libpijul::output=debug,libpijul::apply=debug" pijul pull --repository a b 2> /tmp/out
    cd a
    RUST_LOG="libpijul::backend::dump=debug" pijul info --debug 2> /tmp/log
    mv debug_master /tmp
    cd ..
    [[ ! -f a/toto ]]
}
