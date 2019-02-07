#!/usr/bin/env bats

load ../test_helper

@test "fork conflict status bats" {
    pijul init
    echo "init" > file.txt
    pijul record -A me -n -a -m "init"
    pijul fork a
    echo "a" >> file.txt
    pijul record -A me -am "a"
    pijul checkout master
    pijul fork b
    run pijul branches
    assert_output "master"
    assert_output "a"
    assert_output "b"
    echo "b" >> file.txt
    pijul record -A me -am "b"
    pijul pull . --from-branch "a" -a

    status=`pijul status -s`
    echo "$status" > status
    cp status /tmp
    diff status $BATS_TEST_DIRNAME/../expected/conflicted-short-status
    ! pijul delete-branch "b"
    pijul delete-branch "a"
    pijul branches > output
    grep master output
    grep b output
    if [[ $(wc -l output) -ne 2 ]]; then
        return 1
    fi
}
