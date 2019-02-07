#!/usr/bin/env bats

load ../test_helper

@test "Patch" {
    mkdir a
    cd a
    pijul_uncovered init
    echo -e "a\nc\nd\ne" > a
    echo -e "a\nc\nd\ne" > b
    pijul_uncovered add a
    pijul_uncovered add b
    HASH=$(pijul_uncovered record -a -m "+ac" -A "Me" | sed -e 's/Recorded patch //')
    echo -e "a\nb\nc\ne" > a
    cd ..
    pijul clone a b
    cd a
    HASH2=$(pijul_uncovered record -a -m "+b" -A "Me" | sed -e 's/Recorded patch //')
    echo "HASH"
    RUST_LOG="pijul=debug" RUST_BACKTRACE=1 pijul patch $HASH 2> /tmp/log
    echo "HASH 2"
    RUST_BACKTRACE=1 pijul patch $HASH
    RUST_BACKTRACE=1 pijul patch $HASH2
    RUST_BACKTRACE=1 pijul patch --description $HASH2
    RUST_BACKTRACE=1 pijul patch --date $HASH2
    RUST_BACKTRACE=1 pijul patch --authors $HASH2
    RUST_BACKTRACE=1 pijul patch --name $HASH2

    cd ../b
    echo -e "a\nx\nd\ne" > a
    HASH3=$(pijul_uncovered record -a -m "+x" -A "Me" | sed -e 's/Recorded patch //')
    cd ../a
    pijul_uncovered pull -a ../b
    RUST_BACKTRACE=1 pijul patch $HASH3
}
