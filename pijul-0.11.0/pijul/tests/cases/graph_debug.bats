#!/usr/bin/env bats

load ../test_helper

@test "Graph debug" {
    mkdir a
    cd a
    pijul_uncovered init
    echo -e "a\nc\nd\ne" > a
    echo -e "a\nc\nd\ne" > b
    pijul_uncovered add a
    pijul_uncovered add b
    pijul_uncovered record -a -m "+ac" -A "Me"
    RUST_LOG="libpijul::backend::dump=debug" pijul_uncovered info --debug 2> log
    grep "Inode(.*)$" log | sed -e "s/.*Inode(\(.*\))$/\1/" > inodes
    RUST_BACKTRACE=1 pijul info -a --introducedby --debug --from-inode $(head -n 1 inodes)
}
