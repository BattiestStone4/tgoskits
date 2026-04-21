#!/bin/sh
# BusyBox basic test group: pure userspace computation commands
# No special kernel features required.
# Adapted from ChenLongTest (https://github.com/asmcos/ChenLongTest)

pass=0
fail=0

run_test() {
    _name="$1"
    _cmd="$2"
    _expect="$3"
    _out=$($_cmd 2>&1)
    _rc=$?
    if [ -n "$_expect" ]; then
        if echo "$_out" | grep -q "$_expect"; then
            echo "PASS: $_name"
            pass=$((pass + 1))
        else
            echo "FAIL: $_name [expected '$_expect' got '$_out']"
            fail=$((fail + 1))
        fi
    else
        if [ -n "$_out" ]; then
            echo "PASS: $_name"
            pass=$((pass + 1))
        else
            echo "FAIL: $_name [empty output]"
            fail=$((fail + 1))
        fi
    fi
}

run_test_rc() {
    _name="$1"
    _cmd="$2"
    _expect_rc="$3"
    _expect="$4"
    _out=$($_cmd 2>&1)
    _rc=$?
    if [ "$_rc" -eq "$_expect_rc" ] && [ -z "$_expect" -o "$(echo "$_out" | grep -c "$_expect")" -gt 0 ]; then
        echo "PASS: $_name"
        pass=$((pass + 1))
    else
        echo "FAIL: $_name [rc=$_rc expected_rc=$_expect_rc out='$_out']"
        fail=$((fail + 1))
    fi
}

# order 59: echo
run_test "busybox_echo" "busybox echo echo_ok" "echo_ok"

# order 13: base64
run_test "busybox_base64" "busybox echo test | busybox base64" "dGVzdAo="

# order 14: basename
run_test "busybox_basename" "busybox basename /usr/bin/foo" "foo"

# order 53: dirname
run_test "busybox_dirname" "busybox dirname /usr/bin/foo" "/usr/bin"

# order 12: awk
run_test "busybox_awk" "busybox awk 'BEGIN{print \"awk_ok\"}'" "awk_ok"

# order 219: sed
run_test "busybox_sed" "echo hello | busybox sed 's/hello/sed_ok/'" "sed_ok"

# order 86: grep
run_test "busybox_grep" "echo hello | busybox grep hell" "hello"

# order 60: egrep
run_test "busybox_egrep" "echo hello | busybox egrep hell" "hello"

# order 74: fgrep
run_test "busybox_fgrep" "echo hello | busybox fgrep hell" "hello"

# order 239: sort
run_test "busybox_sort" "printf 'c\na\nb\n' | busybox sort | busybox head -n1" "a"

# order 43: cut
run_test "busybox_cut" "echo 'a:b:c' | busybox cut -d: -f2" "b"

# order 261: tr
run_test "busybox_tr" "echo abc | busybox tr a-z A-Z" "ABC"

# order 221: seq
run_test "busybox_seq" "busybox seq 1 3" "3"

# order 212: rev
run_test "busybox_rev" "echo abcd | busybox rev" "dcba"

# order 236: shuf (check non-empty)
_out=$(printf 'a\nb\nc\n' | busybox shuf 2>&1)
if [ -n "$_out" ]; then
    echo "PASS: busybox_shuf"; pass=$((pass + 1))
else
    echo "FAIL: busybox_shuf [empty output]"; fail=$((fail + 1))
fi

# order 78: fold
run_test "busybox_fold" "echo abcdef | busybox fold -w 2" "ab"

# order 64: expand
_out=$(printf 'a\tb\n' | busybox expand 2>&1)
if echo "$_out" | grep -q 'a.*b'; then
    echo "PASS: busybox_expand"; pass=$((pass + 1))
else
    echo "FAIL: busybox_expand [got '$_out']"; fail=$((fail + 1))
fi

# order 37: comm
_tmp1=$(mktemp); _tmp2=$(mktemp)
printf 'a\nb\nc\n' > "$_tmp1"; printf 'b\nc\nd\n' > "$_tmp2"
run_test "busybox_comm" "busybox comm $_tmp1 $_tmp2" "c"
rm -f "$_tmp1" "$_tmp2"

