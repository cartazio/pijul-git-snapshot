#!/usr/bin/env bats

load ../test_helper

@test "interactive record" {
    make_repo toto
    cd toto
    echo a > a
    pijul add a
    echo yy | pijul record -m "a" -A "I"
    run pijul status -s
    assert_output ""
    echo b > b
    pijul add b
    echo yn | pijul record -m "b"
    run pijul status -s
    assert_output "M b"
    pijul revert -a
    echo c > c
    pijul add c
    echo nn | pijul record -m "c"
    run pijul status -s
    assert_output "A c"
}
