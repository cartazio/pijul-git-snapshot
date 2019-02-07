#!/usr/bin/env bats

load ../test_helper

@test "pull 30 patches" {
    make_single_file_repo a toto
    pijul clone a b

    for i in {1..30}; do
        make_random_file a/toto
        pijul record --repository a -a -m $i -A me
    done
    pijul pull -a --repository b a
    assert_files_equal a/toto b/toto
}
