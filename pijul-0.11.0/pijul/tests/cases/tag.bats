#!/usr/bin/env bats

load ../test_helper

@test "Tag" {
    mkdir a
    cd a
    pijul_uncovered init
    echo -e "a\nc\nd\ne" > a
    pijul_uncovered add a
    pijul_uncovered record -a -m "+ac" -A "Me"
    rm .pijul/meta.toml

    echo "me" | pijul tag -m "tag"
    echo "message" | pijul tag
    pijul tag -m "tag" -A "you"

    rm -f .pijul/meta.toml
    echo "message" | pijul tag --no-editor

    rm -f .pijul/meta.toml $HOME/.pijulconfig/config.toml
    pijul_uncovered key gen --signing --local
    pijul tag -m "tag" -A "me"
}
