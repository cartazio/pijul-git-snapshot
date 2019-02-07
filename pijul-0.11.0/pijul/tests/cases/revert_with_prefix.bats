#!/usr/bin/env bats

load ../test_helper

@test "revert with a prefix" {
    mkdir subdir
    pijul init subdir
    cat > subdir/a << EOF
a
b
c
d
EOF
    cat > subdir/b << EOF
w
x
y
z
EOF
    pijul add --repository subdir a b
    pijul record -a --repository subdir -m "add a and b" -A creator
    cp subdir/b subdir_b
    cat > subdir/a << EOF
a
b
blabla
c
d
EOF
    cp subdir/a subdir_a
    cat > subdir/b << EOF
w
x
blibli
y
z
EOF
    RUST_LOG="pijul=debug,libpijul::output=debug,libpijul::graph=debug" pijul revert --repository subdir -a b 2> /tmp/log
    cp subdir/a /tmp/subdir_a
    cp subdir/b /tmp/subdir_b
    assert_files_equal subdir/a subdir_a
    assert_files_equal subdir/b subdir_b
}
