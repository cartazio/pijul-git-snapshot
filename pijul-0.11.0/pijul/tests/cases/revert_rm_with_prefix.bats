#!/usr/bin/env bats

load ../test_helper

# Try to revert a file scheduled to be moved
@test "revert remove with a prefix" {
    make_repo toto
    cd toto
    echo 'fn main() { println!("Hello"); }' > foo.rs
    echo 'fn main() { println!("World"); }' > bar.rs
    pijul add foo.rs
    pijul add bar.rs

    pijul record -am "a" -A myself
    echo "a" >> bar.rs

    pijul remove foo.rs

    RUST_BACKTRACE=1 RUST_LOG="pijul=debug,libpijul::output=debug" pijul revert -a foo.rs 2> /tmp/log

    RUST_BACKTRACE=1 RUST_LOG="pijul=debug" pijul record -am "not rm" -A myself 2> /tmp/log2
    cd ..
    RUST_BACKTRACE=1 pijul clone toto tata 2> /tmp/log3

    assert_files_equal tata/bar.rs toto/bar.rs
    assert_files_equal tata/foo.rs toto/foo.rs
}
