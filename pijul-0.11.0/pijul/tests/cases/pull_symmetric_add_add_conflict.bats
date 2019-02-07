#!/usr/bin/env bats

load ../test_helper

@test "pull symmetric add/add conflict" {
    make_two_repos a b
    make_random_file a/toto
    pijul add --repository a toto
    pijul record --repository a -a -m msg -A me

    make_random_file b/toto
    pijul add --repository b toto
    pijul record --repository b -a -m msg -A me

    pijul pull -a --repository b a
    pijul pull -a --repository a b
    assert_dirs_equal a b
}
