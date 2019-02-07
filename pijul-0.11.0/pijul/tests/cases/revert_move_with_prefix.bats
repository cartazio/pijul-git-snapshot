#!/usr/bin/env bats

load ../test_helper

# Try to revert a file scheduled to be moved
@test "revert move with a prefix" {
    make_repo toto
    cd toto
    echo 'fn main() { println!("Hello"); }' > foo.rs
    echo 'fn main() { println!("World"); }' > bar.rs
    pijul add foo.rs
    pijul add bar.rs

    pijul record -am "a" -A myself
    echo "a" >> bar.rs
    cp bar.rs bar.rs_backup

    pijul mv foo.rs baz.rs

    RUST_BACKTRACE=1 RUST_LOG="pijul=debug,libpijul::output=debug" pijul revert -a baz.rs 2> /tmp/log

    assert_files_equal bar.rs bar.rs_backup
    cat foo.rs
}
