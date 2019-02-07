#!/usr/bin/env bats

load ../test_helper


@test "directory name conflict: two names" {
    mkdir -p test/a
    echo a > test/a/b
    cd test
    pijul init
    pijul add a/b
    pijul record -a -A "me" -m "+a"
    cd ..

    pijul clone test test2
    cd test
    pijul mv a x
    pijul record -a -A "me" -m "a -> x"
    cd ../test2
    pijul mv a y
    pijul record -a -A "me" -m "a -> y"

    RUST_LOG="libpijul::output=debug" pijul pull -a ../test
}
