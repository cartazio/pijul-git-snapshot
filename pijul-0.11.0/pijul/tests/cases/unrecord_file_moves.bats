#!/usr/bin/env bats

load ../test_helper

@test "unrecord file moves" {
    mkdir toto
    cd toto
    pijul_uncovered init
    echo a > f
    pijul_uncovered add f
    pijul_uncovered record -a -m "+f" -A "Me"
    pijul_uncovered mv f g
    RUST_BACKTRACE=1 pijul_uncovered record -a -m "mv f g"

    echo yd | pijul unrecord # 2> /tmp/log

    run pijul_uncovered diff
    assert_success "file moved to:"
}
