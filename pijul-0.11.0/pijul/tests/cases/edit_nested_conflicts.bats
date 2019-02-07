#!/usr/bin/env bats

load ../test_helper

@test "Edit nested conflict" {
    mkdir a

    cd a
    pijul_uncovered init
    touch file
    pijul_uncovered add file
    pijul_uncovered record -a -m "file" -A "Me"
    cd ..

    pijul_uncovered clone a b

    cd a
    echo -e "a\nb\nc" > file
    pijul_uncovered record -a -m "abc" -A "Me"

    cd ../b
    echo -e "d\ne\nf" > file
    pijul_uncovered record -a -m "def" -A "Me"
    pijul_uncovered pull -a ../a

    cd ../a
    pijul_uncovered pull -a ../b
    sed -i -e "s/a/y/" file
    sed -i -e "s/d/v/" file
    pijul record -a -m "s/a/y" -A "Me"

    cd ../b
    sed -i -e "s/a/x/" file
    sed -i -e "s/d/u/" file
    pijul record -a -m "s/a/x" -A "Me"
    pijul revert -a
    pijul pull -a ../a

    # First check, do we have the expected lines?
    # this is not 100% correct, but there are 4 different outcomes.
    sort file > file2
    sort $BATS_TEST_DIRNAME/../expected/edit_nested_conflict > expected
    diff file2 expected

    # Then keep editing
}
