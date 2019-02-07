#!/usr/bin/env bats

load ../test_helper

@test "revert" {
    mkdir subdir
    touch subdir/file.txt
    pijul init subdir
    pijul add --repository subdir file.txt
    pijul record -a --repository subdir -m "add empty file.txt" -A creator
    cat > subdir/file.txt << EOF
me
me
me
me
me
EOF
    RUST_LOG="libpijul=debug" pijul revert -a --repository subdir 2> /tmp/log
    test -e subdir/file.txt
    test ! -s subdir/file.txt
}
