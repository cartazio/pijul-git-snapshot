#!/usr/bin/env bats

load ../test_helper

@test "move to dir merge" {
    # skip
    make_two_repos titi toto
    cd toto
    echo "version 1" > a
    pijul add a
    pijul record -am a -A a
    echo "version 1" > b
    pijul add b
    pijul record -am b -A b
    mkdir d
    pijul add d
    pijul mv a d
    pijul record -am move -A mover
    pijul mv b d
    pijul record -am move -A mover
    cd ../titi
    echo yyn | pijul pull ../toto
    echo "version 2" > a
    echo "version 2" > b
    pijul record -am a2b2 -A changer
    pijul pull -a ../toto
    run cat d/a
    assert_output "version 2"
    run cat d/b
    assert_output "version 2"
    test ! -e a
    test ! -e b
}
