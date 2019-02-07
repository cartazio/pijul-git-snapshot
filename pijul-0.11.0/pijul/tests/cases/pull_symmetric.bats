#!/usr/bin/env bats

load ../test_helper

@test "pull symmetric" {
    make_single_file_repo a toto
    make_single_file_repo b titi

    pijul pull -a --repository b a
    pijul pull -a --repository a b
    assert_files_equal a/toto b/toto
    assert_files_equal a/titi b/titi
}
