#!/usr/bin/env bats

load ../test_helper

@test "partial checkout" {
    mkdir origin
    cd origin
    pijul init
    mkdir -p a/b a/c d
    make_random_file a/c/y
    make_random_file d/z

    pijul add a/b
    pijul record -a -m abx -A me

    pijul add a/c/y d/z
    pijul record -a -m acydz -A me

    pijul mv d/z a/b
    pijul record -a -m "dz->ab" -A me

    cd ..
    mkdir clone
    cd clone

    pijul init
    pijul pull -a ../origin --path a/b/z

    if [[ "$(ls | wc -l)" -ne "1" ]]; then
        return 1
    fi

    pijul revert -a
    if [[ "$(ls | wc -l)" -ne "1" ]]; then
        return 1
    fi

    pijul revert -a
    if [[ "$(ls | wc -l)" -ne "1" ]]; then
        return 1
    fi
}

@test "partial clone" {
  mkdir test
  cd test
  pijul init
  mkdir a b
  echo test > a/test
  echo foo > b/foo
  pijul record -A "me" -n -a -m "patch"
  cd ..
  pijul clone test test-partial --path a/

  [ "$(ls test-partial)" = "a" ]
}
