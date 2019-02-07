#!/usr/bin/env bats

load ../test_helper

@test "unrecord the rollback of a file addition in a directory" {
    mkdir -p a/b

    cd a
    pijul_uncovered init
    pijul_uncovered add b
    pijul_uncovered record -a -m "+b" -A "Me"
    echo -e "a\nb\nc" > b/file
    pijul_uncovered add b/file
    pijul_uncovered record -a -m "+file" -A "Me"

    pijul_uncovered remove b/file
    pijul_uncovered record -a -m " -file" -A "Me"

    echo yd | RUST_BACKTRACE=1 RUST_LOG="pijul=debug,libpijul::patch=debug" pijul rollback -A "me" -m "rollback" 2> /tmp/rblog
    echo yd | RUST_BACKTRACE=1 RUST_LOG="libpijul::unrecord=debug,libpijul::apply::find_alive" pijul unrecord 2> /tmp/log

    RUST_BACKTRACE=1 RUST_LOG="libpijul::output=debug" pijul_uncovered revert -a 2> /tmp/revert
}
