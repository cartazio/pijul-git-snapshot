#!/usr/bin/env bats

load ../test_helper

@test "pull symmetric edit/edit conflict" {
    make_two_repos a b
    touch a/toto
    pijul add --repository a toto
    pijul record --repository a -a -m msg -A me
    pijul pull -a --repository b a

    make_random_file a/toto
    make_random_file b/toto
    pijul record --repository a -a -m msg -A me
    pijul record --repository b -a -m msg -A me
    pijul pull -a --repository b a
    pijul pull -a --repository a b
    cp a/toto /tmp
    assert_dirs_equal a b
    assert_file_contains a/toto '>>>>>'
}
