#!/usr/bin/env bats

load ../test_helper

# Try to revert a file scheduled to be moved
@test "revert move" {
    make_repo toto
    cd toto
    echo 'fn main() { println!("Hello"); }' > foo.rs
    pijul add foo.rs

    status=`pijul status -s`

    pijul record -am "a" -A myself
    rm -Rf .pijulconfig
    pijul mv foo.rs bar.rs

    pijul revert -a
    status2=`pijul status`
    echo "$status" > status
    echo "$status2" >> status
    diff -u status $BATS_TEST_DIRNAME/../expected/revert_move
}
