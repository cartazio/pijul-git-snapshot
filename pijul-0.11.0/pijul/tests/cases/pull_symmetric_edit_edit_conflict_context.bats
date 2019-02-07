#!/usr/bin/env bats

load ../test_helper

@test "pull symmetric edit/edit conflict with context" {
    make_single_file_repo a toto
    pijul clone a b

    append_random a/toto
    append_random b/toto
    pijul record --repository a -a -m msg -A me
    pijul record --repository b -a -m msg -A me
    pijul pull -a --repository b a
    pijul pull -a --repository a b
    assert_dirs_equal a b
    assert_file_contains a/toto '>>>>>'
}
