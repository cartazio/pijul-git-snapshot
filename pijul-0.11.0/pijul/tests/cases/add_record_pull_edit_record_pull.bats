#!/usr/bin/env bats

load ../test_helper

@test "add/record/pull/edit/record/pull" {
    make_single_file_repo a file.txt
    pijul clone a b
    assert_files_equal a/file.txt b/file.txt

    # Pull back the other way, without making any changes
    pijul pull -a --repository a b
    assert_files_equal a/file.txt b/file.txt

    # Now make a change, and pull back
    sed -i '4i add a line' b/file.txt
    sed -i '2s/.*/blabla/' b/file.txt
    sed -i '7D' b/file.txt
    cp a/file.txt /tmp/a
    cp b/file.txt /tmp/b
    yes | pijul record --repository b -m msg -A me > /tmp/rec
    pijul pull -a --repository a b
    assert_files_equal a/file.txt b/file.txt
}
