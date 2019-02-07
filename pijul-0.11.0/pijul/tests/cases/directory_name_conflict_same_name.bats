#!/usr/bin/env bats

load ../test_helper


@test "directory name conflict: same name" {
    mkdir -p test/a test/c
    echo a > test/a/b
    echo a > test/c/d
    cd test
    pijul init
    pijul add a/b c/d
    pijul record -a -A "me" -m "+a +c"
    cd ..

    pijul clone test test2
    cd test
    pijul mv a x
    pijul record -a -A "me" -m "a -> x"
    cd ../test2
    pijul mv c x
    pijul record -a -A "me" -m "c -> x"

    RUST_LOG="libpijul::output=debug" pijul pull -a ../test

}
