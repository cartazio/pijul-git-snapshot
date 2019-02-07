#!/usr/bin/env bats

load ../test_helper

@test "move merge" {
    make_two_repos titi toto
    cd toto
    echo "version 1" > a
    pijul add a
    pijul record -am a -A a
    pijul mv a b
    pijul record -am move -A mover
    cd ../titi
    echo yn | pijul pull ../toto
    echo "version 2" > a
    pijul record -am a2 -A changer
    pijul pull -a ../toto
    run cat b
    assert_output "version 2"
    test ! -e a
}
