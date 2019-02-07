#!/usr/bin/env bats

load ../test_helper

@test "pull and push are symmetric" {
    make_single_file_repo a toto
    make_single_file_repo b titi

    cd a
    pijul pull -a ../b --set-default
    pijul push -a ../b --set-default
    cd ..

    assert_files_equal a/toto b/toto
    assert_files_equal a/titi b/titi
}
