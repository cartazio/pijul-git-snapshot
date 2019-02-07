#!/usr/bin/env bats

load ../test_helper

@test "unrecord file moves" {
    mkdir a
    pijul init a

    echo "a\nb\nc" > file
    cp file a
    pijul add --repository a file
    pijul record --repository a -a -m "Add file" -A "Me"
    pijul clone a b

    echo "a\nd\nc" > file
    cp file a
    pijul record --repository a -a -m "Remove d in a" -A "Me"
    cp file b
    HASH=$(pijul record --repository b -a -m "Remove d in b" -A "Me" | sed -e 's/Recorded patch //')

    pijul pull --repository a -a b

    pijul unrecord --repository a "$HASH"
    pijul revert --repository a -a

    assert_files_equal "a/file" "file"
}