# order 183: paste
_tmp1=$(mktemp); _tmp2=$(mktemp)
printf 'a\n' > "$_tmp1"; printf 'b\n' > "$_tmp2"
_out=$(busybox paste "$_tmp1" "$_tmp2" 2>&1)
if echo "$_out" | grep -q 'a.*b'; then
    echo "PASS: busybox_paste"; pass=$((pass + 1))
else
    echo "FAIL: busybox_paste [got '$_out']"; fail=$((fail + 1))
fi
rm -f "$_tmp1" "$_tmp2"

# order 275: uniq
run_test "busybox_uniq" "printf 'a\na\nb\n' | busybox uniq" "b"

# order 293: wc
run_test "busybox_wc" "printf 'a\nb\nc\n' | busybox wc -l" "3"

# order 11: ash
run_test "busybox_ash" "busybox ash -c 'echo ash_ok'" "ash_ok"

# order 229: sh
run_test "busybox_sh" "busybox sh -c 'echo sh_ok'" "sh_ok"

# order 299: xargs
run_test "busybox_xargs" "echo a b | busybox xargs echo X" "X a b"

# order 194: printf
run_test "busybox_printf" "busybox printf 'pf_%s_ok' bb" "pf_bb_ok"

# order 65: expr
run_test "busybox_expr" "busybox expr 3 '*' 4" "12"

# order 16: bc
run_test "busybox_bc" "busybox echo '2+2' | busybox bc" "4"

# order 45: dc
run_test "busybox_dc" "echo '2 2 + p' | busybox dc" "4"

# order 66: factor
run_test "busybox_factor" "busybox factor 6" "2 3"

# order 252: tac
run_test "busybox_tac" "printf 'a\nb\n' | busybox tac" "a"

# order 171: nl
run_test "busybox_nl" "busybox nl -ba /etc/passwd" "root:"

# order 238: sleep
run_test "busybox_sleep" "busybox sleep 0 && echo sleep_ok" "sleep_ok"

# order 68: false
run_test_rc "busybox_false" "busybox false" 1 ""

# order 265: true
_out=$(busybox true && echo true_ok 2>&1)
if echo "$_out" | grep -q "true_ok"; then
    echo "PASS: busybox_true"; pass=$((pass + 1))
else
    echo "FAIL: busybox_true"; fail=$((fail + 1))
fi

# order 256: test
run_test "busybox_test" "busybox test 1 -eq 1 && echo test_ok" "test_ok"

# order 84: getopt
run_test "busybox_getopt" "busybox getopt -o ab: -- -a -b bar" "-a"

# order 15: bbconfig
run_test "busybox_bbconfig" "busybox bbconfig 2>&1 | busybox head -1" "CONFIG_BUSYBOX"

# order 145: md5sum
run_test "busybox_md5sum" "echo | busybox md5sum" "-"

# order 230: sha1sum
run_test "busybox_sha1sum" "echo | busybox sha1sum" "-"

# order 231: sha256sum
run_test "busybox_sha256sum" "echo | busybox sha256sum" "-"

# order 233: sha512sum
run_test "busybox_sha512sum" "echo | busybox sha512sum" "-"

# order 232: sha3sum
run_test "busybox_sha3sum" "echo | busybox sha3sum" "-"

# order 179: od
run_test "busybox_od" "echo test | busybox od -tx1" "74"

# order 93: hexdump
run_test "busybox_hexdump" "echo -n ab | busybox hexdump -C" "61"

# order 91: hd
run_test "busybox_hd" "busybox hd -n 64 /etc/passwd" "00000000"

# order 300: xxd
run_test "busybox_xxd" "printf Hi | busybox xxd" "48 69"

# order 242: strings
run_test "busybox_strings" "busybox strings /bin/busybox" "BusyBox"

# order 245: sum
_out=$(echo | busybox sum 2>&1)
if [ -n "$_out" ]; then
    echo "PASS: busybox_sum"; pass=$((pass + 1))
else
    echo "FAIL: busybox_sum [empty output]"; fail=$((fail + 1))
fi

# order 34: cksum
run_test "busybox_cksum" "busybox cksum /etc/passwd" "/etc/passwd"

# order 302: yes
run_test "busybox_yes" "busybox yes y | busybox head -n1" "y"

echo ""
echo "DONE: $pass pass, $fail fail"
