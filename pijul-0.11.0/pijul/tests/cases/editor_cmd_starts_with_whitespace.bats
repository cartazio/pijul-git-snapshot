#!/usr/bin/env bats

load ../test_helper

# given editor starts with whitespace,
# then pijul ignores whitespace and runs editor.

@test "editor command starts with whitespace" {
    create_editor editor.sh "TOKEN"
    make_repo a

    write_meta_file a <<EOF
editor = " ../editor.sh"
EOF

    make_random_file "a/file.txt"
    pijul add --repository a file.txt
    pijul record -a -A me --repository a
    run pijul log --repository a
    assert_output "TOKEN"
}


# helper
create_editor() {
    local editor="$1" msg="$2"
    cat <<EOF > "$editor"
#!/usr/bin/env bash
echo "$msg" > "\$1"
EOF
    chmod 700 "$editor"
}
