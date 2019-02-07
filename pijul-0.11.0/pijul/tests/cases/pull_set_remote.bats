#!/usr/bin/env bats

load ../test_helper

@test "pull set remote" {
    make_two_repos a b
    touch a/file.txt
    pijul add --repository a file.txt
    pijul record --repository a -a -m msg -A me
    echo "modified" > a/file.txt
    pijul record --repository a -a -m msg_mod -A you
    cd b
    echo yn | pijul pull --set-remote a ../a
    echo y | pijul pull
    cd ..
    assert_files_equal a/file.txt b/file.txt

}
