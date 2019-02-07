#!/usr/bin/env bats

load ../test_helper

@test "dist works" {
    make_repo toto
    cd toto
    touch act1 act2 act3
    mkdir d
    touch d/act4 d/act5
    pijul add act1 act2 d d/act4
    pijul record -am "bunch of files" -A "Élisabeth Jacquet de la Guerre"
    pijul dist -d "cephale-et-procris-1.0"
    test -e cephale-et-procris-1.0.tar.gz
    run tar ztvf cephale-et-procris-1.0.tar.gz
    assert_output "cephale-et-procris-1.0/act1"
    assert_output "cephale-et-procris-1.0/act2"
    assert_output "cephale-et-procris-1.0/d/act4"
    [[ ! ( $output =~ act3 ) ]]
    [[ ! ( $output =~ d/act5 ) ]]
}

@test "dist fails with incorrect path" {
    make_repo toto
    cd toto
    touch act1 act2 act3
    mkdir d
    touch d/act4 d/act5
    pijul add act1 act2 d d/act4
    pijul record -am "bunch of files" -A "Élisabeth Jacquet de la Guerre"
    if $(pijul dist -d "cephale-et-procris-1.0" incorrect_path/); then
        assert_failure "pijul dist should have failed"
    fi

    if [ -f cephale-et-procris-1.0.tar.gz ]; then
        assert_failure "The archive was not correctly removed after failure"
    fi
}

@test "dist to file and dist to stdout gives the same result" {
    make_repo toto
    cd toto
    touch act1 act2 act3
    mkdir d
    touch d/act4 d/act5
    pijul add act1 act2 d d/act4
    pijul record -am "bunch of files" -A "Élisabeth Jacquet de la Guerre"
    pijul dist -d "cephale-et-procris-1.0"
    pijul dist -d "cephale-et-procris-1.0" --stdout > stdout.tar.gz

    diff cephale-et-procris-1.0.tar.gz stdout.tar.gz
}